//! End-to-end tests for `/guess`: the comparison logic that produces the
//! board's colours.
//!
//! Today's answer is pinned by writing the `answer` row directly, so every
//! test scores against a known character.

mod common;

use axum::http::StatusCode;
use common::http::{Client, auth};
use cosmeredle::{
    db::insert_character,
    server::{BinaryStatus, GuessResponse, SetStatus, TernaryStatus},
};
use serde_json::{Value, json};
use tokio::sync::OnceCell;

const ANSWER: &str = "Answer Hero";

static SEED: OnceCell<()> = OnceCell::const_new();

/// world, species, nation, nationality, ethnicity, introduced, abilities
type Fixture = (&'static str, [&'static str; 6], &'static [&'static str]);

const FIXTURES: &[Fixture] = &[
    (
        ANSWER,
        ["Roshar", "Human", "Alethkar", "Alethi", "Alethi", "The Way of Kings"],
        &["Windrunner", "Surgebinding"],
    ),
    // Same everything as the answer, different name.
    (
        "Twin Hero",
        ["Roshar", "Human", "Alethkar", "Alethi", "Alethi", "The Way of Kings"],
        &["Windrunner", "Surgebinding"],
    ),
    // Same world, species and book; different nation/nationality/ethnicity.
    (
        "Veden Human",
        ["Roshar", "Human", "Jah Keved", "Veden", "Veden", "The Way of Kings"],
        &["Lightweaver", "Surgebinding"],
    ),
    // Different world and book, same species, no shared abilities.
    (
        "Scadrian Human",
        ["Scadrial", "Human", "Central Dominance", "Scadrian", "Scadrian", "The Final Empire"],
        &["Allomancy"],
    ),
    // Different species, no abilities at all.
    (
        "Honorspren",
        ["Roshar", "Spren", "Alethkar", "Alethi", "Alethi", "The Way of Kings"],
        &[],
    ),
    // Strictly more abilities than the answer.
    (
        "Overpowered Hero",
        ["Roshar", "Human", "Alethkar", "Alethi", "Alethi", "The Way of Kings"],
        &["Windrunner", "Surgebinding", "Bondsmith"],
    ),
    // Introduced in a later book of the answer's series.
    (
        "Same Series Hero",
        ["Roshar", "Human", "Alethkar", "Alethi", "Alethi", "Words of Radiance"],
        &["Windrunner", "Surgebinding"],
    ),
    // Introduced in a book the `book` table has never heard of.
    (
        "Unlisted Book Hero",
        ["Roshar", "Human", "Alethkar", "Alethi", "Alethi", "An Unlisted Book"],
        &["Windrunner", "Surgebinding"],
    ),
];

/// title, series
const BOOKS: &[(&str, Option<&str>)] = &[
    ("The Way of Kings", Some("The Stormlight Archive")),
    ("Words of Radiance", Some("The Stormlight Archive")),
    ("The Final Empire", Some("Mistborn")),
];

async fn seed() {
    common::setup().await;
    SEED.get_or_init(|| async {
        for (name, [world, species, nation, nationality, ethnicity, introduced], abilities) in
            FIXTURES
        {
            let c = common::character(
                name,
                &[
                    ("universe", "Cosmere"),
                    ("world", world),
                    ("species", species),
                    ("nation", nation),
                    ("nationality", nationality),
                    ("ethnicity", ethnicity),
                    ("introduced", introduced),
                ],
                abilities,
            );
            insert_character(&c).await.unwrap_or_else(|e| panic!("insert {name}: {e}"));
        }

        for (title, series) in BOOKS {
            cosmeredle::db::insert_book(&cosmeredle::Book {
                title: (*title).to_string(),
                series: series.map(str::to_string),
            })
            .await
            .unwrap_or_else(|e| panic!("insert {title}: {e}"));
        }

        cosmeredle::db::store_answer(ANSWER, common::today())
            .await
            .expect("pin today's answer");
    })
    .await;
}

async fn guess(c: &mut Client, name: &str) -> Value {
    let res = c.post("/guess", &json!(name)).await;
    assert_eq!(res.status, StatusCode::OK, "guessing {name}: {}", res.body);
    res.json()
}

async fn anon_guess(name: &str) -> Value {
    seed().await;
    let mut c = Client::new().await;
    guess(&mut c, name).await
}

#[tokio::test]
async fn guessing_the_answer_scores_everything_correct() {
    let v = anon_guess(ANSWER).await;

    assert_eq!(v["name"], json!("Correct"));
    assert_eq!(v["world"], json!("Correct"));
    assert_eq!(v["book"], json!("Correct"));
    assert_eq!(v["species"], json!("Correct"));
    assert_eq!(v["abilities"], json!("Equal"));
}

