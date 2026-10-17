//! `repo plan check <launch.md> <roster.md> [--row <id>]`: no two running rows own the
//! same path.
//!
//! `launch.md` holds one section per row, opened by an `<a id="<ID>"></a>` line and running
//! to the next such line. Its `owns` field is a table row (`| owns | `a/**`, `b/**` |`) or
//! a line (`owns: `a/**``, `- **owns:** ...`); the globs are its backticked values, or its
//! comma-separated words when none are backticked.
//!
//! `roster.md` holds Markdown tables; a table with a `row` column and a `state` column
//! lists rows (docs/agents/README.md#state: `| row | session | pr | shard | state |`). A row
//! is running unless its state is one of [`FINISHED`]. A table without a `row` column, such
//! as the roles table, is skipped. `--row <id>` adds a row about to launch to the running
//! set.

use std::collections::BTreeMap;

use anyhow::{Context, Result};

use crate::Outcome;
use crate::args::Args;

/// States of a roster row that no longer runs.
pub const FINISHED: &[&str] = &[
    "done",
    "merged",
    "ended",
    "closed",
    "archived",
    "abandoned",
    "cancelled",
    "canceled",
    "dropped",
    "retired",
    "planned",
];

/// The `owns` globs of each row in `launch.md`, by id.
pub fn launch_rows(text: &str) -> BTreeMap<String, Option<Vec<String>>> {
    let lines: Vec<&str> = text.lines().collect();
    let open = crate::docs::parse::outside_fences(&lines);
    let mut out = BTreeMap::new();
    let mut current: Option<String> = None;
    for (k, line) in lines.iter().enumerate() {
        if !open[k] {
            continue;
        }
        if let Some(id) = crate::docs::parse::anchor_of(line) {
            current = Some(id.to_owned());
            out.entry(id.to_owned()).or_insert(None);
            continue;
        }
        let Some(id) = &current else { continue };
        if let Some(v) = owns_value(line) {
            out.insert(id.clone(), Some(globs(&v)));
        }
    }
    out
}

/// The value of an `owns` field on `line`, if it is one.
fn owns_value(line: &str) -> Option<String> {
    let t = line.trim();
    if let Some(row) = t.strip_prefix('|') {
        let cells: Vec<&str> = row
            .trim_end_matches('|')
            .split('|')
            .map(str::trim)
            .collect();
        let key = cells.first()?.trim_matches('*').trim().to_lowercase();
        return (key == "owns" && cells.len() >= 2).then(|| cells[1..].join("|"));
    }
    let t = t.trim_start_matches(['-', '*', ' ']);
    let t = t.trim_start_matches('*');
    let (key, rest) = t.split_once(':')?;
    let key = key.trim().trim_matches('*').trim().to_lowercase();
    (key == "owns").then(|| rest.trim_start_matches('*').trim().to_owned())
}

fn globs(value: &str) -> Vec<String> {
    let ticked: Vec<String> = value
        .split('`')
        .skip(1)
        .step_by(2)
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(str::to_owned)
        .collect();
    if !ticked.is_empty() {
        return ticked;
    }
    value
        .split([',', ' '])
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(str::to_owned)
        .collect()
}

/// Cells of a Markdown table row.
fn cells(line: &str) -> Option<Vec<String>> {
    let t = line.trim();
    let inner = t.strip_prefix('|')?;
    Some(
        inner
            .strip_suffix('|')
            .unwrap_or(inner)
            .split('|')
            .map(|c| c.trim().to_owned())
            .collect(),
    )
}

/// A cell's plain id: link text or code span unwrapped, first word.
fn plain(cell: &str) -> String {
    let c = cell.trim();
    let c = c
        .strip_prefix('[')
        .and_then(|r| r.split_once(']'))
        .map_or(c, |(t, _)| t);
    c.trim_matches(['`', '*'])
        .split_whitespace()
        .next()
        .unwrap_or_default()
        .trim_matches(['`', '*'])
        .to_owned()
}

/// The ids of roster rows that are running, with their state.
pub fn running(text: &str) -> Vec<(String, String)> {
    let mut out = Vec::new();
    let mut cols: Option<(usize, usize)> = None;
    for line in text.lines() {
        let Some(row) = cells(line) else {
            cols = None;
            continue;
        };
        if row
            .iter()
            .all(|c| c.chars().all(|ch| matches!(ch, '-' | ':' | ' ')) && !c.is_empty())
        {
            continue;
        }
        let Some((ri, si)) = cols else {
            let lower: Vec<String> = row.iter().map(|c| c.to_lowercase()).collect();
            let r = lower
                .iter()
                .position(|c| c == "row")
                .or_else(|| lower.iter().position(|c| c.contains("row")));
            let s = lower.iter().position(|c| c == "state" || c == "status");
            if let (Some(r), Some(s)) = (r, s) {
                cols = Some((r, s));
            }
            continue;
        };
        let (Some(id), Some(state)) = (row.get(ri), row.get(si)) else {
            continue;
        };
        let (id, state) = (plain(id), plain(state).to_lowercase());
        if id.is_empty() || FINISHED.contains(&state.as_str()) {
            continue;
        }
        out.push((id, state));
    }
    out
}

