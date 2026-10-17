//! `repo governance --base <rev> [--head <rev>] [--pr-body <file>] [--labels a,b]
//! [--author <login>] [--into <branch>] [--main <rev>]`.
//!
//! Lists the governance changes between the merge base of `<base>` and `<head>`. Both
//! sides are read as git objects only (`git show`, `git ls-tree`, `git diff`); nothing
//! from the head is executed. The rules that classify changes come from the base's
//! `repo.toml` and stack pack, so a change cannot loosen its own check.
//!
//! Governance changes: edits to `[governance].protected` and `maintainer_only` paths;
//! edits to `covers`, `applies_to`, `enforced_by` or `status` in doc metadata; edits to an
//! active rule's text; deleted docs, sections and rules (a doc moved with its `id` is not
//! deleted); any edit to an accepted ADR other than its `status`, `superseded_by` and
//! status banner; edits to the pack's `[protected_tables]` in manifests; new lint
//! suppressions (the pack's patterns).
//!
//! A change to any `R-GOV-*` rule (added, deleted, or its heading, text or metadata edited)
//! is reported as maintainer-only, as `maintainer-only: <path>#<rule id>`.
//!
//! With `--pr-body` or `--labels` it enforces: when there are changes, the PR needs the
//! `[labels].governance` label and a non-empty `## Governance change` section. An accepted
//! ADR edited beyond `status = "superseded"` with a `superseded_by` and its status banner
//! fails whatever the labels (R-DOC-008). With
//! `--author dependabot[bot]`, a diff whose every changed line re-pins a `uses:` reference
//! to a new commit SHA is exempt (ADR-0004).
//!
//! With `--into wave/<name>` (the PR's base branch), a changed file whose head version is
//! the one on `--main` (default `origin/main`) is skipped: it came from `main`, where it was
//! judged. Any other change to a protected or maintainer-only path fails whatever the
//! labels: such changes target `main`, since a wave's own workflows judge the PRs into it.

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
    /// Manifest globs and the dotted tables protected in them.
    tables: Vec<(Globs, Vec<String>)>,
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
            tables: pack
                .protected_tables
                .iter()
                .map(|(g, t)| Ok((Globs::new(&[g])?, t.clone())))
                .collect::<Result<_>>()?,
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

/// The `@doc` id of a tracked doc's text, if it has one.
fn doc_id(path: &str, text: &str) -> Option<String> {
    let d = parse(path, text, &mut Vec::new());
    let id = d.id();
    (!id.is_empty()).then(|| id.to_owned())
}

/// The tracked docs among `files`, each as one before-and-after pair: a doc deleted at one
/// path and added at another with the same `@doc` id is one pair under the new path, and the
/// added side is not listed again.
fn doc_pairs(rules: &Rules, files: &[FileChange]) -> Vec<FileChange> {
    // Tracked docs added at the head, by `@doc` id: a deleted doc whose id reappears moved.
    let added: BTreeMap<String, &FileChange> = files
        .iter()
        .filter(|f| f.base.is_none() && rules.tracked.is_tracked(&f.path))
        .filter_map(|f| Some((doc_id(&f.path, f.head.as_deref()?)?, f)))
        .collect();
    let mut targets = std::collections::BTreeSet::new();
    let mut out = Vec::new();
    for f in files.iter().filter(|f| rules.tracked.is_tracked(&f.path)) {
        let moved = match (&f.base, &f.head) {
            (Some(b), None) => doc_id(&f.path, b).and_then(|id| added.get(&id)),
            _ => None,
        };
        match moved {
            // Compare the old text with the new one, under the new path.
            Some(to) => {
                targets.insert(to.path.clone());
                out.push(FileChange {
                    path: to.path.clone(),
                    base: f.base.clone(),
                    head: to.head.clone(),
                });
            }
            None => out.push(FileChange {
                path: f.path.clone(),
                base: f.base.clone(),
                head: f.head.clone(),
            }),
        }
    }
    out.retain(|f| f.base.is_some() || !targets.contains(&f.path));
    out
}

