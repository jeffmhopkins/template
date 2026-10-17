//! Serial lanes (`[lanes].file`, by default `lanes.toml`) and `repo lane-guard`.
//!
//! Each top-level key is a lane: `name = { value = <int>, holder = <PR number> }`, with
//! optional `files` (globs of the files that hold the value) and `pattern` (a regular
//! expression for the lines that hold it). A PR that changes a lane (its entry at head
//! differs from the merge base's) must set `value` to the base's value plus one and
//! `holder` to its own number; a new lane passes when its holder is the PR, whatever its
//! initial value; a lane may not be removed. A PR that changes a lane's `files` (only the
//! lines matching its `pattern`, when it has one) without a claim fails: the lanes the base
//! declares decide. With `--wave` (a wave's landing PR, which carries several claims: head
//! `wave/*`, base `main`, and the head branch exists on `origin`) a changed lane only has
//! to advance past the base's value.

use std::collections::BTreeMap;
use std::path::Path;

use anyhow::{Context, Result, bail};
use regex::Regex;
use serde::Deserialize;

use crate::Outcome;
use crate::args::Args;
use crate::config::{self, Config};
use crate::git;
use crate::governance::FileChange;

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Lane {
    pub value: i64,
    pub holder: u64,
    /// Globs of the files that hold the lane's value.
    #[serde(default)]
    pub files: Vec<String>,
    /// A regular expression for the lines in `files` that hold the value; absent: any line.
    #[serde(default)]
    pub pattern: Option<String>,
}

pub type Lanes = BTreeMap<String, Lane>;

pub fn parse(path: &str, text: &str) -> Result<Lanes> {
    let table: toml::Table = text
        .parse()
        .map_err(|e: toml::de::Error| anyhow::anyhow!("{path}: {}", e.message()))?;
    let mut out = Lanes::new();
    for (name, v) in table {
        let lane = Lane::deserialize(v)
            .map_err(|e| anyhow::anyhow!("{path}: lane `{name}`: {}", e.message()))?;
        for g in &lane.files {
            crate::glob::compile(g).map_err(|e| anyhow::anyhow!("{path}: lane `{name}`: {e:#}"))?;
        }
        if let Some(p) = &lane.pattern {
            Regex::new(p).map_err(|e| anyhow::anyhow!("{path}: lane `{name}`: pattern: {e}"))?;
        }
        out.insert(name, lane);
    }
    Ok(out)
}

/// The lanes file in the working tree; empty when absent.
pub fn read_file(root: &Path, path: &str) -> Result<Lanes> {
    match std::fs::read_to_string(root.join(path)) {
        Ok(t) => parse(path, &t),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(Lanes::new()),
        Err(e) => Err(e).with_context(|| format!("reading {path}")),
    }
}

fn at(root: &Path, rev: &str, path: &str) -> Result<Lanes> {
    match git::show(root, rev, path)? {
        Some(t) => parse(&format!("{path} at {rev}"), &t),
        None => Ok(Lanes::new()),
    }
}

/// Problems with a PR's lane claims, given the lanes at the base tip, the merge base and
/// the head.
pub fn guard(
    base: &Lanes,
    merge_base: &Lanes,
    head: &Lanes,
    pr: u64,
    wave: bool,
) -> (Vec<String>, Vec<String>) {
    let (mut problems, mut claims) = (Vec::new(), Vec::new());
    for name in merge_base.keys().filter(|n| !head.contains_key(*n)) {
        problems.push(format!("lane `{name}` is removed; lanes are never removed"));
    }
    for (name, lane) in head {
        if merge_base.get(name) == Some(lane) {
            continue;
        }
        claims.push(format!(
            "{name} = {{ value = {}, holder = {} }}",
            lane.value, lane.holder
        ));
        if !wave && lane.holder != pr {
            problems.push(format!(
                "lane `{name}`: holder is {}, not this PR (#{pr})",
                lane.holder
            ));
        }
        match base.get(name) {
            Some(b) if wave && lane.value <= b.value => problems.push(format!(
                "lane `{name}`: value is {}, but a wave landing must advance the base's {}",
                lane.value, b.value
            )),
            Some(b) if !wave && lane.value != b.value + 1 => problems.push(format!(
                "lane `{name}`: value is {}, but the base has {}, so it must be {}",
                lane.value,
                b.value,
                b.value + 1
            )),
            None if merge_base.contains_key(name) => {
                problems.push(format!("lane `{name}` no longer exists on the base"));
            }
            _ => {}
        }
    }
    (problems, claims)
}

/// Lines in one text and not the other, both ways, as a multiset.
fn changed_lines<'a>(base: &'a str, head: &'a str) -> Vec<&'a str> {
    let mut count: BTreeMap<&str, i64> = BTreeMap::new();
    for l in base.lines() {
        *count.entry(l).or_default() -= 1;
    }
    for l in head.lines() {
        *count.entry(l).or_default() += 1;
    }
    count
        .into_iter()
        .filter(|(_, n)| *n != 0)
        .map(|(l, _)| l)
        .collect()
}

