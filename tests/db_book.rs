//! Tests for the `book` table accessors.
//!
//! Nothing in the running app reaches these: `sync_books` has no caller, and
//! `wiki::get_book` can never return a book while `parse_table` hard-codes the
//! character infobox (see tests/wiki_parse.rs).

mod common;

use cosmeredle::{
    Book,
    db::{book_present, get_book, insert_book},
};

fn book(title: &str, series: Option<&str>) -> Book {
    Book {
        title: title.to_string(),
        series: series.map(str::to_string),
    }
}

#[tokio::test]
async fn insert_then_read_back_a_book() {
    common::setup().await;
    let original = book("The Way of Kings", Some("The Stormlight Archive"));
    insert_book(&original).await.expect("insert");

    let fetched = get_book("The Way of Kings").await.expect("query").expect("stored");
    assert_eq!(fetched, original);
}

#[tokio::test]
async fn a_book_without_a_series_round_trips_as_none() {
    common::setup().await;
    let original = book("Warbreaker", None);
    insert_book(&original).await.expect("insert");

    let fetched = get_book("Warbreaker").await.expect("query").expect("stored");
    assert_eq!(fetched.series, None);
    assert_eq!(fetched, original);
}

#[tokio::test]
async fn inserting_the_same_title_twice_is_a_conflict() {
    common::setup().await;
    insert_book(&book("Elantris", Some("Elantris"))).await.expect("first");

    let err = insert_book(&book("Elantris", Some("Other")))
        .await
        .expect_err("title is the primary key");
    assert!(err.to_string().contains("UNIQUE"), "{err}");

    let fetched = get_book("Elantris").await.expect("query").expect("stored");
    assert_eq!(fetched.series.as_deref(), Some("Elantris"));
}

#[tokio::test]
async fn several_books_may_share_a_series() {
    common::setup().await;
    insert_book(&book("Mistborn 1", Some("Mistborn"))).await.expect("insert");
    insert_book(&book("Mistborn 2", Some("Mistborn"))).await.expect("insert");

    assert!(book_present("Mistborn 1").await);
    assert!(book_present("Mistborn 2").await);
}

#[tokio::test]
async fn unknown_titles_are_absent() {
    common::setup().await;
    assert!(get_book("No Such Book").await.expect("query").is_none());
    assert!(!book_present("No Such Book").await);
    assert!(!book_present("Nor This One").await);
}

#[tokio::test]
async fn title_lookup_is_exact() {
    common::setup().await;
    insert_book(&book("Oathbringer", None)).await.expect("insert");

    assert!(book_present("Oathbringer").await);
    assert!(!book_present("oathbringer").await);
    assert!(!book_present("Oathbringer ").await);
}

#[tokio::test]
async fn titles_are_stored_verbatim() {
    common::setup().await;
    // No validation or normalisation of any kind.
    for title in ["", "   ", "Ünïcödé: A Novel"] {
        insert_book(&book(title, None)).await.expect("insert");
        assert!(book_present(title).await, "{title:?}");
    }
}
