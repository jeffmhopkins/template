//! `repo governance --base <rev> [--head <rev>] [--pr-body <file>] [--labels a,b]`.
//!
//! Lists the governance changes between the merge base of `<base>` and `<head>`. Both
//! sides are read as git objects only (`git show`, `git ls-tree`, `git diff`); nothing
//! from the head is executed. The rules that classify changes come from the base's
//! `repo.toml` and stack pack, so a change cannot loosen its own check.
//!
//! Governance changes: edits to `[governance].protected` and `maintainer_only` paths;
//! edits to `covers`, `applies_to`, `enforced_by` or `status` in doc metadata; deleted
//! docs, sections and rules; any edit to an accepted ADR other than its `status`,
//! `superseded_by` and status banner; new lint suppressions (the pack's patterns).
//!
//! With `--pr-body` or `--labels` it enforces: when there are changes, the PR needs the
//! `[labels].governance` label and a non-empty `## Governance change` section.

use std::collections::BTreeMap;
use std::fmt::Write as _;
use std::path::Path;

use anyhow::{Context, Result};
use regex::Regex;

use crate::Outcome;
use crate::args::Args;
use crate::config::{self, Config, Pack};
use crate::docs::generate::{STATUS, strip_block};
use crate::docs::parse::{Doc, parse};
use crate::git;
use crate::glob::Globs;

/// The PR description section that justifies governance changes.
pub const SECTION: &str = "Governance change";

/// Metadata keys whose edits are governance changes.
const KEYS: &[&str] = &["covers", "applies_to", "enforced_by", "status"];

/// One changed file: contents at the merge base and at the head (`None`: absent).
#[derive(Debug)]
pub struct FileChange {
    pub path: String,
    pub base: Option<String>,
    pub head: Option<String>,
}

/// One governance change.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Change {
    pub path: String,
    pub what: String,
}

/// The classification rules, from the base revision.
pub struct Rules {
    protected: Globs,
    maintainer_only: Globs,
    tracked: crate::docs::Tracked,
    patterns: Vec<Regex>,
    reason_required: bool,
    scanned: Option<Globs>,
}

impl Rules {
    pub fn new(config: &Config, pack: &Pack) -> Result<Self> {
        let patterns = pack
            .suppressions
            .patterns
            .iter()
            .map(|p| Regex::new(p).with_context(|| format!("suppression pattern `{p}`")))
            .collect::<Result<_>>()?;
        Ok(Self {
            protected: Globs::new(&config.governance.protected)?,
            maintainer_only: Globs::new(&config.governance.maintainer_only)?,
            tracked: crate::docs::Tracked::new(config)?,
            patterns,
            reason_required: pack.suppressions.reason_required,
            scanned: if pack.suppressions.files.is_empty() {
                None
            } else {
                Some(Globs::new(&pack.suppressions.files)?)
            },
        })
    }

    fn scans(&self, path: &str) -> bool {
        match &self.scanned {
            Some(g) => g.is_match(path),
            None => !path.ends_with(".md"),
        }
    }
}

fn push(out: &mut Vec<Change>, path: &str, what: String) {
    out.push(Change {
        path: path.to_owned(),
        what,
    });
}

/// Classifies every changed file; also returns the maintainer-only paths touched.
pub fn classify(rules: &Rules, files: &[FileChange]) -> (Vec<Change>, Vec<String>) {
    let mut out = Vec::new();
    let mut maintainer = Vec::new();
    for f in files {
        let p = f.path.as_str();
        let verb = match (&f.base, &f.head) {
            (None, _) => "adds",
            (_, None) => "deletes",
            _ => "edits",
        };
        if rules.maintainer_only.is_match(p) {
            maintainer.push(p.to_owned());
        }
        if rules.protected.is_match(p) {
            push(&mut out, p, format!("{verb} a protected path"));
        } else if rules.maintainer_only.is_match(p) {
            push(&mut out, p, format!("{verb} a maintainer-only path"));
        }
        if rules.tracked.is_tracked(p) {
            doc_meta(f, &mut out);
        }
        if let Some(head) = &f.head {
            if rules.scans(p) {
                new_suppressions(rules, p, f.base.as_deref(), head, &mut out);
            }
        }
    }
    (out, maintainer)
}

