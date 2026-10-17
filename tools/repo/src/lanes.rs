//! Serial lanes (`[lanes].file`, by default `lanes.toml`) and `repo lane-guard`.
//!
//! Each top-level key is a lane: `name = { value = <int>, holder = <PR number> }`. A PR
//! that changes a lane (its value at head differs from the merge base's) must set `value`
//! to the base's value plus one and `holder` to its own number; a new lane passes when its
//! holder is the PR, whatever its initial value; a lane may not be removed. With `--wave`
//! (a wave's landing PR, which carries several claims) a changed lane only has to advance
//! past the base's value.

use std::collections::BTreeMap;
use std::path::Path;

use anyhow::{Context, Result};
use serde::Deserialize;

use crate::Outcome;
use crate::args::Args;
use crate::config::{self, Config};
use crate::git;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Lane {
    pub value: i64,
    pub holder: u64,
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

/// `repo lane-guard --base <rev> --head <rev> --pr <n> [--wave]`.
pub fn run(args: &[String]) -> Outcome {
    let usage = "repo lane-guard --base <rev> --head <rev> --pr <n> [--wave]";
    let a = Args::parse(args, &["base", "head", "pr"], &["wave"])?;
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
    let path = config.lanes.file.as_str();
    let (problems, claims) = guard(
        &at(&root, &base, path)?,
        &at(&root, &mb, path)?,
        &at(&root, &head, path)?,
        pr,
        a.switch("wave"),
    );
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
                holder: 512
            }
        );
        assert!(lanes("# only comments\n").is_empty());
        assert!(parse("l", "p = { value = 1, holder = 2, x = 3 }").is_err());
        assert!(parse("l", "p = { value = 1 }").is_err());
        assert!(parse("l", "p = 3").is_err());
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
