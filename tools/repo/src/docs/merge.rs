//! `docs merge-main [--from <branch>]`: merges `origin/<branch>` (default `main`) into the
//! current branch and resolves two kinds of conflict:
//!
//! - **Generated blocks.** A conflict that lies wholly inside `@generated` blocks takes the
//!   merged branch's side, then every block is rewritten from the merged sources.
//! - **The ledger.** Each conflicting entry is resolved by recomputing the section's
//!   inputs on the merged tree: the side whose recorded hash matches the merged input is
//!   kept, with its note; when neither matches, the entry is dropped and the section is
//!   reported stale.
//! - **Lockfiles** the stack pack names in `[lockfiles]`: once every other conflict is
//!   resolved, the file takes the merged branch's side and the pack's relock command runs in
//!   its directory, so the branch's own dependencies are added back on top of the merged ones.
//!
//! Every other conflict is left for the agent. Afterwards it lists the stale sections, each
//! with the notes this branch had written on it, for the re-read.

use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;

use anyhow::{Result, bail};

use super::ledger::{Entry, Ledger, Stamp};
use super::{Loaded, generate, hash, load, strings};
use crate::config::Repo;
use crate::git;

const REMOTE: &str = "origin";
pub const DEFAULT_BRANCH: &str = "main";

/// What `docs merge-main` did.
#[derive(Debug, Default)]
pub struct Merge {
    /// Sections whose ledger entries conflicted, as `ID#anchor`.
    pub stamps: Vec<String>,
    /// Files whose conflicts were only inside generated blocks.
    pub generated: Vec<String>,
    /// Lockfiles resolved by relocking, with the command that ran.
    pub relocked: Vec<(String, String)>,
    /// Conflicted files left for the agent, with the reason.
    pub refused: Vec<(String, String)>,
    pub committed: bool,
    /// Sections stale after the merge.
    pub stale: Vec<String>,
    /// For each stale section, the notes this branch wrote on it.
    pub notes: BTreeMap<String, Vec<String>>,
}

/// Merges `origin/<branch>` into the current branch of the repository at `root`.
pub fn merge_main(root: &Path, branch: &str) -> Result<Merge> {
    let branch = branch.strip_prefix("origin/").unwrap_or(branch);
    if branch.is_empty() || branch.starts_with('-') || branch.ends_with('/') {
        bail!("`{branch}` is not a branch name");
    }
    if git::succeeds(root, &["rev-parse", "-q", "--verify", "MERGE_HEAD"]) {
        bail!("a merge is already in progress; finish or abort it first");
    }
    if !git::run(root, &["status", "--porcelain", "--untracked-files=no"])?.is_empty() {
        bail!("the working tree has uncommitted changes; commit or stash them first");
    }
    let repo = Repo::at(root.to_path_buf())?;
    let ledger_path = repo.config.docs.ledger.clone();
    git::run(root, &["fetch", "-q", REMOTE, branch])?;
    let upstream = format!("{REMOTE}/{branch}");
    let ours = git::resolve(root, "HEAD")?;
    let theirs = git::resolve(root, &upstream)?;
    let out = git::raw(
        root,
        &[
            "-c",
            "merge.conflictStyle=merge",
            "merge",
            "--no-edit",
            "-q",
            &upstream,
        ],
    )?;
    let mut report = Merge::default();
    if !out.status.success() {
        if !git::succeeds(root, &["rev-parse", "-q", "--verify", "MERGE_HEAD"]) {
            bail!(
                "`git merge {upstream}` failed: {}",
                String::from_utf8_lossy(&out.stderr).trim()
            );
        }
        resolve_conflicts(&repo, &ledger_path, &mut report)?;
        if report.refused.is_empty() {
            git::run(root, &["commit", "-q", "--no-edit"])?;
            report.committed = true;
        }
    } else {
        report.committed = true;
    }
    let l = load(&repo)?;
    report.stale = if report.committed {
        stale_sections(&l)?
    } else {
        report.stamps.clone()
    };
    report.notes = branch_notes(root, &ledger_path, &ours, &theirs, &report.stale)?;
    Ok(report)
}

