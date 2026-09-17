use std::{fmt, io, result, str::Utf8Error};

use axum::{http::StatusCode, response::IntoResponse};
use axum_login::tower_sessions::session_store;
use bcrypt::BcryptError;
use mediawiki::MediaWikiError;
use serde::{Deserialize, Serialize};
use sqlx::{error::ErrorKind, types::Json};
use thiserror::Error;
use tokio::task::JoinError;
use tokio_cron_scheduler::JobSchedulerError;

pub mod answer;
pub mod backend;
pub mod db;
pub mod server;
pub mod wiki;

#[derive(Debug, Error)]
pub enum Error {
    #[error("Password encryption/decryption error: {0}")]
    Bcrypt(#[from] BcryptError),
    #[error("Deserialization error: {0}")]
    CborDe(#[from] ciborium::de::Error<io::Error>),
    #[error("Serialization error: {0}")]
    CborSer(#[from] ciborium::ser::Error<io::Error>),
    #[error("Formatting error: {0}")]
    Format(#[from] fmt::Error),
    #[error("I/O error: {0}")]
    Io(#[from] io::Error),
    #[error("Tokio join error: {0}")]
    Join(#[from] JoinError),
    #[error("Coppermind error: {0}")]
    MediaWiki(#[from] MediaWikiError),
    #[error("Scheduler error: {0}")]
    Sched(#[from] JobSchedulerError),
    #[error("Error storing sessions: {0}")]
    SessionStore(#[from] session_store::Error),
    #[error("Database error: {0}")]
    Sql(#[from] sqlx::Error),
    #[error("Invalid UTF-8 encountered")]
    Utf8(#[from] Utf8Error),

    #[error("{0}")]
    String(String),
    #[error("{0}")]
    User(String),
}

impl IntoResponse for Error {
    fn into_response(self) -> axum::response::Response {
        match self {
            Self::Sql(sqlx::Error::Database(d)) if d.kind() == ErrorKind::UniqueViolation => {
                return StatusCode::CONFLICT.into_response();
            }

            Self::Bcrypt(_)
            | Self::CborDe(_)
            | Self::CborSer(_)
            | Self::Format(_)
            | Self::Io(_)
            | Self::Join(_)
            | Self::MediaWiki(_)
            | Self::Sched(_)
            | Self::SessionStore(_)
            | Self::Sql(_)
            | Self::Utf8(_)
            | Self::String(_) => {
                println!("{self}");
                (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    "Odium's influence has blocked your request".to_string(),
                )
            }
            Self::User(s) => (StatusCode::BAD_REQUEST, s),
        }
        .into_response()
    }
}

#[macro_export]
macro_rules! err {
    ($s:literal $(,$args:expr)*) => {
        $crate::Error::String(format!($s, $($args),*))
    };
}

pub type Result<T> = result::Result<T, Error>;

#[derive(Debug, Default, Clone, Serialize, Deserialize)]
pub struct Character {
    name: String,
    unnamed: bool,
    parents: String,
    spouse: String,
    siblings: String,
    children: String,
    ancestors: String,
    relatives: String,
    descendants: String,
    born: String,
    died: String,
    abilities: Json<Vec<String>>,
    bonded: String,
    titles: String,
    aliases: String,
    skills: String,
    achievements: String,
    powers: String,
    hash_profession: String,
    profession: String,
    occupation: String,
    religion: String,
    groups: String,
    species: String,
    tick_species: String,
    era: String,
    birthplace: String,
    tick_birthplace: String,
    residence: String,
    tick_residence: String,
    ethnicity: String,
    tick_ethnicity: String,
    nation: String,
    tick_nation: String,
    nationality: String,
    world: String,
    tick_world: String,
    hide_world: bool,
    universe: String,
    introduced: String,
}

#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct Book {
    pub title: String,
    pub series: Option<String>,
}

impl Character {
    pub fn add(&mut self, k: &str, v: String) {
        match k {
            "unnamed" => self.unnamed = v == "y",
            "hide_world" => self.hide_world = v == "y",
            "parents" => self.parents = v,
            "spouse" => self.spouse = v,
            "siblings" => self.siblings = v,
            "children" => self.children = v,
            "ancestors" => self.ancestors = v,
            "relatives" => self.relatives = v,
            "descendants" => self.descendants = v,
            "born" => self.born = v,
            "died" => self.died = v,
            "bonded" => self.bonded = v,
            "titles" => self.titles = v,
            "aliases" => self.aliases = v,
            "skills" => self.skills = v,
            "achievements" => self.achievements = v,
            "powers" => self.powers = v,
            "#profession" => self.hash_profession = v,
            "profession" => self.profession = v,
            "occupation" => self.occupation = v,
            "religion" => self.religion = v,
            "groups" => self.groups = v,
            "species" => self.species = v,
            "'species" => self.tick_species = v,
            "era" => self.era = v,
            "birthplace" => self.birthplace = v,
            "'birthplace" => self.tick_birthplace = v,
            "residence" => self.residence = v,
            "'residence" => self.tick_residence = v,
            "ethnicity" => self.ethnicity = v,
            "'ethnicity" => self.tick_ethnicity = v,
            "nation" => self.nation = v,
            "'nation" => self.tick_nation = v,
            "nationality" => self.nationality = v,
            "world" | "earth" => self.world = v,
            "'world" => self.tick_world = v,
            "universe" => self.universe = v,
            "introduced" => self.introduced = v,
            _ => {}
        }
    }

    pub fn abilities(&mut self, v: Vec<String>) {
        self.abilities = Json(v);
    }

    pub fn name(&mut self, v: String) {
        self.name = v;
    }
}

pub async fn init() -> Result<()> {
    wiki::init().await?;
    db::init().await?;
    Ok(())
}
