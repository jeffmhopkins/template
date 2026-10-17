//! `docs due`: docs whose `review_every` has passed since their content or `reviewed`
//! value last changed, and external URLs whose `recheck_every` has passed since their entry
//! last changed. Clocks come from git history on HEAD.

use std::fmt::Write as _;

use anyhow::Result;

use super::check::days;
use super::history::{date, last_change};
use super::{Loaded, hash, parse};

/// One overdue item.
#[derive(Debug, PartialEq, Eq)]
pub struct Due {
    pub what: String,
    pub since: i64,
}

/// Everything due at `now`.
pub fn due(l: &Loaded, now: i64) -> Result<Vec<Due>> {
    let mut out = Vec::new();
    for doc in l
        .docs
        .iter()
        .filter(|d| !d.is_adr() && d.status() != "retired")
    {
        let Some(every) = doc.meta_str("review_every").and_then(days) else {
            continue;
        };
        let path = doc.path.clone();
        let state = move |text: &str| {
            let d = parse::parse(&path, text, &mut Vec::new());
            let reviewed = d
                .meta
                .as_ref()
                .and_then(|m| m.get("reviewed"))
                .map(ToString::to_string);
            (hash::body(&d, None), reviewed)
        };
        if let Some(t) = last_change(&l.root, &doc.path, &state)? {
            let since = t + every * 86_400;
            if since <= now {
                out.push(Due {
                    what: format!(
                        "doc {} ({}): review every {}d; run `repo docs review {} --by <who> --note \"<what you re-read>\"`",
                        doc.id(),
                        doc.path,
                        every,
                        doc.id()
                    ),
                    since,
                });
            }
        }
    }
    let ext = l.config.docs.external.as_str();
    let table: toml::Table = std::fs::read_to_string(l.root.join(ext))
        .ok()
        .and_then(|t| t.parse().ok())
        .unwrap_or_default();
    let entries = table
        .get("external")
        .and_then(|v| v.as_array())
        .cloned()
        .unwrap_or_default();
    for e in entries {
        let get = |k: &str| e.get(k).and_then(|v| v.as_str()).map(str::to_owned);
        let (Some(id), Some(url), Some(every)) = (
            get("id"),
            get("url"),
            get("recheck_every").as_deref().and_then(days),
        ) else {
            continue;
        };
        let key = id.clone();
        let state = move |text: &str| -> Option<String> {
            let t: toml::Table = text.parse().ok()?;
            t.get("external")?
                .as_array()?
                .iter()
                .find(|x| x.get("id").and_then(|v| v.as_str()) == Some(key.as_str()))
                .map(ToString::to_string)
        };
        if let Some(t) = last_change(&l.root, ext, &state)? {
            let since = t + every * 86_400;
            if since <= now {
                out.push(Due {
                    what: format!("external {id} <{url}>: recheck every {every}d, then commit a change to its entry in {ext}"),
                    since,
                });
            }
        }
    }
    Ok(out)
}

pub fn render(items: &[Due]) -> String {
    let mut s = String::new();
    if items.is_empty() {
        s.push_str("docs due: nothing is due\n");
    }
    for d in items {
        let _ = writeln!(s, "- due since {}: {}", date(d.since), d.what);
    }
    s
}