/// Resolves generated-block and ledger conflicts in a merge in progress, staging each
/// resolved file; refused files go to `report.refused`.
fn resolve_conflicts(repo: &Repo, ledger_path: &str, report: &mut Merge) -> Result<()> {
    let root = repo.root.as_path();
    let unmerged = git::split_z(&git::run(
        root,
        &["diff", "--name-only", "-z", "--diff-filter=U"],
    )?);
    let tracked = super::Tracked::new(&repo.config)?;
    let lock_globs: Vec<(crate::glob::Globs, &String)> = repo
        .pack
        .lockfiles
        .iter()
        .map(|(g, cmd)| Ok((crate::glob::Globs::new(&[g])?, cmd)))
        .collect::<Result<_>>()?;
    let mut locks: Vec<(String, String)> = Vec::new();
    let mut ledger_conflict = false;
    for path in &unmerged {
        let stages = git::text(root, &["ls-files", "-u", "-z", "--", path])?;
        let has = |n: &str| {
            stages
                .split('\0')
                .any(|e| e.split_once('\t').is_some_and(|(m, _)| m.ends_with(n)))
        };
        if !(has(" 2") && has(" 3")) {
            report
                .refused
                .push((path.clone(), "not modified on both sides".into()));
            continue;
        }
        if *path == ledger_path {
            ledger_conflict = true;
            continue;
        }
        if let Some((_, cmd)) = lock_globs.iter().find(|(g, _)| g.is_match(path)) {
            locks.push((path.clone(), (*cmd).clone()));
            continue;
        }
        if !tracked.is_tracked(path) {
            report.refused.push((
                path.clone(),
                "a conflict outside the ledger and generated blocks".into(),
            ));
            continue;
        }
        let text = match std::fs::read_to_string(root.join(path)) {
            Ok(t) => t,
            Err(e) => {
                report
                    .refused
                    .push((path.clone(), format!("not readable as text: {e}")));
                continue;
            }
        };
        match resolve_generated(path, &text) {
            Ok(resolved) => {
                std::fs::write(root.join(path), resolved)?;
                git::run(root, &["add", "--", path])?;
                report.generated.push(path.clone());
            }
            Err(why) => report.refused.push((path.clone(), why)),
        }
    }
    if report.refused.is_empty() && !report.generated.is_empty() {
        let l = load(repo)?;
        let body = crate::progress::block_for(&l, &["HEAD", "MERGE_HEAD"])
            .ok()
            .flatten();
        let g = generate::generate(&l, body.as_deref());
        for d in &g.problems {
            report
                .refused
                .push((d.path.clone(), format!("`docs generate`: {}", d.msg)));
        }
        for (path, _, text) in &g.files {
            std::fs::write(root.join(path), text)?;
            git::run(root, &["add", "--", path])?;
        }
    }
    for (path, cmd) in locks {
        if !report.refused.is_empty() {
            report.refused.push((
                path,
                format!(
                    "a lockfile: once the rest is resolved, take the merged side (`git checkout --theirs`) and relock with `{cmd}`"
                ),
            ));
            continue;
        }
        git::run(root, &["checkout", "--theirs", "--", &path])?;
        let dir = Path::new(&path)
            .parent()
            .map_or_else(|| root.to_path_buf(), |d| root.join(d));
        let status = std::process::Command::new("sh")
            .args(["-c", &cmd])
            .current_dir(&dir)
            .status();
        match status {
            Ok(s) if s.success() => {
                git::run(root, &["add", "--", &path])?;
                report.relocked.push((path, cmd));
            }
            Ok(s) => report
                .refused
                .push((path, format!("relock `{cmd}` failed ({s})"))),
            Err(e) => report
                .refused
                .push((path, format!("relock `{cmd}` could not run: {e}"))),
        }
    }
    if ledger_conflict {
        let text = std::fs::read_to_string(root.join(ledger_path))?;
        let l = load(repo)?;
        let current = current_inputs(&l)?;
        match resolve_ledger(&text, &current) {
            Ok((resolved, stamps)) => {
                std::fs::write(root.join(ledger_path), resolved)?;
                git::run(root, &["add", "--", ledger_path])?;
                report.stamps = stamps;
            }
            Err(why) => report.refused.push((ledger_path.to_owned(), why)),
        }
    }
    report.refused.sort();
    Ok(())
}

