//! Unit tests for the wikitext parsers in `wiki`.
//!
//! These are pure functions, so nothing here touches the network or the
//! database. Several tests pin behaviour that is wrong on purpose — each is
//! marked BUG and will fail once the bug is fixed, which is the point.

use std::collections::HashMap;

use cosmeredle::wiki::{format, format_abilities, parse_table};


// ------------------------------------------------------------------- format
//
// Titlecasing is delegated to the `titlecase` crate; these cover how its
// output lands on the values that reach the database and the board.

#[test]
fn format_titlecases_plain_values() {
    assert_eq!(format("kaladin"), "Kaladin");
    assert_eq!(format("Kaladin"), "Kaladin");
    assert_eq!(format("kaladin stormblessed"), "Kaladin Stormblessed");
    assert_eq!(format("jah keved"), "Jah Keved");
}

#[test]
fn format_preserves_the_spacing_between_words() {
    assert_eq!(format("a\tb"), "A\tB");
    assert_eq!(format("double  space"), "Double  Space");
    assert_eq!(format("  pad  "), "Pad", "outer whitespace is still trimmed");
}

#[test]
fn format_applies_title_case_rules_to_small_words() {
    assert_eq!(format("the Way of Kings"), "The Way of Kings");
    assert_eq!(format("of the and"), "Of the And");
}

/// Worth knowing: unlike a per-word capitalisation, the `titlecase` crate
/// rewrites an all-caps value rather than leaving it alone.
#[test]
fn format_normalises_all_caps_but_leaves_mixed_case_alone() {
    assert_eq!(format("HUMAN"), "Human");
    assert_eq!(format("mcDONALD"), "mcDONALD");
    assert_eq!(format("iPhone"), "iPhone");
}

#[test]
fn format_handles_empty_and_non_alphabetic_input() {
    assert_eq!(format(""), "");
    assert_eq!(format("   "), "");
    assert_eq!(format("123 abc"), "123 Abc");
    assert_eq!(format("!!!"), "!!!");
}

#[test]
fn format_handles_multi_byte_characters() {
    assert_eq!(format("über"), "Über");
    assert_eq!(format("élan vital"), "Élan Vital");
    // One char can uppercase into two.
    assert_eq!(format("ßeta"), "SSeta");
}


#[test]
fn format_unwraps_a_plain_wiki_link() {
    assert_eq!(format("[[Roshar]]"), "Roshar");
    assert_eq!(format("[[Scadrial]]"), "Scadrial");
}

#[test]
fn format_keeps_the_display_half_of_a_piped_link() {
    assert_eq!(format("[[Kaladin|Kaladin]]"), "Kaladin");
    // BUG (titlecase): the space in the display text is lost.
    assert_eq!(
        format("[[Kaladin|Kaladin Stormblessed]]"),
        "Kaladin Stormblessed"
    );
}

#[test]
fn format_keeps_the_second_field_of_a_template() {
    assert_eq!(format("{{book ref|sa1|1}}"), "Sa1");
    assert_eq!(format("{{tag|value}}"), "Value");
}

#[test]
fn format_drops_small_tags_entirely() {
    assert_eq!(format("<small>presumed</small>"), "");
}

#[test]
fn format_drops_a_template_trailing_a_link() {
    // The regex swallows a `{{...}}` that immediately follows a link.
    assert_eq!(format("[[Vin]]{{book ref|mb1}}"), "Vin");
}

#[test]
fn format_passes_unmarked_text_through_titlecase() {
    assert_eq!(format("Human"), "Human");
    assert_eq!(format("human"), "Human");
    assert_eq!(format(""), "");
    assert_eq!(format("   "), "");
}

#[test]
fn format_handles_several_links_in_one_value() {
    assert_eq!(format("[[Alethi]] and [[Veden]]"), "Alethi and Veden");
}

// ---------------------------------------------------------- format_abilities

#[test]
fn format_abilities_splits_on_commas_and_strips_markup() {
    assert_eq!(
        format_abilities("[[Surgebinding]], [[Windrunner]]"),
        vec!["Surgebinding".to_string(), "Windrunner".to_string()]
    );
    assert_eq!(
        format_abilities("Allomancy,Feruchemy"),
        vec!["Allomancy".to_string(), "Feruchemy".to_string()]
    );
}

/// "others" is a placeholder on the wiki, not an ability. The `BAD` set is
/// matched against `format`'s output, so it has to hold the titlecased
/// spelling — whatever case the wiki used.
#[test]
fn format_abilities_drops_the_others_placeholder() {
    for v in ["others", "Others", "OTHERS", "[[others]]"] {
        assert!(format_abilities(v).is_empty(), "{v:?} should be dropped");
    }

    assert_eq!(
        format_abilities("Surgebinding, others"),
        vec!["Surgebinding".to_string()]
    );
}

/// Only the placeholder on its own is dropped; a real ability that merely
/// contains the word survives.
#[test]
fn format_abilities_keeps_abilities_that_only_mention_others() {
    assert_eq!(format_abilities("other"), vec!["Other".to_string()]);
    assert_eq!(
        format_abilities("others and more"),
        vec!["Others and More".to_string()]
    );
}

/// Empty entries would otherwise reach the `ability` table as empty-string
/// rows.
#[test]
fn format_abilities_drops_empty_entries() {
    for v in ["", "   ", ","] {
        assert!(format_abilities(v).is_empty(), "{v:?} should yield nothing");
    }

    assert_eq!(format_abilities("Allomancy,"), vec!["Allomancy".to_string()]);
    assert_eq!(
        format_abilities("Allomancy, , Feruchemy"),
        vec!["Allomancy".to_string(), "Feruchemy".to_string()]
    );
}

