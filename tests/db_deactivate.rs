//! Tests for `db::deactivate_characters` / `db::reactivate_characters`, the
//! "retire characters the wiki no longer lists" half of the sync.
//!
//! This lives in its own test binary because it rewrites every row in the
//! `character` table.

mod common;

use cosmeredle::db::{all_characters, deactivate_characters, reactivate_characters};

async fn seed() -> tokio::sync::MutexGuard<'static, ()> {
    let guard = common::serial().await;
    common::setup().await;
    common::exec("DELETE FROM ability").await;
    common::exec("DELETE FROM character").await;
    for name in ["Alpha", "Beta", "Gamma"] {
        common::insert_cosmere(name, "Roshar", "Human", "Oathbringer", &[]).await;
    }
    guard
}

async fn active() -> Vec<String> {
    let mut names = all_characters().await.expect("query");
    names.sort();
    names
}

#[tokio::test]
async fn deactivates_everything_outside_the_given_list() {
    let _guard = seed().await;

    deactivate_characters(&["Alpha".to_string(), "Beta".to_string()])
        .await
        .expect("deactivate");

    assert_eq!(active().await, vec!["Alpha".to_string(), "Beta".to_string()]);

    // The rows are still there, just inactive.
    assert_eq!(common::count("SELECT count(*) FROM character").await, 3);
}

#[tokio::test]
async fn is_idempotent_and_leaves_unknown_names_alone() {
    let _guard = seed().await;

    let keep = vec!["Alpha".to_string(), "Never Existed".to_string()];
    deactivate_characters(&keep).await.expect("first");
    deactivate_characters(&keep).await.expect("second");

    assert_eq!(active().await, vec!["Alpha".to_string()]);
}

/// An empty list deactivates every character, which leaves the game with
/// nothing to pick from.
#[tokio::test]
async fn an_empty_list_deactivates_every_character() {
    let _guard = seed().await;

    deactivate_characters(&[]).await.expect("deactivate");

    assert!(active().await.is_empty());
    assert_eq!(common::count("SELECT count(*) FROM character").await, 3);
}

#[tokio::test]
async fn deactivating_does_not_reactivate_on_its_own() {
    let _guard = seed().await;

    deactivate_characters(&[]).await.expect("deactivate all");
    assert!(active().await.is_empty());

    deactivate_characters(&[
        "Alpha".to_string(),
        "Beta".to_string(),
        "Gamma".to_string(),
    ])
    .await
    .expect("re-list every character");

    assert!(
        active().await.is_empty(),
        "deactivate_characters only ever clears the flag"
    );
}

#[tokio::test]
async fn reactivate_brings_listed_characters_back() {
    let _guard = seed().await;
    deactivate_characters(&[]).await.expect("deactivate all");

    reactivate_characters(&["Alpha".to_string(), "Gamma".to_string()])
        .await
        .expect("reactivate");

    assert_eq!(active().await, vec!["Alpha".to_string(), "Gamma".to_string()]);
}

#[tokio::test]
async fn reactivate_is_idempotent_and_ignores_unknown_names() {
    let _guard = seed().await;
    deactivate_characters(&[]).await.expect("deactivate all");

    let names = vec!["Beta".to_string(), "Never Existed".to_string()];
    reactivate_characters(&names).await.expect("first");
    reactivate_characters(&names).await.expect("second");

    assert_eq!(active().await, vec!["Beta".to_string()]);
}

#[tokio::test]
async fn reactivating_an_empty_list_changes_nothing() {
    let _guard = seed().await;
    deactivate_characters(&["Alpha".to_string()]).await.expect("deactivate");

    reactivate_characters(&[]).await.expect("reactivate nothing");

    assert_eq!(active().await, vec!["Alpha".to_string()]);
}

/// The pair `sync_characters` calls, in its order. A character retired by an
/// earlier sync comes back once the wiki lists it again — `character_present`
/// still skips the re-insert, so the `active` flag is the only thing that can
/// bring it back.
async fn sync(listed: &[String]) {
    deactivate_characters(listed).await.expect("deactivate");
    reactivate_characters(listed).await.expect("reactivate");
}

#[tokio::test]
async fn the_sync_pair_restores_a_character_that_reappears() {
    let _guard = seed().await;
    let all = vec!["Alpha".to_string(), "Beta".to_string(), "Gamma".to_string()];

    sync(&[]).await;
    assert!(active().await.is_empty(), "an empty listing retires everyone");

    sync(&all).await;
    assert_eq!(active().await, all, "and a full listing brings them back");
}

#[tokio::test]
async fn the_sync_pair_retires_and_restores_in_one_pass() {
    let _guard = seed().await;

    sync(&["Alpha".to_string()]).await;
    assert_eq!(active().await, vec!["Alpha".to_string()]);

    // Alpha drops off the wiki in the same sync that Beta returns.
    sync(&["Beta".to_string()]).await;
    assert_eq!(active().await, vec!["Beta".to_string()]);
}

#[tokio::test]
async fn the_sync_pair_is_idempotent() {
    let _guard = seed().await;
    let listed = vec!["Alpha".to_string(), "Gamma".to_string()];

    sync(&listed).await;
    sync(&listed).await;

    assert_eq!(active().await, listed);
}