/// Every stamped section's inputs on the working tree, keyed `ID#anchor`.
fn current_inputs(l: &Loaded) -> Result<BTreeMap<String, BTreeMap<String, String>>> {
    let index = super::Index::new(&l.docs);
    let files = hash::Files::new(&l.root, &l.files, &l.config.docs.ledger);
    let mut out = BTreeMap::new();
    for doc in &l.docs {
        for s in &doc.sections {
            let Some(meta) = &s.meta else { continue };
            let (covers, deps) = (strings(meta, "covers"), strings(meta, "depends_on"));
            if covers.is_empty() && deps.is_empty() {
                continue;
            }
            if let Ok(i) = hash::inputs(doc, &s.anchor, &covers, &deps, &files, &index) {
                out.insert(format!("{}#{}", doc.id(), s.anchor), i);
            }
        }
    }
    Ok(out)
}

/// One conflict hunk: the 0-based line ranges its two sides occupy once resolved.
struct Hunk {
    ours: (usize, usize),
    theirs: (usize, usize),
}

/// The two sides of a conflicted file, each as if resolved wholly to that side, and the
/// hunks' line ranges in each.
fn sides(text: &str) -> Result<(String, String, Vec<Hunk>), String> {
    let (mut ours, mut theirs) = (Vec::new(), Vec::new());
    let mut hunks = Vec::new();
    // 0 both, 1 ours, 2 base (dropped), 3 theirs.
    let mut side = 0;
    let mut start = (0, 0);
    for line in text.split_inclusive('\n') {
        let bare = line.trim_end_matches(['\n', '\r']);
        let marker =
            |m: &str| bare == m || bare.strip_prefix(m).is_some_and(|r| r.starts_with(' '));
        match side {
            0 if marker("<<<<<<<") => {
                side = 1;
                start = (ours.len(), theirs.len());
            }
            1 if marker("|||||||") => side = 2,
            1 | 2 if bare == "=======" => side = 3,
            3 if marker(">>>>>>>") => {
                side = 0;
                hunks.push(Hunk {
                    ours: (start.0, ours.len()),
                    theirs: (start.1, theirs.len()),
                });
            }
            _ if marker("<<<<<<<") || marker(">>>>>>>") => {
                return Err("malformed conflict markers".to_owned());
            }
            0 => {
                ours.push(line);
                theirs.push(line);
            }
            1 => ours.push(line),
            2 => {}
            _ => theirs.push(line),
        }
    }
    if side != 0 {
        return Err("malformed conflict markers".to_owned());
    }
    if hunks.is_empty() {
        return Err("no conflict markers".to_owned());
    }
    Ok((ours.concat(), theirs.concat(), hunks))
}

