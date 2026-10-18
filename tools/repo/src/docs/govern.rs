//! `repo docs for <path>`: what governs a file.
//!
//! - **Sections** whose `covers` match the path (retired docs excluded).
//! - **Rules** whose `applies_to` match it (retired rules excluded).
//! - **Process rules**: active rules with `applies_to = []`, which govern every PR rather
//!   than a path, so they are listed for every file.
//! - **ADRs**, neither superseded nor rejected, cited by a matched section's `depends_on`,
//!   by a link inside a matched section or rule, or by the back-reference line of the file
//!   or of its package root.
//!
//! The path need not exist yet.

use std::collections::BTreeSet;
use std::fmt::Write as _;
use std::path::Path;

use super::backrefs::refs_line;
use super::check::{normalize, package_of, packages};
use super::parse::Doc;
use super::{Index, Loaded, strings};

/// What governs one path: sections and rules as `ID#anchor`, ADRs as ids.
#[derive(Debug, Default, PartialEq, Eq)]
pub struct Governing {
    pub sections: BTreeSet<String>,
    pub rules: BTreeSet<String>,
    /// Active rules with `applies_to = []`: they govern every PR, whatever its paths.
    pub process: BTreeSet<String>,
    pub adrs: BTreeSet<String>,
}

fn matches(globs: &[String], path: &str) -> bool {
    crate::glob::Globs::new(globs).is_ok_and(|s| s.is_match(path))
}

/// The repo path a relative Markdown link in `doc` points at.
fn link_target(doc: &Doc, dest: &str) -> Option<String> {
    let file = dest.split('#').next()?;
    if file.is_empty() || file.contains("://") || file.starts_with("mailto:") {
        return None;
    }
    normalize(&Path::new(&doc.path).parent()?.join(file))
}

pub fn governing(l: &Loaded, index: &Index<'_>, path: &str) -> Governing {
    let mut g = Governing::default();
    let mut cited: BTreeSet<String> = BTreeSet::new();
    for doc in l.docs.iter().filter(|d| d.status() != "retired") {
        let mut ranges: Vec<(usize, usize)> = Vec::new();
        for s in &doc.sections {
            let Some(meta) = &s.meta else { continue };
            if matches(&strings(meta, "covers"), path) {
                g.sections.insert(format!("{}#{}", doc.id(), s.anchor));
                cited.extend(
                    strings(meta, "depends_on")
                        .into_iter()
                        .map(|d| d.split('#').next().unwrap_or_default().to_owned()),
                );
                ranges.push((s.body.0 + 1, s.body.1));
            }
        }
        for r in &doc.rules {
            let Some(meta) = &r.meta else { continue };
            let retired = meta.get("status").and_then(|v| v.as_str()) == Some("retired");
            if retired {
                continue;
            }
            let globs = strings(meta, "applies_to");
            if globs.is_empty() && meta.contains_key("applies_to") {
                g.process.insert(format!("{}#{}", doc.id(), r.id));
            } else if matches(&globs, path) {
                g.rules.insert(format!("{}#{}", doc.id(), r.id));
                ranges.push((r.line, r.body.1));
            }
        }
        for link in &doc.links {
            if ranges.iter().any(|&(a, b)| (a..=b).contains(&link.line)) {
                if let Some(t) = link_target(doc, &link.dest) {
                    cited.insert(t);
                }
            }
        }
    }
    let prefix = &l.pack.backrefs.prefix;
    let mut code = vec![path.to_owned()];
    if let Ok(roots) = crate::glob::Globs::new(&l.pack.backrefs.roots) {
        let mut pkgs = packages(&l.files, &l.pack.detect);
        if l.files.contains(&l.pack.detect) {
            pkgs.push("");
        }
        let own = package_of(path, &pkgs);
        code.extend(
            l.files
                .iter()
                .filter(|f| roots.is_match(f) && own.is_some() && package_of(f, &pkgs) == own)
                .cloned(),
        );
    }
    for f in code {
        if let Ok(text) = std::fs::read_to_string(l.root.join(&f)) {
            if let Some((_, refs)) = refs_line(&text, prefix) {
                cited.extend(
                    refs.iter()
                        .map(|r| r.split('#').next().unwrap_or_default().to_owned()),
                );
            }
        }
    }
    g.adrs = cited
        .into_iter()
        .filter_map(|c| index.get(&c))
        .filter(|d| d.is_adr() && !matches!(d.status(), "superseded" | "rejected"))
        .map(|d| d.id().to_owned())
        .collect();
    g
}

/// A label for a reference: a section's or rule's heading, or a doc's title.
fn label(r: &str, index: &Index<'_>) -> Option<String> {
    let (doc_ref, anchor) = r.split_once('#').map_or((r, None), |(f, a)| (f, Some(a)));
    let doc = index.get(doc_ref)?;
    let Some(anchor) = anchor else {
        return doc.meta_str("title").map(str::to_owned);
    };
    let line = doc
        .section(anchor)
        .map(|s| s.line)
        .or_else(|| doc.rule(anchor).map(|r| r.line))?;
    let heading = doc.text.lines().nth(line.checked_sub(1)?)?;
    Some(heading.trim_start_matches('#').trim().to_owned())
}

pub fn render(path: &str, g: &Governing, index: &Index<'_>) -> String {
    let mut out = format!("What governs {path}:\n");
    for (title, items) in [
        ("Sections (covers)", &g.sections),
        ("Rules (applies_to)", &g.rules),
        ("Process rules (applies_to = [], every PR)", &g.process),
        ("ADRs", &g.adrs),
    ] {
        let _ = writeln!(out, "\n{title}:");
        if items.is_empty() {
            out.push_str("  (none)\n");
        }
        for i in items {
            let file = index
                .get(i.split('#').next().unwrap_or_default())
                .map(|d| d.path.as_str())
                .unwrap_or_default();
            match label(i, index) {
                Some(t) => {
                    let _ = writeln!(out, "  {i}  ({t})  {file}");
                }
                None => {
                    let _ = writeln!(out, "  {i}  {file}");
                }
            }
        }
    }
    out
}
