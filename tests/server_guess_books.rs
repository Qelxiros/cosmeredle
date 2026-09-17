//! Book comparison when the answer's own book has no series.
//!
//! Own test binary because it pins a different answer from
//! tests/server_guess.rs.

mod common;

use common::http::Client;
use cosmeredle::{
    Book,
    db::{insert_book, insert_character, store_answer},
    server::{GuessResponse, TernaryStatus},
};
use serde_json::json;
use tokio::sync::OnceCell;

const ANSWER: &str = "Standalone Answer";

static SEED: OnceCell<()> = OnceCell::const_new();

async fn seed() {
    common::setup().await;
    SEED.get_or_init(|| async {
        for (name, introduced) in [
            (ANSWER, "Warbreaker"),
            ("Other Standalone", "Elantris"),
            ("Series Guess", "The Way of Kings"),
        ] {
            let c = common::character(
                name,
                &[
                    ("universe", "Cosmere"),
                    ("world", "Nalthis"),
                    ("species", "Human"),
                    ("introduced", introduced),
                ],
                &[],
            );
            insert_character(&c).await.unwrap_or_else(|e| panic!("insert {name}: {e}"));
        }

        for (title, series) in [
            ("Warbreaker", None),
            ("Elantris", None),
            ("The Way of Kings", Some("The Stormlight Archive")),
        ] {
            insert_book(&Book {
                title: title.to_string(),
                series: series.map(str::to_string),
            })
            .await
            .unwrap_or_else(|e| panic!("insert {title}: {e}"));
        }

        store_answer(ANSWER, common::today()).await.expect("pin the answer");
    })
    .await;
}

async fn book_status(name: &str) -> TernaryStatus {
    seed().await;
    let mut c = Client::new().await;
    let res = c.post("/guess", &json!(name)).await;
    let parsed: GuessResponse = serde_json::from_str(&res.body).expect("GuessResponse");
    parsed.book
}

/// Two standalones share no series, so `None` must not match `None`.
#[tokio::test]
async fn two_unrelated_standalones_are_not_adjacent() {
    assert_eq!(
        book_status("Other Standalone").await,
        TernaryStatus::Incorrect,
        "Elantris and Warbreaker share no series"
    );
}

#[tokio::test]
async fn a_book_with_a_series_is_not_adjacent_to_a_standalone() {
    assert_eq!(book_status("Series Guess").await, TernaryStatus::Incorrect);
}

#[tokio::test]
async fn the_answers_own_book_is_still_correct() {
    assert_eq!(book_status(ANSWER).await, TernaryStatus::Correct);
}