/// Resolves a conflicted ledger. For each entry the sides disagree on, keeps the side whose
/// hash matches the section's input on the merged tree (`current`); when neither matches,
/// or the input is gone, drops the entry. Returns the canonical text and
/// the sections whose entries conflicted.
pub fn resolve_ledger(
    text: &str,
    current: &BTreeMap<String, BTreeMap<String, String>>,
) -> Result<(String, Vec<String>), String> {
    let (ours, theirs, _) = sides(text)?;
    let parse =
        |t: &str, side: &str| Ledger::parse(t).map_err(|e| format!("{side} doesn't parse: {e}"));
    let (ours, theirs) = (
        parse(&ours, "this branch's side")?,
        parse(&theirs, "the merged side")?,
    );
    let keys: BTreeSet<&String> = ours.sections.keys().chain(theirs.sections.keys()).collect();
    let mut merged = Ledger::default();
    let mut conflicted = Vec::new();
    for key in keys {
        let (o, t) = (ours.sections.get(key), theirs.sections.get(key));
        if o == t {
            if let Some(s) = o {
                merged.sections.insert(key.clone(), s.clone());
            }
            continue;
        }
        conflicted.push(key.clone());
        let empty = Stamp::new();
        let (o, t) = (o.unwrap_or(&empty), t.unwrap_or(&empty));
        let cur = current.get(key);
        let inputs: BTreeSet<&String> = o.keys().chain(t.keys()).collect();
        let mut stamp = Stamp::new();
        for input in inputs {
            let (eo, et) = (o.get(input), t.get(input));
            let want = cur.and_then(|c| c.get(input));
            let matches = |e: Option<&Entry>| e.is_some_and(|e| Some(&e.hash) == want);
            let pick = if eo == et || matches(et) {
                et
            } else if matches(eo) {
                eo
            } else {
                None
            };
            if let Some(e) = pick {
                stamp.insert(input.clone(), e.clone());
            }
        }
        merged.sections.insert(key.clone(), stamp);
    }
    Ok((merged.render(), conflicted))
}

/// Resolves a file whose every conflict lies inside generated blocks on both sides, to the
/// merged branch's side; or says why it can't.
pub fn resolve_generated(path: &str, text: &str) -> Result<String, String> {
    let (ours, theirs, hunks) = sides(text)?;
    for (side, is_theirs) in [(&ours, false), (&theirs, true)] {
        let doc = super::parse::parse(path, side, &mut Vec::new());
        let inside: Vec<bool> = {
            let mut mask = vec![false; side.lines().count()];
            for g in doc
                .generated
                .iter()
                .filter(|g| generate::NAMES.contains(&g.name.as_str()))
            {
                for m in mask.iter_mut().take(g.end).skip(g.begin + 1) {
                    *m = true;
                }
            }
            mask
        };
        for h in &hunks {
            let (a, b) = if is_theirs { h.theirs } else { h.ours };
            if (a..b).any(|k| !inside.get(k).copied().unwrap_or(false)) {
                return Err("a conflict outside the ledger and generated blocks".to_owned());
            }
        }
    }
    Ok(theirs)
}

/// Sections whose stamp doesn't match their inputs, as `ID#anchor`.
pub fn stale_sections(l: &Loaded) -> Result<Vec<String>> {
    let ledger = Ledger::read(&l.root, &l.config.docs.ledger).unwrap_or_default();
    let mut stale = Vec::new();
    for (key, inputs) in current_inputs(l)? {
        if !super::ledger::diff(ledger.sections.get(&key), &inputs).is_empty() {
            stale.push(key);
        }
    }
    Ok(stale)
}

