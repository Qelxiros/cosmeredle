//! Tests for `db::store_answer` / `db::get_answer`.
//!
//! Own test binary: `store_answer` clears the whole `answer` table.

mod common;

use chrono::NaiveDate;
use cosmeredle::db::{get_answer, store_answer};

fn date(y: i32, m: u32, d: u32) -> NaiveDate {
    NaiveDate::from_ymd_opt(y, m, d).expect("valid date")
}

#[tokio::test]
async fn round_trips_an_answer_and_its_date() {
    let _guard = common::serial().await;
    common::setup().await;
    store_answer("Vin", date(2026, 9, 16)).await.expect("store");

    let (answer, day) = get_answer().await.expect("query").expect("stored");
    assert_eq!(answer, "Vin");
    assert_eq!(day, date(2026, 9, 16));
}

#[tokio::test]
async fn storing_replaces_the_previous_answer() {
    let _guard = common::serial().await;
    common::setup().await;
    store_answer("Old", date(2026, 9, 15)).await.expect("store");
    store_answer("New", date(2026, 9, 16)).await.expect("store");

    assert_eq!(common::count("SELECT count(*) FROM answer").await, 1);
    let (answer, day) = get_answer().await.expect("query").expect("stored");
    assert_eq!(answer, "New");
    assert_eq!(day, date(2026, 9, 16));
}

#[tokio::test]
async fn storing_the_same_answer_again_is_fine() {
    let _guard = common::serial().await;
    common::setup().await;
    // `answer` is the table's primary key, so this only works because
    // `store_answer` deletes before inserting.
    store_answer("Repeat", date(2026, 9, 15)).await.expect("first");
    store_answer("Repeat", date(2026, 9, 16)).await.expect("second");

    let (answer, day) = get_answer().await.expect("query").expect("stored");
    assert_eq!(answer, "Repeat");
    assert_eq!(day, date(2026, 9, 16));
}

#[tokio::test]
async fn an_empty_table_reads_back_as_none() {
    let _guard = common::serial().await;
    common::setup().await;
    common::exec("DELETE FROM answer").await;

    assert!(get_answer().await.expect("query").is_none());
}

#[tokio::test]
async fn dates_outside_the_common_range_survive_the_round_trip() {
    let _guard = common::serial().await;
    common::setup().await;
    for d in [date(1, 1, 1), date(1999, 12, 31), date(2100, 2, 28)] {
        store_answer("Boundary", d).await.expect("store");
        let (_, got) = get_answer().await.expect("query").expect("stored");
        assert_eq!(got, d);
    }
}

#[tokio::test]
async fn a_malformed_stored_date_is_reported_as_an_error() {
    let _guard = common::serial().await;
    common::setup().await;
    common::exec("DELETE FROM answer").await;
    common::exec("INSERT INTO answer (answer, date) VALUES ('Corrupt', 'not-a-date')").await;

    let err = get_answer().await.expect_err("unparseable date");
    assert_eq!(err.to_string(), "Failed to parse date from database");
}

#[tokio::test]
async fn an_answer_name_is_stored_verbatim() {
    let _guard = common::serial().await;
    common::setup().await;
    // Nothing checks that the stored answer is a real character.
    store_answer("Nobody At All", date(2026, 9, 16)).await.expect("store");
    let (answer, _) = get_answer().await.expect("query").expect("stored");
    assert_eq!(answer, "Nobody At All");
}