/// Whether an id names a rule on who decides: its edits are maintainer-only.
fn governs_decisions(id: &str) -> bool {
    id.starts_with("R-GOV-")
}

/// A rule's heading, normalized text and metadata, when the doc has it.
fn rule_state(doc: &Doc, id: &str) -> Option<(Option<String>, Option<toml::Table>)> {
    let r = doc.rule(id)?;
    Some((rule_text(doc, id), r.meta.clone()))
}

/// The `R-GOV-*` rules a pair adds, deletes or edits, in id order.
fn decision_rules(f: &FileChange) -> Vec<String> {
    let parsed = |t: &Option<String>| t.as_deref().map(|t| parse(&f.path, t, &mut Vec::new()));
    let (base, head) = (parsed(&f.base), parsed(&f.head));
    let ids: std::collections::BTreeSet<String> = [&base, &head]
        .into_iter()
        .flatten()
        .flat_map(|d| d.rules.iter().map(|r| r.id.clone()))
        .filter(|id| governs_decisions(id))
        .collect();
    ids.into_iter()
        .filter(|id| {
            let state = |d: &Option<Doc>| d.as_ref().and_then(|d| rule_state(d, id));
            state(&base) != state(&head)
        })
        .collect()
}

/// Classifies every changed file; also returns the maintainer-only paths touched, and each
/// `R-GOV-*` rule changed as `<path>#<rule id>`.
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
        if let (Some(base), Some(head)) = (&f.base, &f.head) {
            for (globs, tables) in &rules.tables {
                if globs.is_match(p) {
                    table_edits(p, base, head, tables, &mut out);
                }
            }
        }
        if let Some(head) = &f.head {
            if rules.scans(p) {
                new_suppressions(rules, p, f.base.as_deref(), head, &mut out);
            }
        }
    }
    for pair in doc_pairs(rules, files) {
        doc_meta(&pair, &mut out);
        for id in decision_rules(&pair) {
            push(
                &mut out,
                &pair.path,
                format!("changes rule `{id}`, which only the maintainer approves"),
            );
            maintainer.push(format!("{}#{id}", pair.path));
        }
    }
    (out, maintainer)
}

/// Edits no label allows: an accepted ADR changes beyond becoming `superseded` with a
/// `superseded_by`, plus its generated status banner (R-DOC-008). A moved ADR is compared
/// across the move.
pub fn hard_problems(rules: &Rules, files: &[FileChange]) -> Vec<String> {
    let mut out = Vec::new();
    for f in doc_pairs(rules, files) {
        let Some(base_text) = &f.base else {
            continue;
        };
        let base = parse(&f.path, base_text, &mut Vec::new());
        if !accepted_adr(&base) {
            continue;
        }
        let p = &f.path;
        let Some(head_text) = &f.head else {
            out.push(format!(
                "{p}: deletes an accepted ADR; supersede it with a new ADR instead (R-DOC-008)"
            ));
            continue;
        };
        if base_text == head_text {
            continue;
        }
        let head = parse(p, head_text, &mut Vec::new());
        if adr_content(&base) != adr_content(&head) {
            out.push(format!(
                "{p}: edits an accepted ADR's text; a changed decision is a new ADR that supersedes it (R-DOC-008)"
            ));
        }
        let (Some(b), Some(h)) = (&base.meta, &head.meta) else {
            out.push(format!(
                "{p}: breaks an accepted ADR's metadata block (R-DOC-008)"
            ));
            continue;
        };
        let without = |t: &toml::Table| {
            let mut t = t.clone();
            t.remove("status");
            t.remove("superseded_by");
            t
        };
        if without(b) != without(h) {
            out.push(format!(
                "{p}: edits an accepted ADR's metadata other than `status` and `superseded_by` (R-DOC-008)"
            ));
        }
        let status =
            |t: &toml::Table, k: &str| t.get(k).and_then(toml::Value::as_str).map(str::to_owned);
        let same = status(b, "status") == status(h, "status")
            && b.get("superseded_by") == h.get("superseded_by");
        let superseded = status(h, "status").as_deref() == Some("superseded")
            && status(h, "superseded_by").is_some_and(|s| !s.trim().is_empty());
        if !same && !superseded {
            out.push(format!(
                "{p}: an accepted ADR's status changes only to `superseded`, with `superseded_by` (R-DOC-008)"
            ));
        }
    }
    out
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

/// The value at a dotted key path, if present.
fn at<'a>(t: &'a toml::Table, dotted: &str) -> Option<&'a toml::Value> {
    let mut parts = dotted.split('.');
    let mut v = t.get(parts.next()?)?;
    for k in parts {
        v = v.as_table()?.get(k)?;
    }
    Some(v)
}

