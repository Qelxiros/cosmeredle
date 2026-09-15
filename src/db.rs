use std::{fs::File, sync::OnceLock};

use derive_debug::Dbg;
use sqlx::{SqlitePool, migrate, query, query_as};

use crate::Result;

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
    pub username: String,
    #[dbg(skip)]
    pub bcrypt: String,
}

pub async fn insert_user(user: User) -> Result<()> {
    query!(
        "INSERT INTO user (username, bcrypt) VALUES ($1, $2)",
        user.username,
        user.bcrypt
    )
    .execute(conn())
    .await?;

    Ok(())
}

pub async fn get_user(username: &str) -> Result<Option<User>> {
    Ok(query_as!(
        User,
        "SELECT username, bcrypt FROM user WHERE username = $1",
        username
    )
    .fetch_optional(conn())
    .await?)
}
