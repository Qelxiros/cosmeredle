//! Unit tests for `Character`, the wikitext -> struct mapping layer.
//!
//! `Character`'s fields are private and it has no getters, so these tests
//! observe it through its `Serialize` impl.

use cosmeredle::Character;
use serde_json::{Value, json};

fn value(c: &Character) -> Value {
    serde_json::to_value(c).expect("Character serializes")
}

fn field(c: &Character, name: &str) -> Value {
    value(c)
        .get(name)
        .unwrap_or_else(|| panic!("no field {name}"))
        .clone()
}

fn with(key: &str, v: &str) -> Character {
    let mut c = Character::default();
    c.add(key, v.to_string());
    c
}

/// Every string-valued key `add` accepts, and the field it must land in.
const STRING_KEYS: &[(&str, &str)] = &[
    ("parents", "parents"),
    ("spouse", "spouse"),
    ("siblings", "siblings"),
    ("children", "children"),
    ("ancestors", "ancestors"),
    ("relatives", "relatives"),
    ("descendants", "descendants"),
    ("born", "born"),
    ("died", "died"),
    ("bonded", "bonded"),
    ("titles", "titles"),
    ("aliases", "aliases"),
    ("skills", "skills"),
    ("achievements", "achievements"),
    ("powers", "powers"),
    ("#profession", "hash_profession"),
    ("profession", "profession"),
    ("occupation", "occupation"),
    ("religion", "religion"),
    ("groups", "groups"),
    ("species", "species"),
    ("'species", "tick_species"),
    ("era", "era"),
    ("birthplace", "birthplace"),
    ("'birthplace", "tick_birthplace"),
    ("residence", "residence"),
    ("'residence", "tick_residence"),
    ("ethnicity", "ethnicity"),
    ("'ethnicity", "tick_ethnicity"),
    ("nation", "nation"),
    ("'nation", "tick_nation"),
    ("nationality", "nationality"),
    ("world", "world"),
    ("earth", "world"),
    ("'world", "tick_world"),
    ("universe", "universe"),
    ("introduced", "introduced"),
];

#[test]
fn default_is_entirely_empty() {
    let v = value(&Character::default());
    let obj = v.as_object().expect("object");

    for (k, val) in obj {
        match k.as_str() {
            "unnamed" | "hide_world" => assert_eq!(val, &json!(false), "{k} should default false"),
            "abilities" => assert_eq!(val, &json!([]), "abilities should default empty"),
            _ => assert_eq!(val, &json!(""), "{k} should default to the empty string"),
        }
    }
}

#[test]
fn every_key_lands_in_its_own_field() {
    for (key, expected_field) in STRING_KEYS {
        let c = with(key, "sentinel");
        assert_eq!(
            field(&c, expected_field),
            json!("sentinel"),
            "add({key:?}) should set {expected_field}"
        );

        // and must not have touched anything else
        let baseline = value(&Character::default());
        let actual = value(&c);
        for (name, val) in actual.as_object().expect("object") {
            if name == expected_field {
                continue;
            }
            assert_eq!(
                val, &baseline[name],
                "add({key:?}) unexpectedly changed {name}"
            );
        }
    }
}

#[test]
fn world_and_earth_are_aliases() {
    let mut c = Character::default();
    c.add("world", "Roshar".into());
    assert_eq!(field(&c, "world"), json!("Roshar"));

    c.add("earth", "Scadrial".into());
    assert_eq!(
        field(&c, "world"),
        json!("Scadrial"),
        "`earth` should overwrite the same field as `world`"
    );
}

#[test]
fn later_adds_overwrite_earlier_ones() {
    let mut c = Character::default();
    c.add("species", "Human".into());
    c.add("species", "Singer".into());
    assert_eq!(field(&c, "species"), json!("Singer"));
}

#[test]
fn boolean_keys_accept_exactly_y() {
    for key in ["unnamed", "hide_world"] {
        assert_eq!(field(&with(key, "y"), key), json!(true), "{key}=y");

        // Everything else is false, including spellings the wiki plausibly uses.
        for v in ["n", "no", "", "Y", "yes", "true", " y", "y "] {
            assert_eq!(
                field(&with(key, v), key),
                json!(false),
                "{key}={v:?} should be false"
            );
        }
    }
}

#[test]
fn unknown_keys_are_silently_dropped() {
    for key in ["", "  ", "nonsense", "WORLD", "Species", "abilities", "name"] {
        let c = with(key, "ignored");
        assert_eq!(
            value(&c),
            value(&Character::default()),
            "add({key:?}) should have been a no-op"
        );
    }
}

#[test]
fn add_does_not_handle_abilities_or_name() {
    // Callers must use the dedicated setters; `add` drops these on the floor.
    let mut c = Character::default();
    c.add("abilities", "Windrunner".into());
    c.add("name", "Kaladin".into());

    assert_eq!(field(&c, "abilities"), json!([]));
    assert_eq!(field(&c, "name"), json!(""));
}

#[test]
fn abilities_and_name_setters() {
    let mut c = Character::default();
    c.name("Kaladin".into());
    c.abilities(vec!["Windrunner".into(), "Surgebinding".into()]);

    assert_eq!(field(&c, "name"), json!("Kaladin"));
    assert_eq!(field(&c, "abilities"), json!(["Windrunner", "Surgebinding"]));

    // abilities() replaces rather than appends
    c.abilities(vec!["Bondsmith".into()]);
    assert_eq!(field(&c, "abilities"), json!(["Bondsmith"]));

    c.abilities(vec![]);
    assert_eq!(field(&c, "abilities"), json!([]));
}

#[test]
fn values_are_stored_verbatim() {
    // `add` does no trimming, casing or validation of its own.
    let long = "very ".repeat(200);
    for v in ["  padded  ", "", "Ünïcödé", "a\nb", long.as_str()] {
        assert_eq!(field(&with("titles", v), "titles"), json!(v));
    }
}

#[test]
fn duplicate_abilities_are_preserved_by_the_setter() {
    // Relevant because the `ability` table has a (character, ability) primary
    // key: see tests/db_character.rs.
    let mut c = Character::default();
    c.abilities(vec!["Surgebinding".into(), "Surgebinding".into()]);
    assert_eq!(field(&c, "abilities"), json!(["Surgebinding", "Surgebinding"]));
}