#[tokio::test]
async fn a_different_character_with_identical_attributes_scores_only_the_name_wrong() {
    let v = anon_guess("Twin Hero").await;

    assert_eq!(v["name"], json!("Incorrect"));
    assert_eq!(v["world"], json!("Correct"));
    assert_eq!(v["book"], json!("Correct"));
    assert_eq!(v["species"], json!("Correct"));
    assert_eq!(v["abilities"], json!("Equal"));
}

#[tokio::test]
async fn the_same_species_from_elsewhere_is_adjacent() {
    let v = anon_guess("Veden Human").await;

    assert_eq!(
        v["species"],
        json!("Adjacent"),
        "same species, different nation/nationality/ethnicity"
    );
    assert_eq!(v["world"], json!("Correct"));
    assert_eq!(v["book"], json!("Correct"));
    assert_eq!(v["abilities"], json!("Overlap"));
}

#[tokio::test]
async fn a_different_world_and_book_score_incorrect() {
    let v = anon_guess("Scadrian Human").await;

    assert_eq!(v["world"], json!("Incorrect"));
    assert_eq!(v["book"], json!("Incorrect"));
    assert_eq!(v["species"], json!("Adjacent"));
    assert_eq!(v["abilities"], json!("Disjoint"));
}

#[tokio::test]
async fn a_different_species_scores_incorrect_regardless_of_origin() {
    let v = anon_guess("Honorspren").await;

    // Same nation, nationality and ethnicity as the answer, different species.
    assert_eq!(v["species"], json!("Incorrect"));
    assert_eq!(v["world"], json!("Correct"));
}

#[tokio::test]
async fn an_empty_ability_list_is_a_strict_subset_of_the_answers() {
    let v = anon_guess("Honorspren").await;

    // The statuses describe the answer relative to the guess: the answer has
    // every ability the guess has, and more.
    assert_eq!(v["abilities"], json!("Superset"));
}

#[tokio::test]
async fn guessing_more_abilities_than_the_answer_has_is_a_subset() {
    let v = anon_guess("Overpowered Hero").await;

    assert_eq!(v["abilities"], json!("Subset"));
}

#[tokio::test]
async fn the_response_describes_the_guess_and_never_the_answer() {
    let v = anon_guess("Scadrian Human").await;

    assert_eq!(v["character"]["name"], json!("Scadrian Human"));
    assert_eq!(v["character"]["world"], json!("Scadrial"));
    assert_eq!(v["character"]["species"], json!("Human"));
    assert_eq!(v["character"]["introduced"], json!("The Final Empire"));
    assert_eq!(v["character"]["abilities"], json!(["Allomancy"]));

    assert!(
        !v.to_string().contains(ANSWER),
        "the answer leaked into the response: {v}"
    );
}

#[tokio::test]
async fn the_response_exposes_only_the_wire_fields() {
    let v = anon_guess("Veden Human").await;
    let mut keys: Vec<&str> = v["character"]
        .as_object()
        .expect("character object")
        .keys()
        .map(String::as_str)
        .collect();
    keys.sort_unstable();

    assert_eq!(
        keys,
        [
            "abilities",
            "ethnicity",
            "introduced",
            "name",
            "nation",
            "nationality",
            "species",
            "world"
        ]
    );
}

#[tokio::test]
async fn guessing_an_unknown_character_is_a_bad_request() {
    seed().await;
    let mut c = Client::new().await;

    let res = c.post("/guess", &json!("No Such Character")).await;

    assert_eq!(res.status, StatusCode::BAD_REQUEST);
    assert_eq!(res.body, "Unknown character No Such Character");
}

/// The rejection echoes the submitted name, so it is served as plain text
/// rather than as a document.
#[tokio::test]
async fn the_unknown_character_message_is_not_served_as_html() {
    seed().await;
    let mut c = Client::new().await;

    let res = c.post("/guess", &json!("<b>markup</b>")).await;

    assert_eq!(res.status, StatusCode::BAD_REQUEST);
    assert_eq!(res.body, "Unknown character <b>markup</b>");
    assert_eq!(
        res.content_type.as_deref(),
        Some("text/plain; charset=utf-8")
    );
}

#[tokio::test]
async fn a_rejected_guess_is_not_recorded() {
    seed().await;
    let mut c = Client::new().await;
    c.post("/signup", &auth("rejected-guesser", "password123")).await;

    let res = c.post("/guess", &json!("Still Not A Character")).await;
    assert_eq!(res.status, StatusCode::BAD_REQUEST);

    assert_eq!(
        common::count("SELECT count(*) FROM guess WHERE guess = 'Still Not A Character'").await,
        0,
        "the guess is validated before it is persisted"
    );
    assert_eq!(c.get("/me").await.json()["guesses"], json!([]));
}

#[tokio::test]
async fn guessing_requires_a_json_string_body() {
    seed().await;
    let mut c = Client::new().await;

    let res = c.post("/guess", &json!({ "name": ANSWER })).await;
    assert_eq!(res.status, StatusCode::UNPROCESSABLE_ENTITY);
}