/// Changes to the files of a lane the PR doesn't claim. The lanes come from the base, so a
/// PR can't drop its own files from a lane; a lane counts as claimed when its entry at the
/// head differs from the merge base's.
pub fn unclaimed(
    base: &Lanes,
    merge_base: &Lanes,
    head: &Lanes,
    files: &[FileChange],
) -> Result<Vec<String>> {
    let mut out = Vec::new();
    for (name, lane) in base {
        if lane.files.is_empty() || merge_base.get(name) != head.get(name) {
            continue;
        }
        let globs = crate::glob::Globs::new(&lane.files)?;
        let pattern = lane.pattern.as_deref().map(Regex::new).transpose()?;
        for f in files.iter().filter(|f| globs.is_match(&f.path)) {
            let (b, h) = (
                f.base.as_deref().unwrap_or(""),
                f.head.as_deref().unwrap_or(""),
            );
            let hit = match &pattern {
                None => true,
                Some(r) => changed_lines(b, h).iter().any(|l| r.is_match(l)),
            };
            if hit {
                let what = match &lane.pattern {
                    Some(p) => format!("a line matching `{p}` in `{}`", f.path),
                    None => format!("`{}`", f.path),
                };
                out.push(format!(
                    "lane `{name}`: this PR changes {what}, which holds the lane's value, but makes no claim in the lanes file"
                ));
            }
        }
    }
    Ok(out)
}

/// Why `--wave` doesn't apply, if it doesn't: a landing PR's head is a `wave/*` branch on
/// `origin` and its base is `main`.
pub fn wave_problem(root: &Path, head_ref: Option<&str>, base_ref: Option<&str>) -> Option<String> {
    let (Some(head), Some(base)) = (head_ref, base_ref) else {
        return Some("`--wave` needs `--head-ref` and `--base-ref`".into());
    };
    if !head.starts_with("wave/") || base != "main" {
        return Some(format!(
            "`--wave` is for a wave's landing PR (head `wave/*`, base `main`), not `{head}` into `{base}`"
        ));
    }
    let spec = format!("refs/remotes/origin/{head}");
    (!git::succeeds(root, &["rev-parse", "--verify", "-q", &spec]))
        .then(|| format!("`--wave`: `{head}` is not a branch on origin"))
}

