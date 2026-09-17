//! Tests for the user/auth queries in `db`.

mod common;

use cosmeredle::{
    Error,
    db::{get_user, get_user_auth_by_id, get_user_auth_by_name, insert_user},
};

#[tokio::test]
async fn insert_then_look_up_by_name() {
    common::setup().await;
    insert_user("alice".into(), "$2b$12$hash".into())
        .await
        .expect("insert");

    let user = get_user_auth_by_name("alice")
        .await
        .expect("query")
        .expect("alice exists");
    assert_eq!(user.username, "alice");
    assert_eq!(user.bcrypt, "$2b$12$hash");
    assert!(user.id > 0);
}

#[tokio::test]
async fn look_up_by_id_round_trips() {
    common::setup().await;
    insert_user("bob".into(), "hash-bob".into()).await.expect("insert");

    let by_name = get_user_auth_by_name("bob").await.expect("query").expect("bob");
    let by_id = get_user_auth_by_id(by_name.id)
        .await
        .expect("query")
        .expect("same user by id");

    assert_eq!(by_id.id, by_name.id);
    assert_eq!(by_id.username, "bob");
    assert_eq!(by_id.bcrypt, "hash-bob");
}

#[tokio::test]
async fn unknown_user_is_none_not_an_error() {
    common::setup().await;
    assert!(get_user_auth_by_name("nobody-at-all").await.expect("query").is_none());
    assert!(get_user_auth_by_id(987_654).await.expect("query").is_none());
    assert!(get_user(987_654).await.expect("query").is_none());
}

#[tokio::test]
async fn username_lookup_is_case_and_whitespace_sensitive() {
    common::setup().await;
    insert_user("Carol".into(), "hash".into()).await.expect("insert");

    assert!(get_user_auth_by_name("Carol").await.expect("query").is_some());
    assert!(get_user_auth_by_name("carol").await.expect("query").is_none());
    assert!(get_user_auth_by_name(" Carol").await.expect("query").is_none());
}

#[tokio::test]
async fn duplicate_username_is_a_conflict() {
    common::setup().await;
    insert_user("dave".into(), "hash".into()).await.expect("first");

    let err = insert_user("dave".into(), "other-hash".into())
        .await
        .expect_err("second insert must fail");
    assert!(matches!(err, Error::Sql(_)), "{err}");

    // and the original row is untouched
    let user = get_user_auth_by_name("dave").await.expect("query").expect("dave");
    assert_eq!(user.bcrypt, "hash");
}

#[tokio::test]
async fn usernames_and_hashes_are_stored_without_validation() {
    common::setup().await;
    // Documents that the db layer imposes no constraints of its own.
    for name in ["", "   ", "Ünïcödé 👑", &"n".repeat(1000)] {
        insert_user(name.to_string(), String::new())
            .await
            .unwrap_or_else(|e| panic!("insert {name:?}: {e}"));
        let u = get_user_auth_by_name(name).await.expect("query").expect("present");
        assert_eq!(u.username, name);
        assert_eq!(u.bcrypt, "", "an empty bcrypt hash is accepted");
    }
}

#[tokio::test]
async fn get_user_reports_an_empty_guess_list_when_there_are_none() {
    common::setup().await;
    insert_user("erin".into(), "hash".into()).await.expect("insert");
    let id = get_user_auth_by_name("erin").await.expect("query").expect("erin").id;

    let user = get_user(id).await.expect("query").expect("erin");
    assert_eq!(user.username, "erin");
    assert!(user.guesses.0.is_empty(), "{:?}", user.guesses.0);
}

#[tokio::test]
async fn get_user_aggregates_guesses_across_every_day() {
    common::setup().await;
    insert_user("frank".into(), "hash".into()).await.expect("insert");
    let id = get_user_auth_by_name("frank").await.expect("query").expect("frank").id;

    let today = common::today_str();
    let old = common::date_str(common::days_ago(30));
    common::exec(&format!(
        "INSERT INTO guess (user_id, guess, day) VALUES
         ({id}, 'Today Guess', '{today}'), ({id}, 'Ancient Guess', '{old}')"
    ))
    .await;

    let user = get_user(id).await.expect("query").expect("frank");
    let mut guesses = user.guesses.0.clone();
    guesses.sort();

    // `get_user` has no day filter, unlike `get_guesses`.
    assert_eq!(guesses, vec!["Ancient Guess".to_string(), "Today Guess".to_string()]);
}

#[tokio::test]
async fn get_user_does_not_leak_other_users_guesses() {
    common::setup().await;
    insert_user("gina".into(), "hash".into()).await.expect("insert");
    insert_user("hank".into(), "hash".into()).await.expect("insert");
    let gina = get_user_auth_by_name("gina").await.expect("q").expect("gina").id;
    let hank = get_user_auth_by_name("hank").await.expect("q").expect("hank").id;

    let today = common::today_str();
    common::exec(&format!(
        "INSERT INTO guess (user_id, guess, day) VALUES ({hank}, 'Hank Only', '{today}')"
    ))
    .await;

    let user = get_user(gina).await.expect("query").expect("gina");
    assert!(user.guesses.0.is_empty(), "{:?}", user.guesses.0);
}

#[tokio::test]
async fn user_debug_does_not_print_the_password_hash() {
    common::setup().await;
    insert_user("iris".into(), "$2b$12$supersecrethash".into())
        .await
        .expect("insert");
    let id = get_user_auth_by_name("iris").await.expect("q").expect("iris").id;
    let user = get_user(id).await.expect("q").expect("iris");

    let dbg = format!("{user:?}");
    assert!(!dbg.contains("supersecrethash"), "hash leaked into Debug: {dbg}");
    assert!(dbg.contains("iris"));
}

#[tokio::test]
async fn user_auth_debug_does_print_the_password_hash() {
    common::setup().await;
    // `db::User` redacts `bcrypt` via `#[dbg(skip)]`; `backend::UserAuth`
    // derives plain `Debug` and does not.
    insert_user("jane".into(), "$2b$12$anothersecret".into())
        .await
        .expect("insert");
    let auth = get_user_auth_by_name("jane").await.expect("q").expect("jane");

    assert!(format!("{auth:?}").contains("anothersecret"));
}
