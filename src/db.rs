use std::{fs::File, sync::OnceLock};

use axum_login::UserId;
use chrono::Local;
use derive_debug::Dbg;
use sqlx::{SqlitePool, migrate, query, query_as, types::Json};

use crate::{Character, Result, backend::Backend};

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
        r#"SELECT id, username, bcrypt, json_group_array(guess) FILTER (WHERE guess IS NOT NULL) AS "guesses!: Json<Vec<String>>" FROM user LEFT JOIN guess ON user.id = guess.user_id WHERE id = $1 GROUP BY id, username, bcrypt"#,
        id
    )
    .fetch_optional(conn())
    .await?)
}

pub async fn get_user_by_username(username: &str) -> Result<Option<User>> {
    Ok(query_as!(
        User,
        r#"SELECT id, username, bcrypt, json_group_array(guess) FILTER (WHERE guess IS NOT NULL) AS "guesses!: Json<Vec<String>>" FROM user INNER JOIN guess ON user.id = guess.user_id WHERE username = $1 GROUP BY id, username, bcrypt"#,
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

pub async fn insert_character(character: &Character) -> Result<()> {
    let mut tx = conn().begin().await?;

    let active = character.universe == "Cosmere";

    query!(
        "INSERT INTO character (name, unnamed, parents, spouse, siblings, children, ancestors, relatives, descendants, born, died, bonded, titles, aliases, skills, achievements, powers, hash_profession, profession, occupation, religion, groups, species, tick_species, era, birthplace, tick_birthplace, residence, tick_residence, ethnicity, tick_ethnicity, nation, tick_nation, nationality, world, tick_world, hide_world, universe, introduced, active)
        VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13, $14, $15, $16, $17, $18, $19, $20, $21, $22, $23, $24, $25, $26, $27, $28, $29, $30, $31, $32, $33, $34, $35, $36, $37, $38, $39, $40)",
        character.name,
        character.unnamed,
        character.parents,
        character.spouse,
        character.siblings,
        character.children,
        character.ancestors,
        character.relatives,
        character.descendants,
        character.born,
        character.died,
        character.bonded,
        character.titles,
        character.aliases,
        character.skills,
        character.achievements,
        character.powers,
        character.hash_profession,
        character.profession,
        character.occupation,
        character.religion,
        character.groups,
        character.species,
        character.tick_species,
        character.era,
        character.birthplace,
        character.tick_birthplace,
        character.residence,
        character.tick_residence,
        character.ethnicity,
        character.tick_ethnicity,
        character.nation,
        character.tick_nation,
        character.nationality,
        character.world,
        character.tick_world,
        character.hide_world,
        character.universe,
        character.introduced,
        active,
    ).execute(&mut *tx).await?;

    let abilities = Json(&character.abilities);
    query!(
        "INSERT INTO ability (character, ability) SELECT $1, value FROM json_each($2)",
        character.name,
        abilities
    )
    .execute(&mut *tx)
    .await?;

    Ok(tx.commit().await?)
}

pub async fn get_character(name: &str) -> Result<Character> {
    Ok(query_as!(
        Character,
        r#"SELECT
        character.name AS "name!",
        character.unnamed AS "unnamed!: bool",
        character.parents AS "parents!",
        character.spouse AS "spouse!",
        character.siblings AS "siblings!",
        character.children AS "children!",
        character.ancestors AS "ancestors!",
        character.relatives AS "relatives!",
        character.descendants AS "descendants!",
        character.born AS "born!",
        character.died AS "died!",
        json_group_array(ability) FILTER (WHERE ability IS NOT NULL) AS "abilities!: Json<Vec<String>>",
        character.bonded AS "bonded!",
        character.titles AS "titles!",
        character.aliases AS "aliases!",
        character.skills AS "skills!",
        character.achievements AS "achievements!",
        character.powers AS "powers!",
        character.hash_profession AS "hash_profession!",
        character.profession AS "profession!",
        character.occupation AS "occupation!",
        character.religion AS "religion!",
        character.groups AS "groups!",
        character.species AS "species!",
        character.tick_species AS "tick_species!",
        character.era AS "era!",
        character.birthplace AS "birthplace!",
        character.tick_birthplace AS "tick_birthplace!",
        character.residence AS "residence!",
        character.tick_residence AS "tick_residence!",
        character.ethnicity AS "ethnicity!",
        character.tick_ethnicity AS "tick_ethnicity!",
        character.nation AS "nation!",
        character.tick_nation AS "tick_nation!",
        character.nationality AS "nationality!",
        character.world AS "world!",
        character.tick_world AS "tick_world!",
        character.hide_world AS "hide_world!: bool",
        character.universe AS "universe!",
        character.introduced AS "introduced!"
        FROM character
        LEFT JOIN ability ON character.name = ability.character
        WHERE character.name = $1
        GROUP BY character.name"#,
        name
    )
    .fetch_one(conn())
    .await?)
}

pub async fn character_present(name: &str) -> bool {
    query!(
        "SELECT name FROM character WHERE name = $1 AND active",
        name
    )
    .fetch_optional(conn())
    .await
    .is_ok_and(|o| o.is_some())
}

pub async fn all_characters() -> Result<Vec<String>> {
    Ok(
        query!(r#"SELECT name FROM character WHERE universe = "Cosmere" AND active"#)
            .fetch_all(conn())
            .await?
            .into_iter()
            .map(|r| r.name)
            .collect::<Vec<_>>(),
    )
}

pub async fn deactivate_characters(active: &[String]) -> Result<()> {
    let json = Json(active);
    query!(
        "UPDATE character SET active = false WHERE name NOT IN (SELECT value FROM json_each($1))",
        json
    )
    .execute(conn())
    .await?;

    Ok(())
}

pub async fn store_answer(answer: &str) -> Result<()> {
    let mut tx = conn().begin().await?;

    let affected = query!("UPDATE answer SET answer = $1", answer)
        .execute(&mut *tx)
        .await?;

    if affected.rows_affected() != 1 {
        query!("DELETE FROM answer").execute(&mut *tx).await?;
        query!("INSERT INTO answer (answer) VALUES ($1)", answer)
            .execute(&mut *tx)
            .await?;
    }

    Ok(tx.commit().await?)
}

pub async fn get_answer() -> Result<String> {
    Ok(query!("SELECT answer FROM answer LIMIT 1")
        .fetch_one(conn())
        .await
        .map(|r| r.answer)?)
}
