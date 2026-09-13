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
    let f = File::open("cache").unwrap();
    let map = from_reader(f)?;
    *CACHE.write().await = map;
    Ok(())
}

pub async fn store_cache() -> Result<()> {
    let f = File::create("cache.new")?;
    into_writer(&*CACHE.read().await, f)?;
    rename("cache.new", "cache")?;
    Ok(())
}

pub async fn update_cache() -> Result<()> {
    load_cache().await?;
    let characters = get_character_pages().await.unwrap();
    let modified = sync_characters(characters, &mut *CACHE.write().await)
        .await
        .unwrap();

    if modified {
        store_cache().await?;
    }

    Ok(())
}
