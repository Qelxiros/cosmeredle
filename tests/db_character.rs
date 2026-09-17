//! Tests for character storage: `insert_character`, `get_character`,
//! `character_present` and `all_characters`.

mod common;

use cosmeredle::db::{all_characters, character_present, get_character, insert_character};
use serde_json::{Value, json};

fn value(c: &cosmeredle::Character) -> Value {
    serde_json::to_value(c).expect("serialize")
}

#[tokio::test]
async fn every_field_round_trips_through_the_database() {
    common::setup().await;

    // One distinct value per column, so a mis-ordered INSERT would be visible.
    let fields: Vec<(&str, String)> = [
        "parents", "spouse", "siblings", "children", "ancestors", "relatives", "descendants",
        "born", "died", "bonded", "titles", "aliases", "skills", "achievements", "powers",
        "#profession", "profession", "occupation", "religion", "groups", "species", "'species",
        "era", "birthplace", "'birthplace", "residence", "'residence", "ethnicity", "'ethnicity",
        "nation", "'nation", "nationality", "world", "'world", "introduced",
    ]
    .iter()
    .enumerate()
    .map(|(i, k)| (*k, format!("value-{i}")))
    .collect();

    let mut pairs: Vec<(&str, &str)> = fields.iter().map(|(k, v)| (*k, v.as_str())).collect();
    pairs.push(("universe", "Cosmere"));
    pairs.push(("unnamed", "y"));
    pairs.push(("hide_world", "y"));

    let original = common::character("Round Trip", &pairs, &["Allomancy", "Feruchemy"]);
    insert_character(&original).await.expect("insert");

    let fetched = get_character("Round Trip").await.expect("fetch");
    assert_eq!(value(&fetched), value(&original), "round trip lost or moved a field");
}

#[tokio::test]
async fn booleans_round_trip() {
    common::setup().await;
    let c = common::character(
        "Bool False",
        &[("universe", "Cosmere"), ("unnamed", "n"), ("hide_world", "n")],
        &[],
    );
    insert_character(&c).await.expect("insert");

    let v = value(&get_character("Bool False").await.expect("fetch"));
    assert_eq!(v["unnamed"], json!(false));
    assert_eq!(v["hide_world"], json!(false));
}

#[tokio::test]
async fn a_character_with_no_abilities_comes_back_with_an_empty_list() {
    common::setup().await;
    common::insert_cosmere("No Abilities", "Roshar", "Human", "The Way of Kings", &[]).await;

    let c = get_character("No Abilities").await.expect("fetch");
    assert_eq!(value(&c)["abilities"], json!([]));
}

#[tokio::test]
async fn abilities_round_trip_as_a_set() {
    common::setup().await;
    common::insert_cosmere(
        "Many Abilities",
        "Scadrial",
        "Human",
        "The Final Empire",
        &["Allomancy", "Feruchemy", "Hemalurgy"],
    )
    .await;

    let c = get_character("Many Abilities").await.expect("fetch");
    let mut abilities: Vec<String> =
        serde_json::from_value(value(&c)["abilities"].clone()).expect("abilities");
    abilities.sort();
    assert_eq!(abilities, ["Allomancy", "Feruchemy", "Hemalurgy"]);
}

/// The `ability` table's primary key is `(character, ability)`, so a character
/// the wiki lists a duplicate ability for cannot be stored at all.
#[tokio::test]
async fn a_character_with_a_duplicate_ability_cannot_be_inserted() {
    common::setup().await;
    let c = common::cosmere(
        "Duplicate Ability",
        "Roshar",
        "Human",
        "The Way of Kings",
        &["Surgebinding", "Surgebinding"],
    );

    let err = insert_character(&c).await.expect_err("duplicate ability");
    assert!(err.to_string().contains("UNIQUE"), "{err}");

    // the transaction rolled the character row back too
    assert!(!character_present("Duplicate Ability").await);
    assert_eq!(
        common::count("SELECT count(*) FROM ability WHERE character = 'Duplicate Ability'").await,
        0
    );
}

#[tokio::test]
async fn two_characters_may_share_an_ability() {
    common::setup().await;
    common::insert_cosmere("Sharer One", "Roshar", "Human", "Oathbringer", &["Surgebinding"]).await;
    common::insert_cosmere("Sharer Two", "Roshar", "Human", "Oathbringer", &["Surgebinding"]).await;

    assert!(character_present("Sharer One").await);
    assert!(character_present("Sharer Two").await);
}

#[tokio::test]
async fn inserting_the_same_name_twice_is_a_conflict_and_rolls_back() {
    common::setup().await;
    common::insert_cosmere("Twice", "Roshar", "Human", "Oathbringer", &["First"]).await;

    let again = common::cosmere("Twice", "Scadrial", "Kandra", "Mistborn", &["Second"]);
    let err = insert_character(&again).await.expect_err("duplicate name");
    assert!(err.to_string().contains("UNIQUE"), "{err}");

    // the original survives untouched and the new abilities were not written
    let c = value(&get_character("Twice").await.expect("fetch"));
    assert_eq!(c["world"], json!("Roshar"));
    assert_eq!(c["abilities"], json!(["First"]));
}

#[tokio::test]
async fn get_character_errors_when_the_name_is_unknown() {
    common::setup().await;
    let err = get_character("Definitely Not A Character")
        .await
        .expect_err("missing character");
    assert!(
        matches!(err, cosmeredle::Error::Sql(sqlx::Error::RowNotFound)),
        "{err}"
    );
}

#[tokio::test]
async fn character_lookup_is_exact() {
    common::setup().await;
    common::insert_cosmere("Exact Match", "Roshar", "Human", "Oathbringer", &[]).await;

    assert!(character_present("Exact Match").await);
    assert!(!character_present("exact match").await);
    assert!(!character_present("Exact").await);
    assert!(!character_present("Exact Match ").await);
}

#[tokio::test]
async fn character_present_is_false_for_unknown_names() {
    common::setup().await;
    assert!(!character_present("Nobody Here").await);
    assert!(!character_present("").await);
}

#[tokio::test]
async fn all_characters_lists_only_active_cosmere_characters() {
    common::setup().await;
    common::insert_cosmere("List Active", "Roshar", "Human", "Oathbringer", &[]).await;

    // `insert_character` derives `active` from `universe == "Cosmere"`.
    let non_cosmere = common::character(
        "List NonCosmere",
        &[("universe", "Non-Cosmere"), ("world", "Earth")],
        &[],
    );
    insert_character(&non_cosmere).await.expect("insert");

    common::insert_cosmere("List Inactive", "Roshar", "Human", "Oathbringer", &[]).await;
    common::exec("UPDATE character SET active = false WHERE name = 'List Inactive'").await;

    let names = all_characters().await.expect("query");
    assert!(names.contains(&"List Active".to_string()));
    assert!(
        !names.contains(&"List NonCosmere".to_string()),
        "non-Cosmere characters are never playable"
    );
    assert!(!names.contains(&"List Inactive".to_string()));
}

#[tokio::test]
async fn universe_must_match_cosmere_exactly_to_be_playable() {
    common::setup().await;
    for (name, universe) in [
        ("Case Lower", "cosmere"),
        ("Case Spaced", " Cosmere"),
        ("Case Other", "Cosmere-adjacent"),
    ] {
        let c = common::character(name, &[("universe", universe), ("world", "Roshar")], &[]);
        insert_character(&c).await.expect("insert");

        let active = common::count(&format!(
            "SELECT count(*) FROM character WHERE name = '{name}' AND active"
        ))
        .await;
        assert_eq!(active, 0, "{universe:?} should not have been marked active");
    }
}
