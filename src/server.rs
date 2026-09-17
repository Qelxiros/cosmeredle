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
    Character, Result,
    answer::today,
    backend::AuthSession,
    db::{self, DATE_FORMAT, all_characters, get_book, get_character, get_guesses},
    err,
};

/// The page served at `/`, relative to the working directory.
pub const INDEX_PATH: &str = "src/index.html";
/// bcrypt work factor used when hashing a new account's password.
pub const BCRYPT_COST: u32 = 12;

macro_rules! user_err {
    ($s:literal $(,$args:expr)*) => {
        $crate::Error::User(format!($s, $($args),*))
    };
}

pub fn init() -> Result<()> {
    Ok(())
}

pub async fn home() -> Result<Html<String>> {
    Ok(Html(read_to_string(INDEX_PATH)?))
}

pub async fn day() -> Result<String> {
    let mut s = String::new();
    Local::now()
        .date_naive()
        .format(DATE_FORMAT)
        .write_to(&mut s)?;
    Ok(s)
}

#[derive(Debug, Serialize, Deserialize)]
pub struct User {
    pub username: String,
    pub guesses: Vec<String>,
}

pub async fn me(auth_session: AuthSession) -> Result<Json<User>> {
    let user = auth_session
        .user
        .ok_or(err!("reached /me handler without logging in"))?;

    Ok(Json(User {
        username: user.username,
        guesses: get_guesses(user.id).await?,
    }))
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum BinaryStatus {
    Correct,
    Incorrect,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum TernaryStatus {
    Correct,
    Adjacent,
    Incorrect,
}

// "the answer is a ... of your guess"
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum SetStatus {
    Equal,
    Overlap,
    Subset,
    Superset,
    Disjoint,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CharacterWire {
    pub name: String,
    pub world: String,
    pub introduced: String,
    pub species: String,
    pub nationality: String,
    pub nation: String,
    pub ethnicity: String,
    pub abilities: Vec<String>,
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
    pub name: BinaryStatus,
    pub world: BinaryStatus,
    pub book: TernaryStatus,
    pub species: TernaryStatus,
    pub abilities: SetStatus,
    pub character: CharacterWire,
}

pub async fn handle_guess(
    auth_session: AuthSession,
    Json(guess_str): Json<String>,
) -> Result<Json<GuessResponse>> {
    let guess = get_character(&guess_str)
        .await
        .map_err(|_| user_err!("Unknown character {guess_str}"))?;
    let answer = get_character(&today().await?).await?;

    let db_job = if let Some(user) = auth_session.user {
        let g = guess_str.clone();
        Some(task::spawn(db::insert_guess(user.id, g)))
    } else {
        None
    };

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
            let abook = get_book(&answer.introduced).await?;
            let gbook = get_book(&guess.introduced).await?;
            match (abook.and_then(|a| a.series), gbook.and_then(|g| g.series)) {
                (Some(a), Some(g)) if a == g => TernaryStatus::Adjacent,
                _ => TernaryStatus::Incorrect,
            }
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
        let _: Result<()> = job.await?;
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

pub async fn handle_signup(
    auth_session: AuthSession,
    Json(auth): Json<Auth>,
) -> Result<Response<Body>> {
    if auth.username.len() < 3 {
        return Err(user_err!("Username too short"));
    }
    if auth.password.len() < 8 {
        return Err(user_err!("Password too short"));
    }

    let a = auth.clone();
    let bcrypt = task::spawn_blocking(|| hash(a.password, BCRYPT_COST)).await??;
    db::insert_user(a.username, bcrypt).await?;

    Ok(handle_login(auth_session, Json(auth)).await)
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
