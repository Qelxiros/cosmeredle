use std::{
    env,
    ffi::OsString,
    ops::{Div, Mul},
    str::FromStr,
};

use chrono::{Datelike, Local, NaiveDate};
use itertools::Itertools;
use sha3::{Digest, Sha3_256};
use tokio::sync::RwLock;

use crate::cache::CACHE;

static ANSWER: RwLock<(String, NaiveDate)> = RwLock::const_new((String::new(), NaiveDate::MIN));

pub async fn today() -> String {
    let answer_guard = ANSWER.read().await;
    let today = Local::now().date_naive();
    if answer_guard.1 < today {
        let prefix = env::var_os("COSMEREDLE_SECRET")
            .or_else(|| OsString::from_str("r4nd0mbyt3s").ok())
            .unwrap();
        let mut prefix = prefix.as_encoded_bytes().to_owned();
        prefix.extend_from_slice(today.year().div(4).mul(4).to_string().as_bytes());
        let cache_guard = CACHE.read().await;
        let mut keys = cache_guard.keys().collect_vec();
        let mut hasher = Sha3_256::new_with_prefix(&prefix);
        keys.sort_unstable_by_key(|s| {
            let mut hasher = hasher.clone();
            hasher.update(s.as_bytes());
            hasher.finalize()
        });
        let s = if !answer_guard.0.is_empty() {
            hasher.update(answer_guard.0.as_bytes());
            let yday_hash = hasher.finalize();

            let hasher = Sha3_256::new_with_prefix(&prefix);
            match keys.binary_search_by_key(&yday_hash, |s| {
                let mut hasher = hasher.clone();
                hasher.update(s.as_bytes());
                hasher.finalize()
            }) {
                Ok(i) => keys[(i + 1) % keys.len()],
                Err(i) => keys[i % keys.len()],
            }
            .to_string()
        } else {
            keys[0].to_string()
        };
        drop(cache_guard);
        drop(answer_guard);

        *ANSWER.write().await = (s.clone(), today);

        s
    } else {
        answer_guard.0.clone()
    }
}