fn show(v: Option<&toml::Value>) -> String {
    v.map_or_else(|| "(none)".to_owned(), ToString::to_string)
}

/// Edits of `KEYS` between two metadata tables.
fn key_edits(base: Option<&toml::Table>, head: Option<&toml::Table>) -> Vec<String> {
    KEYS.iter()
        .filter_map(|k| {
            let (b, h) = (base.and_then(|t| t.get(*k)), head.and_then(|t| t.get(*k)));
            (b != h).then(|| format!("`{k}`: {} → {}", show(b), show(h)))
        })
        .collect()
}

fn accepted_adr(doc: &Doc) -> bool {
    doc.is_adr() && doc.status() == "accepted"
}

/// Governance-relevant changes in one tracked doc.
fn doc_meta(f: &FileChange, out: &mut Vec<Change>) {
    let Some(base_text) = &f.base else {
        return;
    };
    let p = f.path.as_str();
    let base = parse(p, base_text, &mut Vec::new());
    let Some(head_text) = &f.head else {
        push(out, p, "deletes a tracked doc (retire it instead)".into());
        if accepted_adr(&base) {
            push(out, p, "deletes an accepted ADR".into());
        }
        return;
    };
    let head = parse(p, head_text, &mut Vec::new());
    if base.meta.is_some() && head.meta.is_some() {
        for e in key_edits(base.meta.as_ref(), head.meta.as_ref()) {
            push(out, p, format!("`@doc` {e}"));
        }
    } else if base.meta.is_some() {
        push(out, p, "breaks the `@doc` block".into());
    }
    for s in &base.sections {
        match head.section(&s.anchor) {
            None => push(out, p, format!("deletes section `#{}`", s.anchor)),
            Some(h) => {
                for e in key_edits(s.meta.as_ref(), h.meta.as_ref()) {
                    push(out, p, format!("section `#{}` {e}", s.anchor));
                }
            }
        }
    }
    for r in &base.rules {
        match head.rule(&r.id) {
            None => push(out, p, format!("deletes rule `{}`", r.id)),
            Some(h) => {
                for e in key_edits(r.meta.as_ref(), h.meta.as_ref()) {
                    push(out, p, format!("rule `{}` {e}", r.id));
                }
            }
        }
    }
    if accepted_adr(&base) && base_text != head_text {
        adr_edits(p, &base, &head, out);
    }
}

/// An accepted ADR may change only its `status`, `superseded_by` and status banner.
fn adr_edits(p: &str, base: &Doc, head: &Doc, out: &mut Vec<Change>) {
    let content = |d: &Doc| {
        let lines: Vec<&str> = d.text.lines().collect();
        strip_block(
            &lines[d.content_start.min(lines.len())..].join("\n"),
            STATUS,
        )
    };
    if content(base) != content(head) {
        push(out, p, "edits an accepted ADR's content".into());
    }
    match (&base.meta, &head.meta) {
        (Some(b), Some(h)) => {
            let keys: std::collections::BTreeSet<&String> = b.keys().chain(h.keys()).collect();
            for k in keys {
                if !matches!(k.as_str(), "status" | "superseded_by") && b.get(k) != h.get(k) {
                    push(out, p, format!("edits an accepted ADR's `{k}`"));
                }
            }
        }
        _ => push(out, p, "edits an accepted ADR's metadata block".into()),
    }
}

