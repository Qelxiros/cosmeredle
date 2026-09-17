//! `answer::today` after a gap, e.g. the server was down for a day.

mod common;

use cosmeredle::answer::today;

#[tokio::test]
async fn two_elapsed_days_over_two_characters_repeats_yesterdays_answer() {
    common::setup().await;
    common::exec("DELETE FROM ability").await;
    common::exec("DELETE FROM character").await;
    common::exec("DELETE FROM answer").await;
    common::insert_cosmere("Gap A", "Roshar", "Human", "Oathbringer", &[]).await;
    common::insert_cosmere("Gap B", "Scadrial", "Human", "Mistborn", &[]).await;

    // The answer advances by the number of days elapsed, so a gap skips
    // characters rather than showing each one in turn.
    common::exec(&format!(
        "INSERT INTO answer (answer, date) VALUES ('Gap A', '{}')",
        common::date_str(common::days_ago(2))
    ))
    .await;

    assert_eq!(
        today().await.expect("answer"),
        "Gap A",
        "two steps through two characters wraps back to the start"
    );
}
