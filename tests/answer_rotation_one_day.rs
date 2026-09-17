//! `answer::today` rotating forward from yesterday's answer.
//!
//! The rotation walks characters in hash order, which the test cannot predict.
//! Seeding exactly two characters sidesteps that: the character after X is
//! always "the other one", whatever the hash order happens to be.

mod common;

use cosmeredle::{answer::today, db::get_answer};

#[tokio::test]
async fn one_elapsed_day_advances_to_the_next_character() {
    common::setup().await;
    common::exec("DELETE FROM ability").await;
    common::exec("DELETE FROM character").await;
    common::exec("DELETE FROM answer").await;
    common::insert_cosmere("Rotate A", "Roshar", "Human", "Oathbringer", &[]).await;
    common::insert_cosmere("Rotate B", "Scadrial", "Human", "Mistborn", &[]).await;

    common::exec(&format!(
        "INSERT INTO answer (answer, date) VALUES ('Rotate A', '{}')",
        common::date_str(common::days_ago(1))
    ))
    .await;

    let answer = today().await.expect("answer");
    assert_eq!(
        answer, "Rotate B",
        "with two characters, one day on from A must be B"
    );

    let (stored, day) = get_answer().await.expect("query").expect("stored");
    assert_eq!(stored, "Rotate B");
    assert_eq!(day, common::today());
}
