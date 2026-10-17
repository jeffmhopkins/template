//! The verification ledger (`[docs].ledger`, by default `docs/verified.toml`).
//!
//! A section's stamp is one entry per input, each with the hash that input had when the
//! section was last re-read against it. Every entry is a table header naming the section
//! and input, one `stamp` line and a blank line. The header stays put while the input
//! exists, so two branches that change different inputs of one section touch different
//! lines and git merges them cleanly; two branches that change the same input conflict on
//! its `stamp` line, which `docs merge-main` resolves.

use std::collections::BTreeMap;
use std::fmt::Write as _;
use std::path::Path;

use anyhow::{Context, Result};

use super::hash::toml_str;

const HEADER: &str = "\
# Verification stamps: one entry per section input. Written by `repo docs verify`; never edit by hand.
";

/// One input's stamp: its hash when last verified, and who verified it and why.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Entry {
    pub hash: String,
    pub by: Option<String>,
    pub note: Option<String>,
}

/// One section's entries, keyed `body`, `file:<path>`, `dep:<ref>`.
pub type Stamp = BTreeMap<String, Entry>;

/// Every section's stamp, keyed `DOC#anchor`.
#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct Ledger {
    pub sections: BTreeMap<String, Stamp>,
}

impl Ledger {
    /// Parses ledger text; the error says what is wrong.
    pub fn parse(text: &str) -> Result<Self, String> {
        let table: toml::Table = text.parse().map_err(|e| format!("{e}"))?;
        let mut sections = BTreeMap::new();
        for (section, v) in table {
            let t = v
                .as_table()
                .ok_or_else(|| format!("`{section}` must be a table of entries"))?;
            let mut stamp = Stamp::new();
            for (key, wrapper) in t {
                let w = wrapper
                    .as_table()
                    .ok_or_else(|| format!("`{section}` entry `{key}` must be a table"))?;
                if let Some(k) = w.keys().find(|k| k.as_str() != "stamp") {
                    return Err(format!("`{section}` entry `{key}`: unknown key `{k}`"));
                }
                let e = w
                    .get("stamp")
                    .ok_or_else(|| format!("`{section}` entry `{key}` has no `stamp`"))?;
                let e_table = e
                    .as_table()
                    .ok_or_else(|| format!("`{section}` entry `{key}`: `stamp` must be a table"))?;
                if let Some(k) = e_table
                    .keys()
                    .find(|k| !["hash", "by", "note"].contains(&k.as_str()))
                {
                    return Err(format!("`{section}` entry `{key}`: unknown key `{k}`"));
                }
                let field = |k: &str| -> Result<Option<String>, String> {
                    match e_table.get(k) {
                        None => Ok(None),
                        Some(toml::Value::String(s)) if !s.is_empty() => Ok(Some(s.clone())),
                        Some(_) => Err(format!(
                            "`{section}` entry `{key}`: `{k}` must be a non-empty string"
                        )),
                    }
                };
                let hash = field("hash")?
                    .ok_or_else(|| format!("`{section}` entry `{key}` has no `hash`"))?;
                let (by, note) = (field("by")?, field("note")?);
                if by.is_some() != note.is_some() {
                    return Err(format!(
                        "`{section}` entry `{key}`: `by` and `note` go together"
                    ));
                }
                stamp.insert(key.clone(), Entry { hash, by, note });
            }
            sections.insert(section, stamp);
        }
        Ok(Self { sections })
    }

    /// The ledger at `root/path`; empty when the file doesn't exist.
    pub fn read(root: &Path, path: &str) -> Result<Self> {
        match std::fs::read_to_string(root.join(path)) {
            Ok(text) => Self::parse(&text).map_err(|e| anyhow::anyhow!("{path}: {e}")),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(Self::default()),
            Err(e) => Err(e).with_context(|| format!("reading {path}")),
        }
    }

    /// The canonical text: sections and entries sorted, each entry a header, its `stamp`
    /// line and a blank line.
    pub fn render(&self) -> String {
        let mut out = format!("{HEADER}\n");
        for (section, stamp) in &self.sections {
            for (key, e) in stamp {
                let _ = write!(
                    out,
                    "[{}.{}]\nstamp = {{ hash = {}",
                    toml_str(section),
                    toml_str(key),
                    toml_str(&e.hash)
                );
                if let (Some(by), Some(note)) = (&e.by, &e.note) {
                    let _ = write!(out, ", by = {}, note = {}", toml_str(by), toml_str(note));
                }
                out.push_str(" }\n\n");
            }
        }
        out
    }

    pub fn write(&self, root: &Path, path: &str) -> Result<()> {
        let full = root.join(path);
        if let Some(dir) = full.parent() {
            std::fs::create_dir_all(dir).with_context(|| format!("creating {}", dir.display()))?;
        }
        std::fs::write(&full, self.render()).with_context(|| format!("writing {path}"))
    }

    /// Drops every section `keep` rejects; returns the dropped keys.
    pub fn prune(&mut self, keep: &dyn Fn(&str) -> bool) -> Vec<String> {
        let gone: Vec<String> = self.sections.keys().filter(|k| !keep(k)).cloned().collect();
        for k in &gone {
            self.sections.remove(k);
        }
        gone
    }
}

