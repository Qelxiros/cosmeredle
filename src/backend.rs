use std::{collections::HashSet, fs::read_to_string};

use axum::{Json, response::Html};
use itertools::Itertools;
use serde::{Deserialize, Serialize};

use crate::{Character, Result, answer::today, cache::CACHE, err};

macro_rules! user_err {
    ($s:literal $(,$args:expr)*) => {
        $crate::Error::User(format!($s, $($args),*))
    };
}

#[axum::debug_handler]
pub async fn home() -> Result<Html<String>> {
    Ok(Html(read_to_string("src/index.html")?))
}

#[derive(Debug, Clone, Serialize, Deserialize)]
enum BinaryStatus {
    Correct,
    Incorrect,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
enum TernaryStatus {
    Correct,
    Adjacent,
    Incorrect,
}

// "the answer is a ... of your guess"
#[derive(Debug, Clone, Serialize, Deserialize)]
enum SetStatus {
    Equal,
    Overlap,
    Subset,
    Superset,
    Disjoint,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct CharacterWire {
    name: String,
    world: String,
    introduced: String,
    species: String,
    nationality: String,
    nation: String,
    ethnicity: String,
    abilities: Vec<String>,
}

impl From<Character> for CharacterWire {
    fn from(value: Character) -> Self {
        Self {
            name: value.name,
            world: value.world,
            introduced: value.introduced,
            species: value.species,
            nationality: value.nationality,
            nation: value.nation,
            ethnicity: value.ethnicity,
            abilities: value.abilities,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GuessResponse {
    name: BinaryStatus,
    world: BinaryStatus,
    book: TernaryStatus,
    species: TernaryStatus,
    abilities: SetStatus,
    character: CharacterWire,
}

#[axum::debug_handler]
pub async fn handle_guess(guess: Json<String>) -> Result<Json<GuessResponse>> {
    let cache = CACHE.read().await;

    let guess = cache
        .get(&*guess)
        .ok_or(user_err!("unrecognized character"))?;

    let answer = cache
        .get(&today().await)
        .ok_or(err!("Missing answer details"))?;

    let out = GuessResponse {
        name: if guess.name == answer.name {
            BinaryStatus::Correct
        } else {
            BinaryStatus::Incorrect
        },
        world: if guess.world == answer.world {
            BinaryStatus::Correct
        } else {
            BinaryStatus::Incorrect
        },
        book: if guess.introduced == answer.introduced {
            TernaryStatus::Correct
        } else {
            TernaryStatus::Incorrect
        },
        species: if guess.species == answer.species {
            if guess.nation == answer.nation
                && guess.nationality == answer.nationality
                && guess.ethnicity == answer.ethnicity
            {
                TernaryStatus::Correct
            } else {
                TernaryStatus::Adjacent
            }
        } else {
            TernaryStatus::Incorrect
        },
        abilities: {
            let ga = guess.abilities.iter().collect::<HashSet<_>>();
            let aa = answer.abilities.iter().collect::<HashSet<_>>();

            match (ga.is_subset(&aa), aa.is_subset(&ga)) {
                (true, true) => SetStatus::Equal,
                (true, false) => SetStatus::Superset,
                (false, true) => SetStatus::Subset,
                (false, false) => {
                    if ga.intersection(&aa).next().is_some() {
                        SetStatus::Overlap
                    } else {
                        SetStatus::Disjoint
                    }
                }
            }
        },
        character: guess.clone().into(),
    };

    Ok(Json(out))
}

pub async fn handle_list() -> Json<Vec<String>> {
    Json(
        CACHE
            .read()
            .await
            .iter()
            .filter(|(_, v)| v.universe == "Cosmere")
            .map(|(k, _)| k)
            .cloned()
            .sorted_unstable()
            .collect_vec(),
    )
}
