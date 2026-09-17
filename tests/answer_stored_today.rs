//! `answer::today` when the database already holds today's answer, which is
//! the path a restarted server takes.

mod common;

use cosmeredle::answer::today;

#[tokio::test]
async fn todays_stored_answer_is_returned_verbatim_but_not_cached() {
    common::setup().await;
    common::exec("DELETE FROM ability").await;
    common::exec("DELETE FROM character").await;
    common::exec("DELETE FROM answer").await;
    common::insert_cosmere("Only Character", "Roshar", "Human", "Oathbringer", &[]).await;

    // Deliberately not a character in the table: this path does no validation.
    common::exec(&format!(
        "INSERT INTO answer (answer, date) VALUES ('Stored Name', '{}')",
        common::today_str()
    ))
    .await;

    assert_eq!(today().await.expect("answer"), "Stored Name");
    assert_eq!(today().await.expect("answer"), "Stored Name");

    // This early return never populates the in-process cache, so every request
    // re-reads the database, and losing the row changes the day's answer.
    common::exec("DELETE FROM answer").await;
    assert_eq!(
        today().await.expect("answer"),
        "Only Character",
        "with the row gone the answer is recomputed mid-day"
    );
}