/// `repo lane-guard --base <rev> --head <rev> --pr <n> [--wave --head-ref <branch> --base-ref <branch>]`.
pub fn run(args: &[String]) -> Outcome {
    let usage = "repo lane-guard --base <rev> --head <rev> --pr <n> [--wave --head-ref <branch> --base-ref <branch>]";
    let a = Args::parse(
        args,
        &["base", "head", "pr", "head-ref", "base-ref"],
        &["wave"],
    )?;
    a.exactly(0, usage)?;
    let pr: u64 = a
        .required("pr", usage)?
        .trim_start_matches('#')
        .parse()
        .context("--pr must be a PR number")?;
    let root = git::root()?;
    let base = git::resolve(&root, a.required("base", usage)?)?;
    let head = git::resolve(&root, a.required("head", usage)?)?;
    let mb = git::merge_base(&root, &base, &head)?;
    let config = match git::show(&root, &base, config::FILE)? {
        Some(t) => Config::parse(&t)?,
        None => Config::load(&root)?,
    };
    let wave = a.switch("wave");
    if wave {
        if let Some(p) = wave_problem(&root, a.value("head-ref"), a.value("base-ref")) {
            bail!("{p}\nusage: {usage}");
        }
    }
    let path = config.lanes.file.as_str();
    let (base_lanes, mb_lanes, head_lanes) = (
        at(&root, &base, path)?,
        at(&root, &mb, path)?,
        at(&root, &head, path)?,
    );
    let (mut problems, claims) = guard(&base_lanes, &mb_lanes, &head_lanes, pr, wave);
    if base_lanes.values().any(|l| !l.files.is_empty()) {
        let files = crate::governance::changed_files(&root, &mb, &head)?;
        problems.extend(unclaimed(&base_lanes, &mb_lanes, &head_lanes, &files)?);
    }
    if claims.is_empty() && problems.is_empty() {
        println!("lane-guard: this PR changes no lane");
        return Ok(true);
    }
    for c in &claims {
        println!("claims: {c}");
    }
    for p in &problems {
        println!("lane-guard: {p}");
    }
    if problems.is_empty() {
        println!("lane-guard: ok");
    }
    Ok(problems.is_empty())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn lanes(text: &str) -> Lanes {
        parse("lanes.toml", text).unwrap()
    }

    #[test]
    fn parses_lanes_strictly() {
        let l = lanes("# c\nprotocol = { value = 21, holder = 512 }\n");
        assert_eq!(
            l["protocol"],
            Lane {
                value: 21,
                holder: 512,
                files: vec![],
                pattern: None,
            }
        );
        assert!(lanes("# only comments\n").is_empty());
        assert!(parse("l", "p = { value = 1, holder = 2, x = 3 }").is_err());
        assert!(parse("l", "p = { value = 1 }").is_err());
        assert!(parse("l", "p = 3").is_err());
        let f = lanes("p = { value = 1, holder = 2, files = [\"src/v.rs\"], pattern = 'VERSION' }");
        assert_eq!(f["p"].files, vec!["src/v.rs"]);
        assert!(parse("l", "p = { value = 1, holder = 2, pattern = '(' }").is_err());
        assert!(parse("l", "p = { value = 1, holder = 2, files = ['a/{'] }").is_err());
    }

    fn file(path: &str, base: &str, head: &str) -> FileChange {
        FileChange {
            path: path.into(),
            base: Some(base.into()),
            head: Some(head.into()),
        }
    }

    #[test]
    fn lane_files_need_a_claim() {
        let decl = "p = { value = 3, holder = 9, files = [\"src/proto/*.rs\"], pattern = 'PROTOCOL: u32' }\n";
        let base = lanes(decl);
        let claimed = lanes(&decl.replace("value = 3, holder = 9", "value = 4, holder = 12"));
        let bump = file(
            "src/proto/version.rs",
            "pub const PROTOCOL: u32 = 3;\nfn a() {}\n",
            "pub const PROTOCOL: u32 = 4;\nfn a() {}\n",
        );
        let other_line = file(
            "src/proto/version.rs",
            "pub const PROTOCOL: u32 = 3;\nfn a() {}\n",
            "pub const PROTOCOL: u32 = 3;\nfn b() {}\n",
        );
        let elsewhere = file("src/main.rs", "PROTOCOL: u32 = 3", "PROTOCOL: u32 = 4");
        let got = unclaimed(&base, &base, &base, std::slice::from_ref(&bump)).unwrap();
        assert_eq!(got.len(), 1, "{got:?}");
        assert!(got[0].contains("lane `p`"), "{got:?}");
        assert!(
            unclaimed(&base, &base, &claimed, &[bump])
                .unwrap()
                .is_empty()
        );
        assert!(
            unclaimed(&base, &base, &base, &[other_line, elsewhere])
                .unwrap()
                .is_empty()
        );
        // Without a pattern, any change to the files needs a claim.
        let any = lanes("m = { value = 1, holder = 2, files = [\"migrations/**\"] }\n");
        let added = FileChange {
            path: "migrations/0002.sql".into(),
            base: None,
            head: Some("create table x;".into()),
        };
        assert_eq!(unclaimed(&any, &any, &any, &[added]).unwrap().len(), 1);
    }

    #[test]
    fn claims_must_be_base_plus_one_and_held() {
        let base = lanes("p = { value = 21, holder = 500 }\nq = { value = 3, holder = 400 }\n");
        let mb = lanes("p = { value = 20, holder = 490 }\nq = { value = 3, holder = 400 }\n");
        let good = lanes("p = { value = 22, holder = 512 }\nq = { value = 3, holder = 400 }\n");
        assert!(guard(&base, &mb, &good, 512, false).0.is_empty());
        let stale = lanes("p = { value = 21, holder = 512 }\nq = { value = 3, holder = 400 }\n");
        assert_eq!(guard(&base, &mb, &stale, 512, false).0.len(), 1);
        assert_eq!(guard(&base, &mb, &good, 513, false).0.len(), 1);
        // Untouched lanes are not checked, even when the base moved past them.
        let untouched =
            lanes("p = { value = 20, holder = 490 }\nq = { value = 3, holder = 400 }\n");
        let (p, c) = guard(&base, &mb, &untouched, 512, false);
        assert!(p.is_empty() && c.is_empty());
        let removed = lanes("p = { value = 20, holder = 490 }\n");
        assert_eq!(guard(&base, &mb, &removed, 512, false).0.len(), 1);
        let added = lanes(
            "p = { value = 20, holder = 490 }\nq = { value = 3, holder = 400 }\nr = { value = 1, holder = 512 }\n",
        );
        assert!(guard(&base, &mb, &added, 512, false).0.is_empty());
        // A wave landing advances a lane by several steps, holder from inside the wave.
        let landed = lanes("p = { value = 24, holder = 530 }\nq = { value = 3, holder = 400 }\n");
        assert!(guard(&base, &mb, &landed, 600, true).0.is_empty());
        assert_eq!(guard(&base, &mb, &landed, 600, false).0.len(), 2);
        assert_eq!(guard(&base, &mb, &stale, 600, true).0.len(), 1);
    }
}