/// The inputs whose hash differs between `stamp` and `inputs` (changed, added or removed),
/// sorted; empty when the section is fresh.
pub fn diff(stamp: Option<&Stamp>, inputs: &BTreeMap<String, String>) -> Vec<String> {
    let empty = Stamp::new();
    let stamp = stamp.unwrap_or(&empty);
    let mut out: Vec<String> = inputs
        .iter()
        .filter(|(k, h)| stamp.get(*k).is_none_or(|e| &e.hash != *h))
        .map(|(k, _)| k.clone())
        .chain(stamp.keys().filter(|k| !inputs.contains_key(*k)).cloned())
        .collect();
    out.sort();
    out
}

/// The stamp `docs verify` writes: entries whose hash still matches keep their `by` and
/// `note`; changed and new inputs get this verification's; removed inputs are dropped.
pub fn restamp(
    old: Option<&Stamp>,
    inputs: &BTreeMap<String, String>,
    by: &str,
    note: &str,
) -> Stamp {
    inputs
        .iter()
        .map(|(k, h)| {
            let e = match old.and_then(|o| o.get(k)) {
                Some(e) if &e.hash == h => e.clone(),
                _ => Entry {
                    hash: h.clone(),
                    by: Some(by.to_owned()),
                    note: Some(note.to_owned()),
                },
            };
            (k.clone(), e)
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn inputs(pairs: &[(&str, &str)]) -> BTreeMap<String, String> {
        pairs
            .iter()
            .map(|(k, v)| ((*k).to_owned(), (*v).to_owned()))
            .collect()
    }

    #[test]
    fn render_parses_back_one_header_line_and_blank_per_entry() {
        let mut l = Ledger::default();
        l.sections.insert(
            "B#b".into(),
            restamp(
                None,
                &inputs(&[("body", "b3:1"), ("file:a \"q\".rs", "b3:2")]),
                "agent:x",
                "said \"why\"",
            ),
        );
        l.sections.insert(
            "A#a".into(),
            [(
                "body".to_owned(),
                Entry {
                    hash: "b3:3".into(),
                    by: None,
                    note: None,
                },
            )]
            .into(),
        );
        let text = l.render();
        assert!(text.starts_with("# Verification stamps"), "{text}");
        assert_eq!(Ledger::parse(&text), Ok(l.clone()));
        let body: Vec<&str> = text.lines().skip_while(|x| x.starts_with('#')).collect();
        assert!(body[0].is_empty(), "{text}");
        for entry in body[1..].chunks(3) {
            assert!(
                entry[0].starts_with('[') && entry[0].ends_with(']'),
                "{text}"
            );
            assert!(entry[1].starts_with("stamp = { hash = "), "{text}");
            assert!(entry[2].is_empty(), "{text}");
        }
        assert_eq!(body.len(), 1 + 3 * 3, "{text}");
    }

    #[test]
    fn parse_rejects_malformed_entries() {
        for (bad, why) in [
            ("x = 1", "table of entries"),
            ("[\"A#a\"]\nbody = 1", "must be a table"),
            ("[\"A#a\".body]\nother = 1", "unknown key"),
            ("[\"A#a\".body]\nstamp = 1", "`stamp` must be a table"),
            (
                "[\"A#a\".body]\nstamp = { by = \"a\", note = \"n\" }",
                "no `hash`",
            ),
            (
                "[\"A#a\".body]\nstamp = { hash = \"h\", by = \"a\" }",
                "go together",
            ),
            ("[\"A#a\".body]\nstamp = { hash = \"\" }", "non-empty"),
            (
                "[\"A#a\".body]\nstamp = { hash = \"h\", x = \"x\" }",
                "unknown key",
            ),
        ] {
            let err = Ledger::parse(bad).expect_err(bad);
            assert!(err.contains(why), "{bad}: {err}");
        }
    }

    #[test]
    fn diff_names_changed_added_and_removed_inputs() {
        let stamp = restamp(
            None,
            &inputs(&[("body", "1"), ("file:a", "2"), ("file:b", "3")]),
            "agent:x",
            "n",
        );
        let now = inputs(&[("body", "1"), ("file:a", "9"), ("file:c", "4")]);
        assert_eq!(diff(Some(&stamp), &now), vec!["file:a", "file:b", "file:c"]);
        assert_eq!(diff(None, &inputs(&[("body", "1")])), vec!["body"]);
    }

    #[test]
    fn restamp_rewrites_only_changed_entries() {
        let old = restamp(
            None,
            &inputs(&[("body", "1"), ("file:a", "2"), ("file:b", "3")]),
            "agent:old",
            "old",
        );
        let new = restamp(
            Some(&old),
            &inputs(&[("body", "1"), ("file:a", "9")]),
            "agent:new",
            "new",
        );
        assert_eq!(new["body"], old["body"]);
        assert_eq!(new["file:a"].by.as_deref(), Some("agent:new"));
        assert!(!new.contains_key("file:b"));
        let mut l = Ledger::default();
        l.sections.insert("A#a".into(), new);
        assert_eq!(l.prune(&|k| k != "A#a"), vec!["A#a".to_owned()]);
        assert!(l.sections.is_empty());
    }
}
