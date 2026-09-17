//! `answer::today` with nothing to choose from.
//!
//! Each `answer_*.rs` file is a separate test binary on purpose: `today()`
//! memoises the day's answer in a process-global `static`, so one scenario per
//! process is the only way to exercise the branches independently.

mod common;

use cosmeredle::answer::today;

#[tokio::test]
async fn an_empty_character_table_is_an_error() {
    let _guard = common::serial().await;
    common::setup().await;
    common::exec("DELETE FROM ability").await;
    common::exec("DELETE FROM character").await;
    common::exec("DELETE FROM answer").await;

    let err = today().await.expect_err("nothing to pick");
    assert_eq!(err.to_string(), "No characters found");

    // and nothing was written
    assert_eq!(common::count("SELECT count(*) FROM answer").await, 0);
}

#[tokio::test]
async fn inactive_characters_do_not_count() {
    let _guard = common::serial().await;
    common::setup().await;
    common::exec("DELETE FROM ability").await;
    common::exec("DELETE FROM character").await;
    common::exec("DELETE FROM answer").await;

    common::insert_cosmere("Retired", "Roshar", "Human", "Oathbringer", &[]).await;
    common::exec("UPDATE character SET active = false").await;

    let err = today().await.expect_err("no active characters");
    assert_eq!(err.to_string(), "No characters found");
}
