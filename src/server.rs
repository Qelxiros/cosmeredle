use std::{collections::HashSet, fs::read_to_string};

use axum::{
    Json,
    body::Body,
    http::{Response, StatusCode},
    response::{Html, IntoResponse},
};
use bcrypt::hash;
use chrono::Local;
use derive_debug::Dbg;
use serde::{Deserialize, Serialize};
use tokio::task;

use crate::{
    Character, Error, Result,
    answer::today,
    backend::AuthSession,
    db::{self, all_characters, get_character},
};

pub fn init() -> Result<()> {
    Ok(())
}

macro_rules! user_err {
    ($s:literal $(,$args:expr)*) => {
        $crate::Error::User(format!($s, $($args),*))
    };
}

pub async fn home() -> Result<Html<String>> {
    Ok(Html(read_to_string("src/index.html")?))
}

pub async fn day() -> Result<String> {
    let mut s = String::new();
    Local::now()
        .date_naive()
        .format("%Y-%m-%d")
        .write_to(&mut s)?;
    Ok(s)
}

pub async fn me(auth_session: AuthSession) -> impl IntoResponse {
    match auth_session.user {
        Some(user) => (StatusCode::OK, Json(user.username)).into_response(),
        None => StatusCode::UNAUTHORIZED.into_response(),
    }
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
            abilities: value.abilities.0,
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

pub async fn handle_guess(
    auth_session: AuthSession,
    Json(guess): Json<String>,
) -> Result<Json<GuessResponse>> {
    let db_job = if let Some(user) = auth_session.user {
        if user.guesses.0.contains(&guess) {
            return Err(user_err!("Already guessed!"));
        }
        let g = guess.clone();
        Some(task::spawn(db::insert_guess(user.id, g)))
    } else {
        None
    };

    let guess = get_character(&guess).await?;

    let answer = get_character(&today().await?).await?;

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

    if let Some(job) = db_job {
        job.await??
    }

    Ok(Json(out))
}

pub async fn handle_list() -> Result<Json<Vec<String>>> {
    Ok(Json(all_characters().await?))
}

#[derive(Dbg, Clone, Serialize, Deserialize)]
pub struct Auth {
    pub username: String,
    #[dbg(skip)]
    pub password: String,
}

pub async fn handle_signup(auth_session: AuthSession, Json(auth): Json<Auth>) -> impl IntoResponse {
    let a = auth.clone();
    let bcrypt = match hash(a.password, 12) {
        Ok(b) => b,
        Err(_) => return StatusCode::INTERNAL_SERVER_ERROR.into_response(),
    };
    match db::insert_user(a.username, bcrypt).await {
        Err(Error::Sql(sqlx::Error::Database(_))) => return StatusCode::CONFLICT.into_response(),
        Err(_) => return StatusCode::INTERNAL_SERVER_ERROR.into_response(),
        Ok(()) => {}
    }

    handle_login(auth_session, Json(auth)).await
}

pub async fn handle_login(mut auth_session: AuthSession, Json(auth): Json<Auth>) -> Response<Body> {
    let user = match auth_session.authenticate(auth).await {
        Ok(Some(user)) => user,
        Ok(None) => {
            return (StatusCode::UNAUTHORIZED, "incorrect username or password").into_response();
        }
        Err(_) => return StatusCode::INTERNAL_SERVER_ERROR.into_response(),
    };

    if auth_session.login(&user).await.is_err() {
        StatusCode::INTERNAL_SERVER_ERROR.into_response()
    } else {
        (StatusCode::OK, user.username).into_response()
    }
}

pub async fn handle_logout(mut auth_session: AuthSession) -> impl IntoResponse {
    match auth_session.logout().await {
        Ok(_) => StatusCode::OK,
        Err(_) => StatusCode::INTERNAL_SERVER_ERROR,
    }
}