/// Suppression lines in `head` that `base` doesn't have (compared as a multiset of
/// whitespace-collapsed lines).
fn new_suppressions(rules: &Rules, p: &str, base: Option<&str>, head: &str, out: &mut Vec<Change>) {
    if rules.patterns.is_empty() {
        return;
    }
    let hits = |text: &str| -> Vec<(String, usize)> {
        text.lines()
            .enumerate()
            .filter(|(_, l)| rules.patterns.iter().any(|r| r.is_match(l)))
            .map(|(i, l)| (l.split_whitespace().collect::<Vec<_>>().join(" "), i + 1))
            .collect()
    };
    let mut old: BTreeMap<String, usize> = BTreeMap::new();
    for (l, _) in hits(base.unwrap_or_default()) {
        *old.entry(l).or_default() += 1;
    }
    for (l, line) in hits(head) {
        if let Some(n) = old.get_mut(&l).filter(|n| **n > 0) {
            *n -= 1;
            continue;
        }
        let missing = rules.reason_required && !(l.contains("reason =") || l.contains("reason="));
        let why = if missing {
            " (missing `reason = \"...\"`)"
        } else {
            ""
        };
        push(out, p, format!("line {line}: new suppression `{l}`{why}"));
    }
}

/// The text of the description's first `## <title>` section, HTML comments removed and
/// trimmed; empty when absent. The title matches case-insensitively as a prefix of the
/// heading; the section ends at the next `## ` heading.
pub fn section(body: &str, title: &str) -> String {
    let lines: Vec<&str> = body.lines().collect();
    let title = title.to_lowercase();
    let start = lines.iter().position(|l| {
        l.strip_prefix("## ")
            .is_some_and(|t| t.trim().to_lowercase().starts_with(&title))
    });
    let Some(start) = start else {
        return String::new();
    };
    let end = (start + 1..lines.len())
        .find(|&k| lines[k].starts_with("## ") || lines[k].trim_end() == "##")
        .unwrap_or(lines.len());
    let mut text = lines[start + 1..end].join("\n");
    while let Some(open) = text.find("<!--") {
        let Some(close) = text[open..].find("-->") else {
            text.truncate(open);
            break;
        };
        text.replace_range(open..open + close + 3, "");
    }
    text.trim().to_owned()
}

/// The files changed between two commits, renames split into a delete and an add, with
/// both versions read from the object store.
pub fn changed_files(root: &Path, from: &str, to: &str) -> Result<Vec<FileChange>> {
    let out = git::run(
        root,
        &["diff", "--name-status", "-z", "--no-renames", from, to],
    )?;
    let out = String::from_utf8_lossy(&out).into_owned();
    let mut fields = out.split('\0').filter(|s| !s.is_empty());
    let mut files = Vec::new();
    while let (Some(status), Some(path)) = (fields.next(), fields.next()) {
        let base = if status.starts_with('A') {
            None
        } else {
            git::show(root, from, path)?
        };
        let head = if status.starts_with('D') {
            None
        } else {
            git::show(root, to, path)?
        };
        files.push(FileChange {
            path: path.to_owned(),
            base,
            head,
        });
    }
    Ok(files)
}

/// The config and pack at `rev`, else the working tree's.
fn rules_at(root: &Path, rev: &str) -> Result<Rules> {
    let config = match git::show(root, rev, config::FILE)? {
        Some(t) => Config::parse(&t)?,
        None => Config::load(root)?,
    };
    let path = config.pack_path();
    let pack = match git::show(root, rev, &path)? {
        Some(t) => Pack::parse(&path, &t)?,
        None => Pack::load(root, &config)?,
    };
    Rules::new(&config, &pack)
}

/// The label name, from the base config.
fn label_at(root: &Path, rev: &str) -> Result<String> {
    let config = match git::show(root, rev, config::FILE)? {
        Some(t) => Config::parse(&t)?,
        None => Config::load(root)?,
    };
    Ok(config.labels.governance)
}

pub fn render(from: &str, to: &str, changes: &[Change], maintainer: &[String]) -> String {
    let short = |s: &str| s.get(..12).unwrap_or(s).to_owned();
    let mut out = String::new();
    if changes.is_empty() {
        let _ = writeln!(
            out,
            "governance: no governance changes between {} and {}",
            short(from),
            short(to)
        );
    } else {
        let _ = writeln!(
            out,
            "governance: {} governance change(s) between {} and {}:",
            changes.len(),
            short(from),
            short(to)
        );
        for c in changes {
            let _ = writeln!(out, "  {}: {}", c.path, c.what);
        }
    }
    for m in maintainer {
        let _ = writeln!(out, "maintainer-only: {m}");
    }
    out
}