/// For each of `keys`, the distinct notes this branch (`ours`, before the merge) carries on
/// it: those on entries that differ from the merge base's, else every note it carries.
fn branch_notes(
    root: &Path,
    ledger_path: &str,
    ours: &str,
    theirs: &str,
    keys: &[String],
) -> Result<BTreeMap<String, Vec<String>>> {
    let mut out = BTreeMap::new();
    if keys.is_empty() {
        return Ok(out);
    }
    let at = |rev: &str| -> Result<Ledger> {
        Ok(git::show(root, rev, ledger_path)?
            .and_then(|t| Ledger::parse(&t).ok())
            .unwrap_or_default())
    };
    let base = git::merge_base(root, ours, theirs).ok();
    let branch = at(ours)?;
    let before = match &base {
        Some(b) => at(b)?,
        None => Ledger::default(),
    };
    for key in keys {
        let Some(stamp) = branch.sections.get(key) else {
            continue;
        };
        let old = before.sections.get(key);
        let mut own: Vec<String> = Vec::new();
        let mut all: Vec<String> = Vec::new();
        for (input, e) in stamp {
            let Some(n) = &e.note else { continue };
            if !all.contains(n) {
                all.push(n.clone());
            }
            if old.and_then(|o| o.get(input)) != Some(e) && !own.contains(n) {
                own.push(n.clone());
            }
        }
        let notes = if own.is_empty() { all } else { own };
        if !notes.is_empty() {
            out.insert(key.clone(), notes);
        }
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn entry(hash: &str, note: &str) -> String {
        format!("stamp = {{ hash = \"{hash}\", by = \"agent:x\", note = \"{note}\" }}\n")
    }

    #[test]
    fn ledger_conflicts_keep_the_side_that_matches() {
        let head =
            "# h\n\n[\"S#a\".\"body\"]\nstamp = { hash = \"b3:b\" }\n\n[\"S#a\".\"file:x\"]\n";
        let text = format!(
            "{head}<<<<<<< HEAD\n{}=======\n{}>>>>>>> origin/main\n\n[\"T#t\".\"body\"]\n<<<<<<< HEAD\n{}=======\n{}>>>>>>> origin/main\n",
            entry("b3:ours", "ours"),
            entry("b3:theirs", "theirs"),
            entry("b3:t1", "t ours"),
            entry("b3:t2", "t theirs"),
        );
        let mut current = BTreeMap::new();
        current.insert(
            "S#a".to_owned(),
            BTreeMap::from([
                ("body".to_owned(), "b3:b".to_owned()),
                ("file:x".to_owned(), "b3:ours".to_owned()),
            ]),
        );
        current.insert(
            "T#t".to_owned(),
            BTreeMap::from([("body".to_owned(), "b3:t3".to_owned())]),
        );
        let (out, stamps) = resolve_ledger(&text, &current).unwrap();
        assert_eq!(stamps, vec!["S#a".to_owned(), "T#t".to_owned()]);
        let l = Ledger::parse(&out).unwrap();
        assert_eq!(l.sections["S#a"]["file:x"].hash, "b3:ours");
        assert!(
            l.sections
                .get("T#t")
                .is_none_or(|s| !s.contains_key("body"))
        );
        assert!(super::super::ledger::diff(l.sections.get("S#a"), &current["S#a"]).is_empty());
        assert!(!super::super::ledger::diff(l.sections.get("T#t"), &current["T#t"]).is_empty());
    }

    #[test]
    fn generated_only_conflicts_take_the_merged_side() {
        let block = |o: &str, t: &str| {
            format!(
                "<!-- @doc\nid = \"R\"\n-->\n# R\n\n<!-- @generated begin progress -->\n| a |\n<<<<<<< HEAD\n{o}=======\n{t}>>>>>>> origin/main\n<!-- @generated end progress -->\n\nAfter.\n"
            )
        };
        let text = block("7 left.\n", "8 left.\n");
        let out = resolve_generated("README.md", &text).unwrap();
        assert!(
            out.contains("| a |\n8 left.\n<!-- @generated end progress -->"),
            "{out}"
        );
        let outside = text.replace(
            "\nAfter.\n",
            "\n<<<<<<< HEAD\nOurs.\n=======\nMain.\n>>>>>>> m\n",
        );
        assert!(resolve_generated("README.md", &outside).is_err());
        let unowned = text.replace("progress -->", "other -->");
        assert!(resolve_generated("README.md", &unowned).is_err());
        assert!(resolve_generated("README.md", "no markers\n").is_err());
    }

    #[test]
    fn diff3_markers_drop_the_base() {
        let text = "a\n<<<<<<< HEAD\nx\n||||||| base\nb\n=======\ny\n>>>>>>> m\nz\n";
        let (o, t, h) = sides(text).unwrap();
        assert_eq!(
            (o.as_str(), t.as_str(), h.len()),
            ("a\nx\nz\n", "a\ny\nz\n", 1)
        );
        assert!(sides("<<<<<<< HEAD\nx\n").is_err());
    }
}
