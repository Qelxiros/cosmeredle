use std::{
    collections::{HashMap, HashSet},
    sync::{LazyLock, OnceLock},
};

use itertools::Itertools;
use mediawiki::{
    Api,
    action_api::{
        ActionApiContinuable, ActionApiList, ActionApiQuery, ActionApiQueryCommonBuilder,
        ActionApiRunnable,
    },
};
use regex::{Captures, Regex};
use serde_json::Value;
use titlecase::Titlecase;

use crate::{
    Book, Character, Result,
    db::{
        book_present, character_present, deactivate_characters, insert_book, insert_character,
        reactivate_characters,
    },
    err,
};

static REGEX: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(?<brackets>\[\[([^\|\]]*?\|)?(.*?)\]\])(\{\{.*?\}\})?|(?<braces>\{\{([^\|\}]*\|)([^\|]*?)(\|.*?)?\}\})(\{\{.*?\}\})?|(?<small><small>(.*?)</small>)").unwrap()
});

/// Coppermind's MediaWiki endpoint.
pub const API_URL: &str = "https://coppermind.net/w/api.php";
/// Templates whose transclusions define the character and book lists.
pub const CHARACTER_TEMPLATE: &str = "Template:Character";
pub const BOOK_TEMPLATE: &str = "Template:Book";
/// Infobox names passed to `parse_table`.
pub const CHARACTER_TABLE: &str = "character";
pub const BOOK_TABLE: &str = "book";

static API: OnceLock<Api> = OnceLock::new();

pub(crate) async fn init() -> Result<()> {
    let api = Api::new(API_URL).await?;
    API.get_or_init(|| api);

    Ok(())
}

fn api() -> &'static Api {
    API.get().unwrap()
}

/// Strips wiki markup (links, templates, `<small>`) and titlecases the result.
pub fn format(v: &str) -> String {
    REGEX
        .replace_all(v, |cap: &Captures| {
            if cap.name("brackets").is_some() {
                cap[3].to_string()
            } else if cap.name("braces").is_some() {
                cap[7].to_string()
            } else {
                String::new()
            }
        })
        .trim()
        .to_string()
        .titlecase()
}

/// Splits a comma-separated `abilities` field into formatted entries.
pub fn format_abilities(v: &str) -> Vec<String> {
    static BAD: LazyLock<HashSet<&str>> = LazyLock::new(|| ["Others", ""].into_iter().collect());

    v.split(',')
        .map(|s| format(s.trim()))
        .filter(|s| !BAD.contains(s.as_str()))
        .collect()
}

/// Extracts the named infobox from a page's wikitext as key/value pairs.
pub fn parse_table(wikitext: &str, table: &str) -> Option<HashMap<String, String>> {
    let prefix = "{{".to_string() + table;
    let start = wikitext.find(&prefix)?;
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
            .strip_prefix(&prefix)?
            .strip_suffix("}}")?
            .trim()
            .lines()
            .filter_map(|s| s.strip_prefix('|')?.split_once('='))
            .map(|(k, v)| (k.trim().to_string(), v.trim().to_string()))
            .collect(),
    )
}

pub async fn get_template_pages(template: &str) -> Result<HashSet<String>> {
    let mut pages = vec![];
    let mut cont = Value::Null;

    loop {
        let chunk = ActionApiList::embeddedin()
            .eititle(template)
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

async fn get_page_table(name: &str, table: &str) -> Result<HashMap<String, String>> {
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

    parse_table(text, table).ok_or(err!("No table found"))
}

pub async fn get_character(name: &str) -> Result<Character> {
    let map = get_page_table(name, CHARACTER_TABLE).await?;
    let mut out = Character::default();

    for (k, v) in map {
        if k == "abilities" {
            out.abilities(format_abilities(&v));
        } else {
            out.add(&k, format(&v));
        }
    }
    out.name(name.to_string());

    Ok(out)
}

pub async fn get_book(name: &str) -> Result<Book> {
    let map = get_page_table(name, BOOK_TABLE).await?;

    Ok(Book {
        title: name.to_string(),
        series: map.get("series").map(|s| format(s)),
    })
}

pub async fn sync_characters() -> Result<()> {
    let names = get_template_pages(CHARACTER_TEMPLATE)
        .await?
        .into_iter()
        .collect_vec();
    deactivate_characters(&names).await?;
    reactivate_characters(&names).await?;
    for name in names {
        if character_present(&name).await {
            continue;
        }

        let character = get_character(&name).await;
        if let Ok(ch) = character
            && let Err(e) = insert_character(&ch).await
        {
            log::warn!("Failed to insert character {ch:?}: {e}");
        }
    }

    Ok(())
}

pub async fn sync_books() -> Result<()> {
    let titles = get_template_pages(BOOK_TEMPLATE)
        .await?
        .into_iter()
        .collect_vec();
    for title in titles {
        if book_present(&title).await {
            continue;
        }

        let book = get_book(&title).await;
        if let Ok(b) = book {
            insert_book(&b).await?;
        }
    }

    Ok(())
}
