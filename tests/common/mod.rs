//! Shared harness for the integration tests.
//!
//! Every test binary gets its own sandbox directory and its own database, via
//! `db::init_at`, so the developer's real `storage/sqlite.db` is never touched.
//! The binary also chdirs into the sandbox, because `server::INDEX_PATH` is
//! resolved relative to the working directory. `migrate!()` embeds the
//! migrations at compile time, so they still resolve after the chdir.
#![allow(dead_code)]

use std::{
    fs,
    path::{Path, PathBuf},
    sync::Once,
};

use chrono::{Local, NaiveDate};
use cosmeredle::{Character, db, server};
use sqlx::SqlitePool;
use tokio::sync::{Mutex, MutexGuard, OnceCell};

pub mod http;

static CHDIR: Once = Once::new();
/// Guards tables that are process-global state rather than per-test rows (the
/// single-row `answer` table, whole-table character updates).
static SERIAL: Mutex<()> = Mutex::const_new(());
static DB: OnceCell<()> = OnceCell::const_new();

/// A directory unique to this test binary, under `target/`.
fn sandbox() -> PathBuf {
    let exe = std::env::current_exe().expect("current_exe");
    let name = exe.file_name().expect("exe name").to_string_lossy().to_string();

    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("target")
        .join("test-sandbox")
        .join(name)
}

/// Creates (once per process) a sandbox CWD and an empty, migrated database.
pub async fn setup() -> &'static SqlitePool {
    DB.get_or_init(|| async {
        CHDIR.call_once(|| {
            let dir = sandbox();
            let _ = fs::remove_dir_all(&dir);
            for relative in [db::DB_PATH, server::INDEX_PATH] {
                let parent = Path::new(relative).parent().expect("a parent directory");
                fs::create_dir_all(dir.join(parent)).expect("create sandbox directory");
            }

            // `server::home()` reads INDEX_PATH relative to the CWD.
            let manifest = Path::new(env!("CARGO_MANIFEST_DIR"));
            fs::copy(manifest.join(server::INDEX_PATH), dir.join(server::INDEX_PATH))
                .expect("copy the index page");

            std::env::set_current_dir(&dir).expect("chdir to sandbox");
        });

        let db = sandbox().join(db::DB_PATH);
        db::init_at(&db.to_string_lossy())
            .await
            .expect("db::init_at");
    })
    .await;

    db::conn()
}

/// Serializes tests that rewrite a shared table.
pub async fn serial() -> MutexGuard<'static, ()> {
    SERIAL.lock().await
}

/// Builds a `Character` through the only public constructor path there is:
/// `Default` + `add` + `abilities` + `name`.
pub fn character(name: &str, fields: &[(&str, &str)], abilities: &[&str]) -> Character {
    let mut c = Character::default();
    for (k, v) in fields {
        c.add(k, (*v).to_string());
    }
    c.abilities(abilities.iter().map(|s| (*s).to_string()).collect());
    c.name(name.to_string());
    c
}

/// A Cosmere character (so `insert_character` marks it active) with sensible
/// defaults for the fields the game actually compares.
pub fn cosmere(
    name: &str,
    world: &str,
    species: &str,
    introduced: &str,
    abilities: &[&str],
) -> Character {
    character(
        name,
        &[
            ("universe", "Cosmere"),
            ("world", world),
            ("species", species),
            ("introduced", introduced),
            ("nation", "Alethkar"),
            ("nationality", "Alethi"),
            ("ethnicity", "Alethi"),
        ],
        abilities,
    )
}

pub async fn insert_cosmere(
    name: &str,
    world: &str,
    species: &str,
    introduced: &str,
    abilities: &[&str],
) {
    db::insert_character(&cosmere(name, world, species, introduced, abilities))
        .await
        .unwrap_or_else(|e| panic!("insert {name}: {e}"));
}

pub fn today() -> NaiveDate {
    Local::now().date_naive()
}

pub fn today_str() -> String {
    today().format(db::DATE_FORMAT).to_string()
}

pub fn days_ago(n: i64) -> NaiveDate {
    today() - chrono::Duration::days(n)
}

pub fn date_str(d: NaiveDate) -> String {
    d.format(db::DATE_FORMAT).to_string()
}

/// Raw statement, for setting up states the public API cannot reach.
pub async fn exec(sql: &str) {
    sqlx::query(sql)
        .execute(db::conn())
        .await
        .unwrap_or_else(|e| panic!("exec {sql}: {e}"));
}

pub async fn count(sql: &str) -> i64 {
    sqlx::query_scalar(sql)
        .fetch_one(db::conn())
        .await
        .unwrap_or_else(|e| panic!("count {sql}: {e}"))
}

pub async fn scalar(sql: &str) -> Option<String> {
    sqlx::query_scalar(sql)
        .fetch_optional(db::conn())
        .await
        .unwrap_or_else(|e| panic!("scalar {sql}: {e}"))
}
