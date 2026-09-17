//! The initialisation contract of the two global `OnceLock`s.
//!
//! This binary deliberately never calls `db::init()` or `wiki::init()`, so the
//! statics stay empty. Note what is *not* covered anywhere: `wiki`'s parsing
//! helpers (`titlecase`, `format`, `format_abilities`, `parse_table`) are
//! private, and every public `wiki` entry point calls out to coppermind.net,
//! so an integration test cannot reach the wikitext parser at all.

use cosmeredle::{db, wiki};

#[test]
#[should_panic(expected = "Option::unwrap()")]
fn db_conn_panics_before_init() {
    let _ = db::conn();
}

#[tokio::test]
#[should_panic(expected = "Option::unwrap()")]
async fn wiki_calls_panic_before_init() {
    // Panics on `API.get().unwrap()` before any request is made, so this test
    // does not touch the network.
    let _ = wiki::get_template_pages("Template:Character").await;
}
