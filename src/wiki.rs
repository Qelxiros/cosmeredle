use std::{
    collections::{HashMap, HashSet},
    sync::{LazyLock, OnceLock},
};

use mediawiki::{
    Api,
    action_api::{
        ActionApiContinuable, ActionApiList, ActionApiQuery, ActionApiQueryCommonBuilder,
        ActionApiRunnable,
    },
};
use regex::{Captures, Regex};
use serde_json::Value;

use crate::{
    Character, Result,
    cache::{CACHE, store_cache},
    err,
};

static REGEX: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(?<brackets>\[\[([^\|\]]*?\|)?(.*?)\]\])(\{\{.*?\}\})?|(?<braces>\{\{([^\|\}]*\|)([^\|]*?)(\|.*?)?\}\})(\{\{.*?\}\})?|(?<small><small>(.*?)</small>)").unwrap()
});

static TAGS: LazyLock<HashMap<&str, &str>> = LazyLock::new(|| {
    include_str!("../tags")
        .lines()
        .map(|s| s.split_once('=').unwrap())
        .collect()
});

static API: OnceLock<Api> = OnceLock::new();

pub(crate) async fn init() -> Result<()> {
    let api = Api::new("https://coppermind.net/w/api.php").await?;
    API.get_or_init(|| api);

    Ok(())
}

fn api() -> &'static Api {
    API.get().unwrap()
}

fn format(v: &str) -> String {
    REGEX
        .replace_all(v, |cap: &Captures| {
            if cap.name("brackets").is_some() {
                cap[3].to_string()
            } else if cap.name("braces").is_some() {
                let left = &cap[6];
                let right = &cap[7];
                if left == "tag+|" {
                    TAGS.get(right).copied().unwrap_or(right).to_string()
                } else {
                    cap[7].to_string()
                }
            } else {
                String::new()
            }
        })
        .trim()
        .to_string()
}

fn format_abilities(v: &str) -> Vec<String> {
    static BAD: LazyLock<HashSet<&str>> = LazyLock::new(|| ["others"].into_iter().collect());

    v.split(',')
        .map(|s| format(s.trim()))
        .filter(|s| !BAD.contains(s.as_str()))
        .collect()
}

fn parse_table(wikitext: &str) -> Option<HashMap<&str, &str>> {
    let start = wikitext.find("{{character")?;
    let mut count = 0;
    let len = wikitext.split_at(start).1.find(|c| {
        match c {
            '{' => count += 1,
            '}' => count -= 1,
            _ => (),
        }
        count <= 0
    })?;
    let table = &wikitext[start..=start + len];

    Some(
        table
            .strip_prefix("{{character")?
            .strip_suffix("}}")?
            .trim()
            .lines()
            .filter_map(|s| s.strip_prefix('|')?.split_once('='))
            .collect(),
    )
}

pub async fn get_character_pages() -> Result<HashSet<String>> {
    let mut pages = vec![];
    let mut cont = Value::Null;

    loop {
        let chunk = ActionApiList::embeddedin()
            .eipageid(282)
            .einamespace(&[0])
            .eilimit(500)
            .continue_from(&cont)
            .run(api())
            .await?;
        let ch = chunk["query"]["embeddedin"]
            .as_array()
            .ok_or(err!("unexpected json structure {chunk}"))?;
        pages.extend_from_slice(ch);

        cont = chunk.clone();
        if cont["continue"].as_object().is_none() {
            break;
        }
    }

    Ok(pages
        .into_iter()
        .filter_map(|v| v["title"].as_str().map(str::to_string))
        .filter(|s| !s.contains(':'))
        .collect())
}

pub async fn get_character(name: &str) -> Result<Character> {
    let val = ActionApiQuery::revisions()
        .titles(&[name])
        .rvprop(&["ids", "content", "contentmodel", "timestamp", "flags"])
        .rvlimit(1)
        .rvdir("older")
        .rvslots(&["main"])
        .run(api())
        .await?;

    let pages = val["query"]["pages"]
        .as_object()
        .ok_or(err!("unexpected json structure {val}"))?;
    let page_id = pages.keys().next().ok_or(err!("no pages"))?;
    let text = pages[page_id]["revisions"]
        .as_array()
        .and_then(|x| x.first().and_then(|x| x["slots"]["main"]["*"].as_str()))
        .ok_or(err!("failed to get wikitext"))?;

    let map = parse_table(text).ok_or(err!("No table found"))?;
    let mut out = Character::default();

    for (k, v) in map {
        if k == "abilities" {
            out.abilities(format_abilities(v));
        } else {
            out.add(k, format(v));
        }
    }
    out.name(name.to_string());

    Ok(out)
}

pub async fn sync_characters(names: HashSet<String>) -> Result<()> {
    for name in names {
        if CACHE.read().await.contains_key(&name) {
            continue;
        }

        let character = get_character(&name).await;
        if let Ok(ch) = character {
            CACHE.write().await.insert(name, ch);
            let _ = store_cache().await;
        }
    }

    store_cache().await
}