/// Edits to protected tables in one manifest. A head that doesn't parse is reported, since
/// its tables can't be compared.
fn table_edits(p: &str, base: &str, head: &str, tables: &[String], out: &mut Vec<Change>) {
    let Ok(b) = base.parse::<toml::Table>() else {
        return;
    };
    let Ok(h) = head.parse::<toml::Table>() else {
        push(
            out,
            p,
            "does not parse as TOML, so its protected tables can't be compared".into(),
        );
        return;
    };
    for t in tables {
        if at(&b, t) != at(&h, t) {
            push(out, p, format!("edits the protected table `[{t}]`"));
        }
    }
}

/// A rule's heading and normalized text.
fn rule_text(doc: &Doc, id: &str) -> Option<String> {
    let r = doc.rule(id)?;
    let heading = doc
        .text
        .lines()
        .nth(r.line.saturating_sub(1))
        .unwrap_or_default();
    Some(format!(
        "{}\n{}",
        heading.trim_end(),
        crate::docs::hash::body(doc, Some(id))?
    ))
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
                let active = r
                    .meta
                    .as_ref()
                    .and_then(|m| m.get("status"))
                    .and_then(toml::Value::as_str)
                    == Some("active");
                if active && rule_text(&base, &r.id) != rule_text(&head, &r.id) {
                    push(out, p, format!("edits the text of active rule `{}`", r.id));
                }
            }
        }
    }
    if accepted_adr(&base) && base_text != head_text {
        adr_edits(p, &base, &head, out);
    }
}

/// An ADR's text after its metadata block, without its generated status banner.
fn adr_content(d: &Doc) -> String {
    let lines: Vec<&str> = d.text.lines().collect();
    strip_block(
        &lines[d.content_start.min(lines.len())..].join("\n"),
        STATUS,
    )
}