/// What blocks a PR with these changes; empty when it may pass.
pub fn problems(changes: &[Change], labels: &[String], body: &str, label: &str) -> Vec<String> {
    if changes.is_empty() {
        return Vec::new();
    }
    let mut out = Vec::new();
    if !labels.iter().any(|l| l == label) {
        out.push(format!("the PR lacks the `{label}` label"));
    }
    if section(body, SECTION).is_empty() {
        out.push(format!(
            "the PR description has no non-empty \"## {SECTION}\" section (HTML comments don't count)"
        ));
    }
    out
}

pub fn run(args: &[String]) -> Outcome {
    let usage = "repo governance --base <rev> [--head <rev>] [--pr-body <file>] [--labels a,b]";
    let a = Args::parse(args, &["base", "head", "pr-body", "labels"], &[])?;
    a.exactly(0, usage)?;
    let root = git::root()?;
    let base = git::resolve(&root, a.required("base", usage)?)?;
    let head = git::resolve(&root, a.value("head").unwrap_or("HEAD"))?;
    let mb = git::merge_base(&root, &base, &head)?;
    let rules = rules_at(&root, &base)?;
    let files = changed_files(&root, &mb, &head)?;
    let (changes, maintainer) = classify(&rules, &files);
    print!("{}", render(&mb, &head, &changes, &maintainer));
    if a.value("pr-body").is_none() && a.value("labels").is_none() {
        return Ok(true);
    }
    let body = match a.value("pr-body") {
        Some(p) => std::fs::read_to_string(p).with_context(|| format!("reading {p}"))?,
        None => String::new(),
    };
    let labels: Vec<String> = a
        .value("labels")
        .unwrap_or_default()
        .split(',')
        .map(str::trim)
        .filter(|l| !l.is_empty())
        .map(str::to_owned)
        .collect();
    let label = label_at(&root, &base)?;
    let found = problems(&changes, &labels, &body, &label);
    for p in &found {
        println!("governance: {p}");
    }
    if found.is_empty() && !changes.is_empty() {
        println!("governance: ok (labeled `{label}` and justified)");
    }
    Ok(found.is_empty())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn rules() -> Rules {
        let config = Config::parse(crate::config::tests::CONFIG).unwrap();
        let pack = Pack::parse("p", crate::config::tests::PACK).unwrap();
        Rules::new(&config, &pack).unwrap()
    }

    fn change(path: &str, base: Option<&str>, head: Option<&str>) -> FileChange {
        FileChange {
            path: path.into(),
            base: base.map(str::to_owned),
            head: head.map(str::to_owned),
        }
    }

    fn whats(files: &[FileChange]) -> Vec<String> {
        classify(&rules(), files)
            .0
            .into_iter()
            .map(|c| format!("{}: {}", c.path, c.what))
            .collect()
    }

    const DOC: &str = "<!-- @doc\nid = \"D\"\ntitle = \"D\"\nkind = \"guide\"\nstatus = \"active\"\n-->\n<a id=\"a\"></a>\n## A\n<!-- @section\ncovers = [\"src/**\"]\n-->\nText.\n\n<a id=\"R-X-001\"></a>\n### R-X-001: R\n<!-- @rule\nstatus = \"active\"\nenforced_by = [\"review\"]\n-->\nRule.\n";

    #[test]
    fn protected_and_maintainer_paths() {
        let (c, m) = classify(
            &rules(),
            &[
                change(".github/workflows/ci.yml", Some("a"), Some("b")),
                change("REVIEW.md", None, Some("x")),
                change("src/a.rs", Some("a"), Some("b")),
            ],
        );
        assert_eq!(c.len(), 2, "{c:?}");
        assert_eq!(m, vec!["REVIEW.md".to_owned()]);
    }

    #[test]
    fn metadata_edits_and_deletions() {
        let narrowed = DOC.replace("src/**", "src/a/**");
        let retired = DOC.replace(
            "status = \"active\"\nenforced_by",
            "status = \"retired\"\nenforced_by",
        );
        let no_section = DOC.replace(
            "<a id=\"a\"></a>\n## A\n<!-- @section\ncovers = [\"src/**\"]\n-->\nText.\n",
            "",
        );
        let prose = DOC.replace("Text.", "Other text.");
        assert_eq!(
            whats(&[change("docs/D.md", Some(DOC), Some(&narrowed))]),
            vec!["docs/D.md: section `#a` `covers`: [\"src/**\"] → [\"src/a/**\"]"]
        );
        assert_eq!(
            whats(&[change("docs/D.md", Some(DOC), Some(&retired))]).len(),
            1
        );
        assert_eq!(
            whats(&[change("docs/D.md", Some(DOC), Some(&no_section))]),
            vec!["docs/D.md: deletes section `#a`"]
        );
        assert!(whats(&[change("docs/D.md", Some(DOC), Some(&prose))]).is_empty());
        assert_eq!(whats(&[change("docs/D.md", Some(DOC), None)]).len(), 1);
        assert!(whats(&[change("docs/D.md", None, Some(DOC))]).is_empty());
    }

    #[test]
    fn accepted_adrs_change_only_status() {
        let adr = "<!-- @doc\nid = \"ADR-0001\"\ntitle = \"T\"\nkind = \"adr\"\nstatus = \"accepted\"\ntier = \"agent\"\n-->\n\n# ADR-0001: T\n\n<!-- @generated begin adr-status -->\n> **Status:** accepted\n<!-- @generated end adr-status -->\n\nDecision.\n";
        let superseded = adr
            .replace(
                "status = \"accepted\"",
                "status = \"superseded\"\nsuperseded_by = \"ADR-0002\"",
            )
            .replace(
                "> **Status:** accepted",
                "> **Status:** superseded by [ADR-0002](0002-x.md)",
            );
        let got = whats(&[change("docs/adr/0001-t.md", Some(adr), Some(&superseded))]);
        assert_eq!(
            got,
            vec!["docs/adr/0001-t.md: `@doc` `status`: \"accepted\" → \"superseded\""]
        );
        let edited = adr.replace("Decision.", "Another decision.");
        assert_eq!(
            whats(&[change("docs/adr/0001-t.md", Some(adr), Some(&edited))]),
            vec!["docs/adr/0001-t.md: edits an accepted ADR's content"]
        );
        let tier = adr.replace("tier = \"agent\"", "tier = \"maintainer\"");
        assert_eq!(
            whats(&[change("docs/adr/0001-t.md", Some(adr), Some(&tier))]),
            vec!["docs/adr/0001-t.md: edits an accepted ADR's `tier`"]
        );
    }

    #[test]
    fn new_suppressions_are_flagged() {
        let base = "#[allow(dead_code, reason = \"x\")]\nfn a() {}\n";
        let head =
            format!("{base}#[allow(unused)]\nfn b() {{}}\n#[allow(dead_code, reason = \"x\")]\n");
        let got = whats(&[change("src/a.rs", Some(base), Some(&head))]);
        assert_eq!(got.len(), 2, "{got:?}");
        assert!(
            got[0].contains("line 3") && got[0].contains("missing `reason"),
            "{got:?}"
        );
        assert!(!got[1].contains("missing"), "{got:?}");
        assert!(whats(&[change("docs/x.md", None, Some("#[allow(x)]\n"))]).is_empty());
    }

    #[test]
    fn pr_needs_label_and_section() {
        let c = vec![Change {
            path: "a".into(),
            what: "b".into(),
        }];
        let label = "governance-approved";
        let ok_body = "## Summary\nx\n\n## Governance change\nNarrows covers to the moved module.\n\n## Tests\n";
        assert!(problems(&c, &[label.into()], ok_body, label).is_empty());
        assert_eq!(problems(&c, &[], ok_body, label).len(), 1);
        let empty = "## Governance change\n<!-- explain -->\n\n## Tests\n";
        assert_eq!(problems(&c, &[label.into()], empty, label).len(), 1);
        assert!(problems(&[], &[], "", label).is_empty());
        assert_eq!(section("## governance changes\nwhy\n", SECTION), "why");
    }
}
