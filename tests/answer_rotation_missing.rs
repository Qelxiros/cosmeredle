//! `answer::today` when the previous answer is no longer a playable character
//! (it was retired from the wiki, or the row was written by hand).

mod common;

use cosmeredle::{answer::today, db::get_answer};

const NAMES: [&str; 2] = ["Missing A", "Missing B"];

#[tokio::test]
async fn an_unknown_previous_answer_still_yields_a_playable_character() {
    common::setup().await;
    common::exec("DELETE FROM ability").await;
    common::exec("DELETE FROM character").await;
    common::exec("DELETE FROM answer").await;
    for name in NAMES {
        common::insert_cosmere(name, "Roshar", "Human", "Oathbringer", &[]).await;
    }

    // Not in the character table: `binary_search_by_key` takes its Err branch.
    common::exec(&format!(
        "INSERT INTO answer (answer, date) VALUES ('Retired Character', '{}')",
        common::date_str(common::days_ago(1))
    ))
    .await;

    let answer = today().await.expect("answer");
    assert!(NAMES.contains(&answer.as_str()), "picked {answer:?}");

    let (stored, day) = get_answer().await.expect("query").expect("stored");
    assert_eq!(stored, answer);
    assert_eq!(day, common::today());
}