/// An accepted ADR may change only its `status`, `superseded_by` and status banner.
fn adr_edits(p: &str, base: &Doc, head: &Doc, out: &mut Vec<Change>) {
    if adr_content(base) != adr_content(head) {
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

/// Protected and maintainer-only paths, and `R-GOV-*` rules (`maintainer` entries holding a
/// `#`), a PR into a wave changes. They must target `main`.
pub fn wave_problems(
    rules: &Rules,
    files: &[FileChange],
    maintainer: &[String],
    into: &str,
) -> Vec<String> {
    let paths = files
        .iter()
        .filter(|f| rules.protected.is_match(&f.path) || rules.maintainer_only.is_match(&f.path))
        .map(|f| f.path.as_str());
    let decision_rules = maintainer
        .iter()
        .filter(|m| m.contains('#'))
        .map(String::as_str);
    paths
        .chain(decision_rules)
        .map(|p| {
            format!(
                "{p}: a protected path changes in a PR into `{into}`; target `main` instead (docs/runbooks/waves.md#which-wave)"
            )
        })
        .collect()
}

/// The blob id of `path` at `rev`; `None` when absent there.
fn blob(root: &Path, rev: &str, path: &str) -> Option<String> {
    git::text(
        root,
        &["rev-parse", "--verify", "-q", &format!("{rev}:{path}")],
    )
    .ok()
    .map(|s| s.trim().to_owned())
}

/// The author whose pin-only PRs are exempt (ADR-0004).
pub const DEPENDABOT: &str = "dependabot[bot]";

/// Whether every changed line re-pins a `uses:` reference of the same action to a new
/// full commit SHA, with its release in a trailing comment. Files are compared line by line
/// (a re-pin never adds or removes lines); an added or deleted file is not a re-pin.
pub fn repin_only(files: &[FileChange]) -> bool {
    let pin = Regex::new(r"^(\s*(?:-\s+)?uses:\s*)([^@\s]+)@[0-9a-f]{40}\s+#\s*\S.*$")
        .expect("valid regex");
    let parts = |l: &str| pin.captures(l).map(|c| (c[1].to_owned(), c[2].to_owned()));
    !files.is_empty()
        && files.iter().all(|f| {
            let (Some(base), Some(head)) = (&f.base, &f.head) else {
                return false;
            };
            let (b, h): (Vec<&str>, Vec<&str>) = (base.lines().collect(), head.lines().collect());
            b.len() == h.len()
                && b.iter().zip(&h).all(|(x, y)| {
                    x == y || matches!((parts(x), parts(y)), (Some(px), Some(py)) if px == py)
                })
        })
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
    let usage = "repo governance --base <rev> [--head <rev>] [--pr-body <file>] [--labels a,b] [--author <login>] [--into <branch>] [--main <rev>]";
    let a = Args::parse(
        args,
        &[
            "base", "head", "pr-body", "labels", "author", "into", "main",
        ],
        &[],
    )?;
    a.exactly(0, usage)?;
    let root = git::root()?;
    let base = git::resolve(&root, a.required("base", usage)?)?;
    let head = git::resolve(&root, a.value("head").unwrap_or("HEAD"))?;
    let mb = git::merge_base(&root, &base, &head)?;
    let rules = rules_at(&root, &base)?;
    let mut files = changed_files(&root, &mb, &head)?;
    let into = a.value("into").filter(|b| b.starts_with("wave/"));
    if into.is_some() {
        let main = git::resolve(&root, a.value("main").unwrap_or("origin/main"))?;
        let mut kept = Vec::new();
        for f in files {
            if blob(&root, &head, &f.path) == blob(&root, &main, &f.path) {
                println!("governance: {}: as on main, skipped", f.path);
            } else {
                kept.push(f);
            }
        }
        files = kept;
    }
    if a.value("author") == Some(DEPENDABOT) && repin_only(&files) {
        for f in &files {
            println!("governance: {}: re-pins actions only", f.path);
        }
        println!(
            "governance: ok (a {DEPENDABOT} PR that only re-pins actions is exempt, ADR-0004)"
        );
        return Ok(true);
    }
    let (changes, maintainer) = classify(&rules, &files);
    print!("{}", render(&mb, &head, &changes, &maintainer));
    let wave = into
        .map(|b| wave_problems(&rules, &files, &maintainer, b))
        .unwrap_or_default();
    for p in &wave {
        println!("governance: {p}");
    }
    let hard = hard_problems(&rules, &files);
    for p in &hard {
        println!("governance: no label allows this: {p}");
    }
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
    if found.is_empty() && wave.is_empty() && hard.is_empty() && !changes.is_empty() {
        println!("governance: ok (labeled `{label}` and justified)");
    }
    Ok(found.is_empty() && wave.is_empty() && hard.is_empty())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn rules() -> Rules {
        let config = Config::parse(crate::config::tests::CONFIG).unwrap();
        let pack = Pack::parse(
            "p",
            &format!(
                "{}\n[protected_tables]\n\"**/Cargo.toml\" = [\"lints\", \"workspace.lints\"]\n",
                crate::config::tests::PACK.replace(
                    "patterns = ['#\\[allow\\(']",
                    "patterns = ['#\\[allow\\(', 'cfg_attr\\(.*\\b(allow|expect)\\(', '#\\[ignore']"
                )
            ),
        )
        .unwrap();
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

    const ADR: &str = "<!-- @doc\nid = \"ADR-0001\"\ntitle = \"T\"\nkind = \"adr\"\nstatus = \"accepted\"\ntier = \"agent\"\n-->\n\n# ADR-0001: T\n\n<!-- @generated begin adr-status -->\n> **Status:** accepted\n<!-- @generated end adr-status -->\n\nDecision.\n";

    fn hard(files: &[FileChange]) -> Vec<String> {
        hard_problems(&rules(), files)
    }

    #[test]
    fn accepted_adr_edits_fail_whatever_the_labels() {
        let path = "docs/adr/0001-t.md";
        let superseded = ADR
            .replace(
                "status = \"accepted\"",
                "status = \"superseded\"\nsuperseded_by = \"ADR-0002\"",
            )
            .replace(
                "> **Status:** accepted",
                "> **Status:** superseded by [ADR-0002](0002-x.md)",
            );
        assert!(hard(&[change(path, Some(ADR), Some(&superseded))]).is_empty());
        assert!(hard(&[change(path, Some(ADR), Some(ADR))]).is_empty());

        let edited = ADR.replace("Decision.", "Another decision.");
        let got = hard(&[change(path, Some(ADR), Some(&edited))]);
        assert_eq!(got.len(), 1, "{got:?}");
        assert!(got[0].contains("edits an accepted ADR's text"), "{got:?}");

        // Superseding doesn't carry a content edit with it.
        let both = superseded.replace("Decision.", "Another decision.");
        assert_eq!(hard(&[change(path, Some(ADR), Some(&both))]).len(), 1);

        let tier = ADR.replace("tier = \"agent\"", "tier = \"maintainer\"");
        assert!(hard(&[change(path, Some(ADR), Some(&tier))])[0].contains("metadata other than"));

        let no_successor = ADR.replace("status = \"accepted\"", "status = \"superseded\"");
        assert!(
            hard(&[change(path, Some(ADR), Some(&no_successor))])[0]
                .contains("only to `superseded`")
        );
        let reopened = ADR.replace("status = \"accepted\"", "status = \"proposed\"");
        assert_eq!(hard(&[change(path, Some(ADR), Some(&reopened))]).len(), 1);

        assert!(hard(&[change(path, Some(ADR), None)])[0].contains("deletes an accepted ADR"));

        // A move with the same text is not an edit; a move with an edit is.
        let moved = [
            change(path, Some(ADR), None),
            change("docs/adr/0001-u.md", None, Some(ADR)),
        ];
        assert!(hard(&moved).is_empty(), "{:?}", hard(&moved));
        let moved_edited = [
            change(path, Some(ADR), None),
            change("docs/adr/0001-u.md", None, Some(&edited)),
        ];
        assert_eq!(hard(&moved_edited).len(), 1);

        // Proposed ADRs, new ADRs and other docs are free to change.
        let proposed = ADR.replace("status = \"accepted\"", "status = \"proposed\"");
        let proposed_edit = proposed.replace("Decision.", "Better decision.");
        assert!(hard(&[change(path, Some(&proposed), Some(&proposed_edit))]).is_empty());
        assert!(hard(&[change(path, None, Some(ADR))]).is_empty());
        let other = DOC.replace("Text.", "Other text.");
        assert!(hard(&[change("docs/D.md", Some(DOC), Some(&other))]).is_empty());
    }

    const GOV: &str = "<!-- @doc\nid = \"RULES\"\ntitle = \"R\"\nkind = \"normative\"\nstatus = \"active\"\n-->\n<a id=\"governance\"></a>\n## Governance\n<!-- @section\n-->\n\n<a id=\"R-GOV-001\"></a>\n### R-GOV-001: Labels\n<!-- @rule\nstatus = \"active\"\nenforced_by = [\"review\"]\napplies_to = []\n-->\nOnly the maintainer applies it.\n\n<a id=\"R-DOC-001\"></a>\n### R-DOC-001: Docs\n<!-- @rule\nstatus = \"active\"\nenforced_by = [\"review\"]\napplies_to = []\n-->\nDocs.\n";

    #[test]
    fn decision_rule_edits_are_maintainer_only() {
        let path = "docs/RULES.md";
        let maintainer = |head: &str| classify(&rules(), &[change(path, Some(GOV), Some(head))]).1;
        let loosened = GOV.replace("Only the maintainer applies it.", "Anyone applies it.");
        assert_eq!(
            maintainer(&loosened),
            vec!["docs/RULES.md#R-GOV-001".to_owned()]
        );
        assert!(
            whats(&[change(path, Some(GOV), Some(&loosened))])
                .iter()
                .any(|w| w.contains("`R-GOV-001`, which only the maintainer approves"))
        );
        let retitled = GOV.replace("### R-GOV-001: Labels", "### R-GOV-001: Labels, loosely");
        assert_eq!(maintainer(&retitled).len(), 1);
        let retired = GOV.replacen(
            "status = \"active\"\nenforced_by",
            "status = \"retired\"\nenforced_by",
            1,
        );
        assert_eq!(maintainer(&retired).len(), 1);
        let added = GOV.replace(
            "<a id=\"R-DOC-001\"></a>",
            "<a id=\"R-GOV-002\"></a>\n### R-GOV-002: New\n<!-- @rule\nstatus = \"active\"\nenforced_by = [\"review\"]\napplies_to = []\n-->\nNew.\n\n<a id=\"R-DOC-001\"></a>",
        );
        assert_eq!(
            maintainer(&added),
            vec!["docs/RULES.md#R-GOV-002".to_owned()]
        );
        // Other rules and prose outside the rule stay agent-labelled.
        let other = GOV.replace("Docs.\n", "Docs, edited.\n");
        assert!(maintainer(&other).is_empty());
        let prose = GOV.replace(
            "## Governance\n<!-- @section\n-->\n",
            "## Governance\n<!-- @section\n-->\nIntro.\n",
        );
        assert!(maintainer(&prose).is_empty(), "{:?}", maintainer(&prose));
        // Into a wave, a decision rule change fails like a protected path.
        let files = [change(path, Some(GOV), Some(&loosened))];
        let m = classify(&rules(), &files).1;
        assert_eq!(wave_problems(&rules(), &files, &m, "wave/w1").len(), 1);
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
    fn active_rule_text_edits_are_flagged() {
        let edited = DOC.replace("Rule.", "Rule, unless it is inconvenient.");
        assert_eq!(
            whats(&[change("docs/D.md", Some(DOC), Some(&edited))]),
            vec!["docs/D.md: edits the text of active rule `R-X-001`"]
        );
        let retitled = DOC.replace("### R-X-001: R", "### R-X-001: Rarely");
        assert_eq!(
            whats(&[change("docs/D.md", Some(DOC), Some(&retitled))]).len(),
            1
        );
        let retired = DOC.replace(
            "status = \"active\"\nenforced_by",
            "status = \"retired\"\nenforced_by",
        );
        let retired_edit = retired.replace("Rule.", "Old rule.");
        assert!(whats(&[change("docs/D.md", Some(&retired), Some(&retired_edit))]).is_empty());
    }

    #[test]
    fn a_moved_doc_is_not_deleted() {
        let moved = [
            change("docs/D.md", Some(DOC), None),
            change("docs/guides/D.md", None, Some(DOC)),
        ];
        assert!(whats(&moved).is_empty(), "{:?}", whats(&moved));
        let narrowed = DOC.replace("src/**", "src/a/**");
        let moved_and_narrowed = [
            change("docs/D.md", Some(DOC), None),
            change("docs/guides/D.md", None, Some(&narrowed)),
        ];
        assert_eq!(
            whats(&moved_and_narrowed),
            vec!["docs/guides/D.md: section `#a` `covers`: [\"src/**\"] → [\"src/a/**\"]"]
        );
        let other = DOC.replace("id = \"D\"", "id = \"E\"");
        let replaced = [
            change("docs/D.md", Some(DOC), None),
            change("docs/E.md", None, Some(&other)),
        ];
        assert_eq!(
            whats(&replaced),
            vec!["docs/D.md: deletes a tracked doc (retire it instead)"]
        );
    }

    #[test]
    fn protected_manifest_tables() {
        let base = "[package]\nname = \"a\"\n\n[workspace.lints.clippy]\nunwrap_used = \"deny\"\n";
        let loosened = base.replace("\"deny\"", "\"allow\"");
        assert_eq!(
            whats(&[change("Cargo.toml", Some(base), Some(&loosened))]),
            vec!["Cargo.toml: edits the protected table `[workspace.lints]`"]
        );
        let member = "[package]\nname = \"b\"\n\n[lints]\nworkspace = true\n";
        let dropped = "[package]\nname = \"b\"\n";
        assert_eq!(
            whats(&[change("crates/b/Cargo.toml", Some(member), Some(dropped))]),
            vec!["crates/b/Cargo.toml: edits the protected table `[lints]`"]
        );
        let bumped = base.replace("name = \"a\"", "name = \"a\"\nversion = \"0.2.0\"");
        assert!(whats(&[change("Cargo.toml", Some(base), Some(&bumped))]).is_empty());
        assert_eq!(
            whats(&[change("Cargo.toml", Some(base), Some("[lints"))]).len(),
            1
        );
    }

    #[test]
    fn suppressions_in_cfg_attr_and_ignored_tests() {
        let head = "#[cfg_attr(all(), allow(clippy::unwrap_used))]\nfn a() {}\n#[test]\n#[ignore]\nfn t() {}\n";
        let got = whats(&[change("src/a.rs", Some(""), Some(head))]);
        assert_eq!(got.len(), 2, "{got:?}");
        assert!(
            got[0].contains("line 1") && got[1].contains("line 4"),
            "{got:?}"
        );
    }

    #[test]
    fn dependabot_repins() {
        let a = "    steps:\n      - uses: actions/checkout@3d3c42e5aac5ba805825da76410c181273ba90b1 # v7.0.1\n        with:\n          ref: x\n";
        let b = a.replace(
            "3d3c42e5aac5ba805825da76410c181273ba90b1 # v7.0.1",
            "1111111111111111111111111111111111111111 # v7.1.0",
        );
        assert!(repin_only(&[change(
            ".github/workflows/ci.yml",
            Some(a),
            Some(&b)
        )]));
        let other_action = b.replace("actions/checkout", "evil/checkout");
        assert!(!repin_only(&[change(
            ".github/workflows/ci.yml",
            Some(a),
            Some(&other_action)
        )]));
        let tag = a.replace("3d3c42e5aac5ba805825da76410c181273ba90b1 # v7.0.1", "v7");
        assert!(!repin_only(&[change(
            ".github/workflows/ci.yml",
            Some(a),
            Some(&tag)
        )]));
        let extra = b.replace("ref: x", "ref: y");
        assert!(!repin_only(&[change(
            ".github/workflows/ci.yml",
            Some(a),
            Some(&extra)
        )]));
        let added_line = format!("{b}      - run: curl x | sh\n");
        assert!(!repin_only(&[change(
            ".github/workflows/ci.yml",
            Some(a),
            Some(&added_line)
        )]));
        assert!(!repin_only(&[change(
            ".github/workflows/new.yml",
            None,
            Some(&b)
        )]));
        assert!(!repin_only(&[]));
    }

    #[test]
    fn protected_paths_into_a_wave_fail() {
        let files = [
            change(".github/workflows/ci.yml", Some("a"), Some("b")),
            change("REVIEW.md", Some("a"), Some("b")),
            change("src/a.rs", Some("a"), Some("b")),
        ];
        let got = wave_problems(&rules(), &files, &[], "wave/w1");
        assert_eq!(got.len(), 2, "{got:?}");
        assert!(got[0].contains("into `wave/w1`"), "{got:?}");
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
