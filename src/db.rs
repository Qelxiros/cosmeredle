use std::{fs::File, sync::OnceLock};

use axum_login::UserId;
use chrono::Local;
use derive_debug::Dbg;
use sqlx::{SqlitePool, migrate, query, query_as, types::Json};

use crate::{Result, backend::Backend};

static CONN: OnceLock<SqlitePool> = OnceLock::new();

pub async fn init() -> Result<()> {
    let _ = File::create_new("storage/sqlite.db");
    let pool = SqlitePool::connect("storage/sqlite.db").await?;
    migrate!().run(&pool).await.map_err(sqlx::Error::from)?;

    CONN.get_or_init(|| pool);

    Ok(())
}

pub fn conn() -> &'static SqlitePool {
    CONN.get().unwrap()
}

#[derive(Dbg, Clone)]
pub struct User {
    pub id: i64,
    pub username: String,
    #[dbg(skip)]
    pub bcrypt: String,
    pub guesses: Json<Vec<String>>,
}

pub async fn insert_user(username: String, bcrypt: String) -> Result<()> {
    query!(
        "INSERT INTO user (username, bcrypt) VALUES ($1, $2)",
        username,
        bcrypt
    )
    .execute(conn())
    .await?;

    Ok(())
}

pub async fn get_user(id: UserId<Backend>) -> Result<Option<User>> {
    Ok(query_as!(
        User,
        r#"SELECT id, username, bcrypt, json_group_array(guess) AS "guesses!: Json<Vec<String>>" FROM user INNER JOIN guess ON user.id = guess.user_id WHERE id = $1 GROUP BY id, username, bcrypt"#,
        id
    )
    .fetch_optional(conn())
    .await?)
}

pub async fn get_user_by_username(username: &str) -> Result<Option<User>> {
    Ok(query_as!(
        User,
        r#"SELECT id, username, bcrypt, json_group_array(guess) AS "guesses!: Json<Vec<String>>" FROM user INNER JOIN guess ON user.id = guess.user_id WHERE username = $1 GROUP BY id, username, bcrypt"#,
        username
    )
    .fetch_optional(conn())
    .await?)
}

pub async fn insert_guess(user_id: UserId<Backend>, guess: String) -> Result<()> {
    let mut day = String::new();
    Local::now()
        .date_naive()
        .format("%Y-%m-%d")
        .write_to(&mut day)?;
    query!(
        "INSERT INTO guess (user_id, guess, day) VALUES ($1, $2, $3)",
        user_id,
        guess,
        day,
    )
    .execute(conn())
    .await?;

    Ok(())
}

pub async fn get_guesses(user_id: UserId<Backend>) -> Result<Vec<String>> {
    let mut day = String::new();
    Local::now()
        .date_naive()
        .format("%Y-%m-%d")
        .write_to(&mut day)?;

    Ok(query!(
        "SELECT guess FROM guess WHERE user_id = $1 AND day = $2",
        user_id,
        day
    )
    .fetch_all(conn())
    .await
    .map(|v| v.into_iter().map(|r| r.guess).collect())?)
}
