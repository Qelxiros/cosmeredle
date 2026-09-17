use std::{
    env,
    ops::{Div, Mul},
};

use chrono::{Datelike, Local, NaiveDate};
use rand::{RngExt, rng};
use sha3::{Digest, Sha3_256};
use tokio::sync::RwLock;

use crate::{
    Result,
    db::{all_characters, get_answer, store_answer},
    err,
};

/// Environment variable holding the secret that seeds the shuffle.
pub const SECRET_ENV: &str = "COSMEREDLE_SECRET";
/// The shuffle is reseeded once every this many years.
pub const ERA_YEARS: i32 = 4;

static ANSWER: RwLock<(String, NaiveDate)> = RwLock::const_new((String::new(), NaiveDate::MIN));

pub async fn today() -> Result<String> {
    let hash = |hasher: &Sha3_256, s: &str| {
        let mut hasher = hasher.clone();
        hasher.update(s.as_bytes());
        hasher.finalize()
    };

    let mut answer_guard = ANSWER.write().await;
    let today = Local::now().date_naive();
    if answer_guard.1 == today {
        Ok(answer_guard.0.clone())
    } else {
        let yday = get_answer().await?;
        let days_since = if let Some((ref yday, yday_day)) = yday {
            if yday_day == today {
                return Ok(yday.clone());
            } else {
                (today - yday_day).num_days().unsigned_abs() as usize
            }
        } else {
            0
        };
        let mut prefix = env::var_os(SECRET_ENV)
            .map(|s| s.as_encoded_bytes().to_vec())
            .unwrap_or_else(|| {
                log::warn!("Missing {SECRET_ENV}, falling back to random bytes");
                let random_bytes: [u8; 32] = rng().random();
                random_bytes.to_vec()
            });
        prefix.extend_from_slice(today.year().div(ERA_YEARS).mul(ERA_YEARS).to_string().as_bytes());
        let mut keys = all_characters().await?;
        if keys.is_empty() {
            return Err(err!("No characters found"));
        }
        let hasher = Sha3_256::new_with_prefix(&prefix);
        keys.sort_unstable_by_key(|s| hash(&hasher, s));
        let s = if let Some((yday, _)) = yday {
            let yday_hash = hash(&hasher, &yday);
            match keys.binary_search_by_key(&yday_hash, |s| hash(&hasher, s)) {
                Ok(i) => &keys[(i + days_since) % keys.len()],
                Err(i) => &keys[(i + days_since - 1) % keys.len()],
            }
            .to_string()
        } else {
            keys[0].to_string()
        };

        store_answer(&s, today).await?;
        *answer_guard = (s.clone(), today);

        Ok(s)
    }
}