#[tokio::test]
async fn anonymous_guesses_are_answered_but_not_recorded() {
    seed().await;
    let mut c = Client::new().await;
    // A fixture no logged-in test guesses, so the count is unambiguous.
    guess(&mut c, "Scadrian Human").await;

    assert_eq!(
        common::count("SELECT count(*) FROM guess WHERE guess = 'Scadrian Human'").await,
        0,
        "an anonymous guess should not create a row"
    );
}

#[tokio::test]
async fn a_logged_in_guess_is_recorded_and_shows_up_in_me() {
    seed().await;
    let mut c = Client::new().await;
    c.post("/signup", &auth("guesser", "password123")).await;

    guess(&mut c, "Veden Human").await;
    guess(&mut c, "Honorspren").await;

    let mut guesses: Vec<String> =
        serde_json::from_value(c.get("/me").await.json()["guesses"].clone()).expect("guesses");
    guesses.sort();
    assert_eq!(guesses, ["Honorspren", "Veden Human"]);
}

/// The guess is recorded even when it is correct, and the handler ignores the
/// result of the insert: repeating a guess violates the `guess` primary key,
/// yet the request still succeeds.
#[tokio::test]
async fn repeating_a_guess_still_returns_200() {
    seed().await;
    let mut c = Client::new().await;
    c.post("/signup", &auth("repeater", "password123")).await;

    guess(&mut c, "Twin Hero").await;
    guess(&mut c, "Twin Hero").await;

    let id: i64 = common::count("SELECT id FROM user WHERE username = 'repeater'").await;
    assert_eq!(
        common::count(&format!(
            "SELECT count(*) FROM guess WHERE user_id = {id} AND guess = 'Twin Hero'"
        ))
        .await,
        1,
        "the duplicate insert failed silently"
    );
}

/// Nothing stops a player from guessing after they have already found the
/// answer, or from guessing an unlimited number of times.
#[tokio::test]
async fn guessing_continues_after_the_answer_has_been_found() {
    seed().await;
    let mut c = Client::new().await;
    c.post("/signup", &auth("persistent", "password123")).await;

    assert_eq!(guess(&mut c, ANSWER).await["name"], json!("Correct"));
    assert_eq!(guess(&mut c, "Twin Hero").await["name"], json!("Incorrect"));

    let id: i64 = common::count("SELECT id FROM user WHERE username = 'persistent'").await;
    assert_eq!(
        common::count(&format!("SELECT count(*) FROM guess WHERE user_id = {id}")).await,
        2
    );
}

#[tokio::test]
async fn guesses_of_inactive_characters_are_still_scored() {
    seed().await;
    // `/list` only offers active Cosmere characters, but `/guess` looks up any
    // row in the table.
    let c = common::character(
        "Retired Guessable",
        &[("universe", "Non-Cosmere"), ("world", "Roshar"), ("species", "Human")],
        &[],
    );
    insert_character(&c).await.expect("insert");

    let v = anon_guess("Retired Guessable").await;
    assert_eq!(v["world"], json!("Correct"));
    assert_eq!(v["character"]["name"], json!("Retired Guessable"));
}

#[tokio::test]
async fn the_response_deserialises_into_the_handler_s_own_types() {
    seed().await;
    let mut c = Client::new().await;
    let res = c.post("/guess", &json!("Veden Human")).await;

    let parsed: GuessResponse =
        serde_json::from_str(&res.body).expect("the wire format matches GuessResponse");

    assert_eq!(parsed.name, BinaryStatus::Incorrect);
    assert_eq!(parsed.world, BinaryStatus::Correct);
    assert_eq!(parsed.book, TernaryStatus::Correct);
    assert_eq!(parsed.species, TernaryStatus::Adjacent);
    assert_eq!(parsed.abilities, SetStatus::Overlap);
    assert_eq!(parsed.character.name, "Veden Human");
    assert_eq!(parsed.character.abilities, ["Lightweaver", "Surgebinding"]);
}

#[tokio::test]
async fn a_different_book_in_the_same_series_is_adjacent() {
    let v = anon_guess("Same Series Hero").await;
    assert_eq!(v["book"], json!("Adjacent"));
}

#[tokio::test]
async fn a_book_from_another_series_is_incorrect() {
    let v = anon_guess("Scadrian Human").await;
    assert_eq!(v["book"], json!("Incorrect"), "Mistborn is not Stormlight");
}

#[tokio::test]
async fn the_answers_own_book_is_correct_not_adjacent() {
    let v = anon_guess("Twin Hero").await;
    assert_eq!(v["book"], json!("Correct"));
}

/// A book the sync has never stored scores as `Incorrect` rather than
/// erroring, which is the state the app is actually in: `sync_books` has no
/// caller, so the `book` table is empty in production.
#[tokio::test]
async fn an_unknown_book_is_incorrect() {
    let v = anon_guess("Unlisted Book Hero").await;
    assert_eq!(v["book"], json!("Incorrect"));
}