/// Duplicates survive, and `insert_character` then violates the
/// `(character, ability)` primary key: see tests/db_character.rs.
#[test]
fn format_abilities_keeps_duplicates() {
    assert_eq!(
        format_abilities("Surgebinding, Surgebinding"),
        vec!["Surgebinding".to_string(), "Surgebinding".to_string()]
    );
    // Each word is capitalised independently, so two spellings of one ability
    // become two distinct entries and never compare equal in `/guess`.
    assert_eq!(
        format_abilities("Soul casting, Soulcasting"),
        vec!["Soul Casting".to_string(), "Soulcasting".to_string()]
    );
}

// --------------------------------------------------------------- parse_table

const CHARACTER: &str = "\
Some intro prose.
{{character
|world=[[Roshar]]
|species=[[Human]]
|abilities=[[Surgebinding]], [[Windrunner]]
}}
Trailing prose.";

fn parsed(text: &str, table: &str) -> HashMap<String, String> {
    parse_table(text, table).unwrap_or_else(|| panic!("no {table} table in\n{text}"))
}

#[test]
fn parse_table_reads_the_character_infobox() {
    let map = parsed(CHARACTER, "character");

    assert_eq!(map.get("world").map(String::as_str), Some("[[Roshar]]"));
    assert_eq!(map.get("species").map(String::as_str), Some("[[Human]]"));
    assert_eq!(map.len(), 3, "{map:?}");
}

#[test]
fn parse_table_leaves_markup_for_format_to_handle() {
    // The values come back raw; `get_character` formats them afterwards.
    assert_eq!(parsed(CHARACTER, "character")["world"], "[[Roshar]]");
}

#[test]
fn parse_table_trims_keys_and_values() {
    let map = parsed("{{character\n|  world  =  [[Roshar]]  \n}}", "character");
    assert_eq!(map.get("world").map(String::as_str), Some("[[Roshar]]"));
}

#[test]
fn parse_table_spans_nested_templates() {
    let map = parsed(
        "{{character\n|born={{date|1173}}\n|world=Roshar\n}}",
        "character",
    );

    assert_eq!(map.get("born").map(String::as_str), Some("{{date|1173}}"));
    assert_eq!(
        map.get("world").map(String::as_str),
        Some("Roshar"),
        "a nested template must not end the table early"
    );
}

#[test]
fn parse_table_keeps_everything_after_the_first_equals() {
    let map = parsed("{{character\n|titles=a=b=c\n}}", "character");
    assert_eq!(map.get("titles").map(String::as_str), Some("a=b=c"));
}

#[test]
fn parse_table_ignores_lines_that_are_not_fields() {
    let map = parsed(
        "{{character\n|world=Roshar\nstray line\n|no equals sign\n|species=Human\n}}",
        "character",
    );

    assert_eq!(map.len(), 2, "{map:?}");
    assert!(map.contains_key("world") && map.contains_key("species"));
}

/// Values are read line by line, so a field wrapped across lines loses
/// everything after the first line.
#[test]
fn parse_table_truncates_multi_line_values() {
    let map = parsed(
        "{{character\n|titles=First line\ncontinued here\n}}",
        "character",
    );
    assert_eq!(map.get("titles").map(String::as_str), Some("First line"));
}

#[test]
fn parse_table_takes_the_last_of_a_repeated_key() {
    let map = parsed("{{character\n|world=First\n|world=Second\n}}", "character");
    assert_eq!(map.get("world").map(String::as_str), Some("Second"));
}

#[test]
fn parse_table_returns_none_when_the_table_is_absent() {
    assert!(parse_table("no templates here", "character").is_none());
    assert!(parse_table("{{other|x=y}}", "character").is_none());
}

#[test]
fn parse_table_returns_none_when_the_braces_never_close() {
    assert!(parse_table("{{character\n|world=Roshar\n", "character").is_none());
}

#[test]
fn parse_table_handles_an_empty_infobox() {
    assert_eq!(parsed("{{character}}", "character").len(), 0);
    assert_eq!(parsed("{{character\n}}", "character").len(), 0);
}

#[test]
fn parse_table_reads_a_named_infobox_other_than_character() {
    let book = "{{book\n|series=[[The Stormlight Archive]]\n|released=2010\n}}";
    let map = parsed(book, "book");

    assert_eq!(
        map.get("series").map(String::as_str),
        Some("[[The Stormlight Archive]]")
    );
    assert_eq!(map.len(), 2, "{map:?}");
}

/// The table name is matched as a prefix, not as a whole template name, so a
/// partial name still parses — the leftover characters land on a line with no
/// leading `|` and are dropped.
#[test]
fn parse_table_matches_a_table_name_by_prefix() {
    assert_eq!(
        parsed(CHARACTER, "char").get("world").map(String::as_str),
        Some("[[Roshar]]")
    );
    assert!(parse_table(CHARACTER, "c").is_some());
}

/// The search is a plain substring match, so an earlier template whose name
/// merely starts with the table name wins.
#[test]
fn parse_table_matches_the_first_template_with_a_matching_prefix() {
    let text = "{{characters list|a=b}}\n{{character\n|world=Roshar\n}}";

    // Starts at `{{characters list`, whose body does not begin with
    // `{{character` followed by the rest of that table, so the strip succeeds
    // on the wrong template and the real infobox is never reached.
    let map = parse_table(text, "character");
    assert_eq!(
        map.map(|m| m.len()),
        Some(0),
        "the wrong template was parsed"
    );
}
