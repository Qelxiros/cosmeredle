//! Tests for `db::insert_guess` / `db::get_guesses`.

mod common;

use cosmeredle::db::{get_guesses, get_user_auth_by_name, insert_guess, insert_user};

async fn user(name: &str) -> i64 {
    common::setup().await;
    insert_user(name.into(), "hash".into()).await.expect("insert user");
    get_user_auth_by_name(name).await.expect("query").expect("user").id
}

#[tokio::test]
async fn guesses_round_trip_for_today() {
    let id = user("guess-round-trip").await;
    assert!(get_guesses(id).await.expect("query").is_empty());

    insert_guess(id, "Vin".into()).await.expect("insert");
    insert_guess(id, "Kelsier".into()).await.expect("insert");

    let mut guesses = get_guesses(id).await.expect("query");
    guesses.sort();
    assert_eq!(guesses, vec!["Kelsier".to_string(), "Vin".to_string()]);
}

#[tokio::test]
async fn insert_guess_stamps_the_local_date() {
    let id = user("guess-stamps-date").await;
    insert_guess(id, "Dalinar".into()).await.expect("insert");

    let day = common::scalar(&format!(
        "SELECT day FROM guess WHERE user_id = {id} AND guess = 'Dalinar'"
    ))
    .await;
    assert_eq!(day.as_deref(), Some(common::today_str().as_str()));
}

#[tokio::test]
async fn get_guesses_only_returns_todays() {
    let id = user("guess-day-filter").await;
    let yesterday = common::date_str(common::days_ago(1));
    common::exec(&format!(
        "INSERT INTO guess (user_id, guess, day) VALUES ({id}, 'Yesterday Name', '{yesterday}')"
    ))
    .await;

    insert_guess(id, "Today Name".into()).await.expect("insert");

    assert_eq!(get_guesses(id).await.expect("query"), vec!["Today Name".to_string()]);
}

#[tokio::test]
async fn guesses_are_scoped_to_one_user() {
    let mine = user("guess-scope-mine").await;
    let theirs = user("guess-scope-theirs").await;

    insert_guess(mine, "Mine".into()).await.expect("insert");
    insert_guess(theirs, "Theirs".into()).await.expect("insert");

    assert_eq!(get_guesses(mine).await.expect("query"), vec!["Mine".to_string()]);
    assert_eq!(get_guesses(theirs).await.expect("query"), vec!["Theirs".to_string()]);
}

/// `guess` has a (user_id, guess, day) primary key, so re-guessing the same
/// character on the same day is a database error rather than a no-op.
#[tokio::test]
async fn repeating_a_guess_on_the_same_day_is_an_error() {
    let id = user("guess-duplicate").await;
    insert_guess(id, "Vin".into()).await.expect("first");

    let err = insert_guess(id, "Vin".into()).await.expect_err("duplicate");
    assert!(err.to_string().contains("UNIQUE"), "{err}");

    assert_eq!(get_guesses(id).await.expect("query").len(), 1);
}

#[tokio::test]
async fn the_same_guess_on_a_different_day_is_fine() {
    let id = user("guess-different-day").await;
    let yesterday = common::date_str(common::days_ago(1));
    common::exec(&format!(
        "INSERT INTO guess (user_id, guess, day) VALUES ({id}, 'Vin', '{yesterday}')"
    ))
    .await;

    insert_guess(id, "Vin".into()).await.expect("today's Vin is a new row");
    assert_eq!(
        common::count(&format!("SELECT count(*) FROM guess WHERE user_id = {id}")).await,
        2
    );
}

/// sqlx turns `PRAGMA foreign_keys` on for SQLite connections, so the `guess`
/// -> `user` foreign key is enforced.
#[tokio::test]
async fn guesses_for_a_user_that_does_not_exist_are_rejected() {
    common::setup().await;
    let err = insert_guess(424_242, "Orphan Guess".into())
        .await
        .expect_err("foreign key must be enforced");
    assert!(err.to_string().contains("FOREIGN KEY"), "{err}");

    assert!(get_guesses(424_242).await.expect("query").is_empty());
}

#[tokio::test]
async fn guess_text_is_stored_verbatim_and_unvalidated() {
    let id = user("guess-unvalidated").await;
    // Nothing checks that a guess names a real character.
    for guess in ["", "   ", "Not A Character", "'; DROP TABLE guess; --"] {
        insert_guess(id, guess.to_string())
            .await
            .unwrap_or_else(|e| panic!("insert {guess:?}: {e}"));
    }

    let guesses = get_guesses(id).await.expect("query");
    assert_eq!(guesses.len(), 4);
    assert!(guesses.contains(&"'; DROP TABLE guess; --".to_string()));
    assert!(common::count("SELECT count(*) FROM guess").await > 0, "table survived");
}
