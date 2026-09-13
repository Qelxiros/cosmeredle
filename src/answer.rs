use std::{
    env,
    ops::{Div, Mul},
};

use chrono::{Datelike, Local, NaiveDate};
use itertools::Itertools;
use sha3::{Digest, Sha3_256};
use tokio::sync::RwLock;

use crate::cache::CACHE;

static ANSWER: RwLock<(String, NaiveDate)> = RwLock::const_new((String::new(), NaiveDate::MIN));

pub async fn today() -> String {
    let guard = ANSWER.read().await;
    let today = Local::now().date_naive();
    if guard.1 < today {
        let yday = guard.0.clone();
        drop(guard);
        compute_today(&yday, today).await
    } else {
        guard.0.clone()
    }
}

async fn compute_today(yday: &str, today: NaiveDate) -> String {
    let prefix = env::var_os("COSMEREDLE_SECRET").unwrap();
    let mut prefix = prefix.as_encoded_bytes().to_owned();
    prefix.extend_from_slice(today.year().div(4).mul(4).to_string().as_bytes());
    let guard = CACHE.read().await;
    let mut keys = guard.keys().collect_vec();
    let mut hasher = Sha3_256::new_with_prefix(&prefix);
    keys.sort_unstable_by_key(|s| {
        let mut hasher = hasher.clone();
        hasher.update(s.as_bytes());
        hasher.finalize()
    });
    hasher.update(yday.as_bytes());
    let yday_hash = hasher.finalize();

    let hasher = Sha3_256::new_with_prefix(&prefix);
    let s = match keys.binary_search_by_key(&yday_hash, |s| {
        let mut hasher = hasher.clone();
        hasher.update(s.as_bytes());
        hasher.finalize()
    }) {
        Ok(i) => keys[(i + 1) % keys.len()],
        Err(i) => keys[i % keys.len()],
    }
    .to_string();
    let s = "Ashravan".to_string();
    println!("{:?}", guard.get(&s));
    drop(guard);

    *ANSWER.write().await = (s.clone(), today);

    s
}
