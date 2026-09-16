use std::{
    env,
    ffi::OsString,
    ops::{Div, Mul},
    str::FromStr,
};

use chrono::{Datelike, Local, NaiveDate};
use sha3::{Digest, Sha3_256};
use tokio::sync::RwLock;

use crate::{
    Result,
    db::{all_characters, get_answer, store_answer},
    err,
};

static ANSWER: RwLock<(String, NaiveDate)> = RwLock::const_new((String::new(), NaiveDate::MIN));

pub async fn today() -> Result<String> {
    let hash = |hasher: &Sha3_256, s: &str| {
        let mut hasher = hasher.clone();
        hasher.update(s.as_bytes());
        hasher.finalize()
    };

    let answer_guard = ANSWER.read().await;
    let today = Local::now().date_naive();
    if answer_guard.1 == today {
        Ok(answer_guard.0.clone())
    } else {
        let prefix = env::var_os("COSMEREDLE_SECRET")
            .or_else(|| OsString::from_str("r4nd0mbyt3s").ok())
            .ok_or(err!("Missing COSMEREDLE_SECRET variable"))?;
        let mut prefix = prefix.as_encoded_bytes().to_owned();
        prefix.extend_from_slice(today.year().div(4).mul(4).to_string().as_bytes());
        let mut keys = all_characters().await?;
        let hasher = Sha3_256::new_with_prefix(&prefix);
        keys.sort_unstable_by_key(|s| hash(&hasher, s));
        let yday = get_answer().await?;
        let s = if !yday.is_empty() {
            let yday_hash = hash(&hasher, &yday);
            match keys.binary_search_by_key(&yday_hash, |s| hash(&hasher, s)) {
                Ok(i) => &keys[(i + 1) % keys.len()],
                Err(i) => &keys[i % keys.len()],
            }
            .to_string()
        } else {
            keys[0].to_string()
        };
        drop(answer_guard);

        store_answer(&s).await?;
        *ANSWER.write().await = (s.clone(), today);

        Ok(s)
    }
}
