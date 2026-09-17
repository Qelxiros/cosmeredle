//! `answer::today` when the stored answer is dated in the future, which
//! happens if the host clock or timezone moves backwards.

mod common;

use cosmeredle::answer::today;

#[tokio::test]
async fn a_future_stored_date_rotates_forward_by_its_distance() {
    common::setup().await;
    common::exec("DELETE FROM ability").await;
    common::exec("DELETE FROM character").await;
    common::exec("DELETE FROM answer").await;
    common::insert_cosmere("Future A", "Roshar", "Human", "Oathbringer", &[]).await;
    common::insert_cosmere("Future B", "Scadrial", "Human", "Mistborn", &[]).await;

    // The day count is an absolute difference, so "tomorrow" is treated
    // exactly like "yesterday" and the answer still steps forward.
    common::exec(&format!(
        "INSERT INTO answer (answer, date) VALUES ('Future A', '{}')",
        common::date_str(common::days_ago(-1))
    ))
    .await;

    assert_eq!(today().await.expect("answer"), "Future B");
}
