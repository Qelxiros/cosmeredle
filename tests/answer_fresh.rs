//! `answer::today` on a database that has never had an answer stored.

mod common;

use cosmeredle::{answer::today, db::get_answer};

const NAMES: [&str; 3] = ["Fresh One", "Fresh Two", "Fresh Three"];

async fn seed() {
    common::setup().await;
    common::exec("DELETE FROM ability").await;
    common::exec("DELETE FROM character").await;
    common::exec("DELETE FROM answer").await;
    for name in NAMES {
        common::insert_cosmere(name, "Roshar", "Human", "Oathbringer", &[]).await;
    }
}

#[tokio::test]
async fn picks_an_active_character_persists_it_and_then_caches_it() {
    seed().await;

    let answer = today().await.expect("an answer");
    assert!(NAMES.contains(&answer.as_str()), "picked {answer:?}");

    // It is written back so that tomorrow's rotation has a starting point.
    let (stored, day) = get_answer().await.expect("query").expect("stored");
    assert_eq!(stored, answer);
    assert_eq!(day, common::today());
    assert_eq!(common::count("SELECT count(*) FROM answer").await, 1);

    // A second call is stable.
    assert_eq!(today().await.expect("an answer"), answer);

    // ...and served from the in-process cache, not the database: changing the
    // stored row has no effect for the rest of the day.
    common::exec("DELETE FROM answer").await;
    common::exec(&format!(
        "INSERT INTO answer (answer, date) VALUES ('Fresh Two', '{}')",
        common::today_str()
    ))
    .await;
    assert_eq!(
        today().await.expect("an answer"),
        answer,
        "the cached answer should win until the date changes"
    );
}