/// Problems among the running rows: overlapping `owns`, and rows without one.
pub fn check(
    launch: &BTreeMap<String, Option<Vec<String>>>,
    running: &[String],
    known: &[String],
) -> Result<Vec<String>> {
    let mut problems = Vec::new();
    let mut rows: Vec<(&str, &[String])> = Vec::new();
    for id in running {
        match launch.get(id) {
            None => problems.push(format!("running row `{id}` has no section in launch.md")),
            Some(None) => problems.push(format!("row `{id}` has no `owns` field")),
            Some(Some(g)) => rows.push((id, g)),
        }
    }
    for (i, (a, ga)) in rows.iter().enumerate() {
        for (b, gb) in &rows[i + 1..] {
            for x in *ga {
                for y in *gb {
                    if crate::glob::overlap(x, y, known)? {
                        problems.push(format!(
                            "rows `{a}` and `{b}` both own paths: `{x}` and `{y}` overlap"
                        ));
                    }
                }
            }
        }
    }
    Ok(problems)
}

pub fn run(args: &[String]) -> Outcome {
    let usage = "repo plan check <launch.md> <roster.md> [--row <id>]";
    let a = Args::parse(args, &["row"], &[])?;
    let paths = a.exactly(2, usage)?;
    let launch_text =
        std::fs::read_to_string(&paths[0]).with_context(|| format!("reading {}", paths[0]))?;
    let roster_text =
        std::fs::read_to_string(&paths[1]).with_context(|| format!("reading {}", paths[1]))?;
    let launch = launch_rows(&launch_text);
    // Roster lines for roles (not rows) have no launch.md section and are skipped.
    let mut ids: Vec<String> = running(&roster_text)
        .into_iter()
        .map(|(id, _)| id)
        .filter(|id| launch.contains_key(id))
        .collect();
    if let Some(r) = a.value("row") {
        ids.push(r.to_owned());
    }
    ids.sort();
    ids.dedup();
    let known = crate::git::root()
        .ok()
        .and_then(|r| crate::git::ls_files(&r, &[]).ok())
        .unwrap_or_default();
    let problems = check(&launch, &ids, &known)?;
    for p in &problems {
        println!("plan check: {p}");
    }
    if problems.is_empty() {
        println!("plan check: ok ({} running row(s), no overlap)", ids.len());
    }
    Ok(problems.is_empty())
}

#[cfg(test)]
mod tests {
    use super::*;

    const LAUNCH: &str = "# Launch\n\n<a id=\"B1\"></a>\n## B1: Parser\n\n| Field | Value |\n|---|---|\n| id | B1 |\n| owns | `src/parse/**`, `docs/PARSE.md` |\n\n<a id=\"B2\"></a>\n## B2: Lexer\n- **owns:** `src/lex/**`\n\n<a id=\"B3\"></a>\n## B3\nowns: src/**\n\n<a id=\"B4\"></a>\n## B4\nNo owns yet.\n```\n<a id=\"B9\"></a>\n```\n";

    const ROSTER: &str = "coordinator\n\n| role | session | state |\n|---|---|---|\n| planner | s0 | running |\n\n| row | session | pr | shard | state |\n|---|---|---|---|---|\n| [B1](launch.md#B1) | s1 | #3 | 1 | building |\n| `B2` | s2 | #4 | 0 | review |\n| B3 | s3 | #5 | 1 | merged |\n| B5 | s5 | #7 | 1 | ended |\n";

    #[test]
    fn reads_launch_rows() {
        let rows = launch_rows(LAUNCH);
        assert_eq!(
            rows["B1"].as_deref(),
            Some(&["src/parse/**".to_owned(), "docs/PARSE.md".to_owned()][..])
        );
        assert_eq!(rows["B2"].as_deref(), Some(&["src/lex/**".to_owned()][..]));
        assert_eq!(rows["B3"].as_deref(), Some(&["src/**".to_owned()][..]));
        assert_eq!(rows["B4"], None);
        assert!(!rows.contains_key("B9"));
    }

    #[test]
    fn reads_running_rows() {
        let r: Vec<String> = running(ROSTER).into_iter().map(|(id, _)| id).collect();
        assert_eq!(r, vec!["B1", "B2"]);
    }

    #[test]
    fn overlap_between_running_rows_fails() {
        let rows = launch_rows(LAUNCH);
        let ok = check(&rows, &["B1".into(), "B2".into()], &[]).unwrap();
        assert!(ok.is_empty(), "{ok:?}");
        let bad = check(&rows, &["B1".into(), "B3".into()], &[]).unwrap();
        assert_eq!(bad.len(), 1, "{bad:?}");
        let missing = check(&rows, &["B4".into(), "B7".into()], &[]).unwrap();
        assert_eq!(missing.len(), 2, "{missing:?}");
    }
}
