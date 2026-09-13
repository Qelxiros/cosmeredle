use std::{
    collections::HashMap,
    fs::{File, rename},
    sync::LazyLock,
};

use ciborium::{from_reader, into_writer};
use tokio::sync::RwLock;

use crate::{
    Character, Result,
    wiki::{get_character_pages, sync_characters},
};

pub static CACHE: LazyLock<RwLock<HashMap<String, Character>>> =
    LazyLock::new(|| RwLock::new(HashMap::new()));

pub async fn load_cache() -> Result<()> {
    let Ok(f) = File::open("storage/cache") else {
        return Ok(());
    };
    let map = from_reader(f)?;
    *CACHE.write().await = map;
    Ok(())
}

pub async fn store_cache() -> Result<()> {
    let f = File::create("storage/cache.new")?;
    into_writer(&*CACHE.read().await, f)?;
    rename("storage/cache.new", "storage/cache")?;
    Ok(())
}

pub async fn update_cache() -> Result<()> {
    load_cache().await?;
    let characters = get_character_pages().await?;
    sync_characters(characters).await
}
