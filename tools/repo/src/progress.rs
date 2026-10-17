//! `repo progress`: roadmap status derived from commit trailers.
//!
//! `[docs].progress` (by default `docs/progress.toml`) holds milestones and items only.
//! Status comes from trailers in the commit messages reachable from HEAD:
//! `Completes: <id>`, `Part-of: <id>` and `Verified-on-hardware: <id>` (several ids may be
//! comma-separated). A trailer counts only in a trailer paragraph: one whose every line is
//! `Key: value`, outside code fences, with HTML comments removed.
//!
//! A commit whose message says `This reverts commit <sha>`, or whose subject is
//! `Revert "<subject>"` or `revert: <subject>` naming an earlier commit's subject, cancels
//! that commit's trailers, unless a later revert cancels it in turn. A
//! `Verified-on-hardware:` counts only after the item's latest `Completes:`.

use std::collections::{BTreeMap, BTreeSet};
use std::fmt::Write as _;
use std::path::Path;

use anyhow::{Context, Result, bail};
use serde::{Deserialize, Serialize};

use crate::Outcome;
use crate::args::Args;
use crate::config::Repo;
use crate::git;

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Milestone {
    pub id: String,
    pub title: String,
    pub demo: String,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Item {
    pub id: String,
    pub milestone: String,
    pub title: String,
    #[serde(default)]
    pub deps: Vec<String>,
    #[serde(default)]
    pub hardware: bool,
}

/// The parsed plan.
#[derive(Debug, Clone, Default, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Plan {
    #[serde(default)]
    pub milestone: Vec<Milestone>,
    #[serde(default)]
    pub item: Vec<Item>,
}

impl Plan {
    /// Parses and validates the plan.
    pub fn parse(text: &str) -> Result<Self> {
        let plan: Self = toml::from_str(text).map_err(|e| anyhow::anyhow!("{}", e.message()))?;
        let mut errs = Vec::new();
        let mut ms = BTreeSet::new();
        for m in &plan.milestone {
            if !ms.insert(m.id.as_str()) {
                errs.push(format!("duplicate milestone `{}`", m.id));
            }
        }
        let mut ids = BTreeSet::new();
        for i in &plan.item {
            if !ids.insert(i.id.as_str()) {
                errs.push(format!("duplicate item `{}`", i.id));
            }
            if !ms.contains(i.milestone.as_str()) {
                errs.push(format!(
                    "item `{}`: unknown milestone `{}`",
                    i.id, i.milestone
                ));
            }
        }
        for i in &plan.item {
            for d in &i.deps {
                if !ids.contains(d.as_str()) {
                    errs.push(format!("item `{}`: unknown dependency `{d}`", i.id));
                }
            }
        }
        if let Some(c) = plan.cycle() {
            errs.push(format!("dependency cycle through `{c}`"));
        }
        if !errs.is_empty() {
            bail!("{}", errs.join("; "));
        }
        Ok(plan)
    }

    /// The plan at `root/path`; empty when the file doesn't exist.
    pub fn load(root: &Path, path: &str) -> Result<Self> {
        match std::fs::read_to_string(root.join(path)) {
            Ok(t) => Self::parse(&t).with_context(|| path.to_owned()),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(Self::default()),
            Err(e) => Err(e).with_context(|| format!("reading {path}")),
        }
    }

    /// An item on a dependency cycle, if there is one.
    fn cycle(&self) -> Option<String> {
        let deps: BTreeMap<&str, &[String]> = self
            .item
            .iter()
            .map(|i| (i.id.as_str(), i.deps.as_slice()))
            .collect();
        // 0 unvisited, 1 on the stack, 2 done.
        let mut state: BTreeMap<&str, u8> = BTreeMap::new();
        fn visit<'a>(
            n: &'a str,
            deps: &BTreeMap<&'a str, &'a [String]>,
            state: &mut BTreeMap<&'a str, u8>,
        ) -> Option<String> {
            match state.get(n) {
                Some(1) => return Some(n.to_owned()),
                Some(2) => return None,
                _ => {}
            }
            state.insert(n, 1);
            for d in deps.get(n).copied().unwrap_or_default() {
                if let Some(c) = visit(d, deps, state) {
                    return Some(c);
                }
            }
            state.insert(n, 2);
            None
        }
        self.item
            .iter()
            .find_map(|i| visit(&i.id, &deps, &mut state))
    }
}

/// One commit's message.
#[derive(Debug, Clone)]
pub struct Message {
    pub sha: String,
    pub body: String,
}

impl Message {
    fn subject(&self) -> &str {
        self.body.lines().next().unwrap_or_default().trim()
    }
}

/// The trailer kinds.
const COMPLETES: &str = "Completes";
const PART_OF: &str = "Part-of";
const VERIFIED: &str = "Verified-on-hardware";

/// A `Key: value` line, as git writes trailers: a key of letters, digits and `-`.
fn trailer_line(line: &str) -> Option<(&str, &str)> {
    let (key, value) = line.trim().split_once(':')?;
    let ok = key.starts_with(|c: char| c.is_ascii_alphabetic())
        && key.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'-')
        && !value.trim().is_empty();
    ok.then(|| (key, value.trim()))
}

/// The message without HTML comments, and with code-fenced lines blanked.
fn plain(body: &str) -> String {
    let mut text = body.to_owned();
    while let Some(open) = text.find("<!--") {
        match text[open..].find("-->") {
            Some(close) => text.replace_range(open..open + close + 3, ""),
            None => text.truncate(open),
        }
    }
    let lines: Vec<&str> = text.lines().collect();
    let outside = crate::docs::parse::outside_fences(&lines);
    lines
        .iter()
        .zip(outside)
        .map(|(l, o)| {
            if o && !l.trim_start().starts_with("```") {
                *l
            } else {
                ""
            }
        })
        .collect::<Vec<_>>()
        .join("\n")
}

/// `(kind, id)` trailers in a message, from its trailer paragraphs only.
fn trailers(body: &str) -> Vec<(&'static str, String)> {
    let mut out = Vec::new();
    let text = plain(body);
    let lines: Vec<&str> = text.lines().collect();
    for para in lines.split(|l| l.trim().is_empty()) {
        if para.is_empty() || !para.iter().all(|l| trailer_line(l).is_some()) {
            continue;
        }
        for (key, value) in para.iter().filter_map(|l| trailer_line(l)) {
            let kind = [COMPLETES, PART_OF, VERIFIED]
                .into_iter()
                .find(|k| k.eq_ignore_ascii_case(key));
            let Some(kind) = kind else {
                continue;
            };
            for id in value.split(',').map(|v| v.trim().trim_matches('`')) {
                if !id.is_empty() && !id.contains(char::is_whitespace) {
                    out.push((kind, id.to_owned()));
                }
            }
        }
    }
    out
}

/// The commit a revert names: by `This reverts commit <sha>`, else by the quoted subject.
fn reverted<'a>(m: &Message, earlier: &'a [Message]) -> Option<&'a Message> {
    if let Some(k) = m.body.find("This reverts commit ") {
        let sha: String = m.body[k + "This reverts commit ".len()..]
            .chars()
            .take_while(char::is_ascii_hexdigit)
            .collect();
        if sha.len() >= 7 {
            if let Some(t) = earlier.iter().rev().find(|e| e.sha.starts_with(&sha)) {
                return Some(t);
            }
        }
    }
    let inner = match m.subject().strip_prefix("Revert \"") {
        Some(rest) => rest.rfind('"').map(|k| &rest[..k])?,
        None => conventional_revert(m.subject())?,
    };
    let strip = |s: &str| -> String {
        match crate::docs::history::pr_of(s) {
            Some(_) => s
                .trim_end()
                .rsplit_once(" (#")
                .map_or(s, |(h, _)| h)
                .to_owned(),
            None => s.to_owned(),
        }
    };
    earlier
        .iter()
        .rev()
        .find(|e| e.subject() == inner || strip(e.subject()) == strip(inner))
}

/// The subject a Conventional Commit revert names: `revert: <subject>` or
/// `revert(<scope>): <subject>`, without a trailing ` (#<n>)`.
fn conventional_revert(subject: &str) -> Option<&str> {
    let rest = subject.strip_prefix("revert")?;
    let rest = match rest.strip_prefix('(') {
        Some(r) => &r[r.find(')')? + 1..],
        None => rest,
    };
    let rest = rest
        .strip_prefix('!')
        .unwrap_or(rest)
        .strip_prefix(':')?
        .trim();
    let rest = match crate::docs::history::pr_of(rest) {
        Some(_) => rest.trim_end().rsplit_once(" (#").map_or(rest, |(h, _)| h),
        None => rest,
    };
    (!rest.is_empty()).then_some(rest)
}

/// Status of each item.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum Status {
    Planned,
    InProgress,
    Done,
    Hardware,
}

impl Status {
    pub fn label(self) -> &'static str {
        match self {
            Self::Planned => "planned",
            Self::InProgress => "in-progress",
            Self::Done => "done",
            Self::Hardware => "hardware",
        }
    }
}

/// What the trailers say.
#[derive(Debug, Default)]
pub struct Derived {
    /// id → (completed, part-of seen, verified on hardware), each with the commits.
    pub completes: BTreeMap<String, Vec<String>>,
    pub part_of: BTreeMap<String, Vec<String>>,
    pub verified: BTreeMap<String, Vec<String>>,
}

/// Applies `messages` (oldest first), honouring reverts.
pub fn derive(messages: &[Message]) -> Derived {
    // Newest first: a commit is cancelled when a revert that is not itself cancelled names
    // it, so a chain of reverts settles by its length.
    let mut cancelled: BTreeSet<&str> = BTreeSet::new();
    for k in (0..messages.len()).rev() {
        let m = &messages[k];
        if cancelled.contains(m.sha.as_str()) {
            continue;
        }
        if let Some(t) = reverted(m, &messages[..k]) {
            cancelled.insert(&t.sha);
        }
    }
    let mut d = Derived::default();
    for m in messages
        .iter()
        .filter(|m| !cancelled.contains(m.sha.as_str()))
    {
        let found = trailers(&m.body);
        // Completions first: a verification counts only after the latest one.
        for (kind, id) in found.iter().filter(|(k, _)| *k != VERIFIED) {
            if *kind == COMPLETES {
                d.verified.remove(id);
            }
            let map = if *kind == COMPLETES {
                &mut d.completes
            } else {
                &mut d.part_of
            };
            map.entry(id.clone()).or_default().push(m.sha.clone());
        }
        for (_, id) in found.iter().filter(|(k, _)| *k == VERIFIED) {
            d.verified
                .entry(id.clone())
                .or_default()
                .push(m.sha.clone());
        }
    }
    d
}

/// Commit messages reachable from `revs`, oldest first.
pub fn messages(root: &Path, revs: &[&str]) -> Result<Vec<Message>> {
    let mut args = vec!["log", "--topo-order", "--reverse", "--format=%H%x1f%B%x1e"];
    args.extend_from_slice(revs);
    let out = git::text(root, &args)?;
    Ok(out
        .split('\x1e')
        .filter_map(|r| {
            let (sha, body) = r.trim_start_matches('\n').split_once('\x1f')?;
            Some(Message {
                sha: sha.to_owned(),
                body: body.trim().to_owned(),
            })
        })
        .collect())
}

#[derive(Debug, Serialize)]
pub struct ItemReport {
    pub id: String,
    pub title: String,
    pub status: Status,
    pub hardware: bool,
    pub deps: Vec<String>,
    pub commits: Vec<String>,
}

#[derive(Debug, Serialize)]
pub struct MilestoneReport {
    pub id: String,
    pub title: String,
    pub demo: String,
    pub items: Vec<ItemReport>,
}

#[derive(Debug, Serialize)]
pub struct Report {
    pub milestones: Vec<MilestoneReport>,
    /// Trailer ids that name no item.
    pub unknown: Vec<String>,
}

pub fn report(plan: &Plan, d: &Derived) -> Report {
    let status = |i: &Item| {
        if d.completes.contains_key(&i.id) {
            if i.hardware && !d.verified.contains_key(&i.id) {
                Status::Hardware
            } else {
                Status::Done
            }
        } else if d.part_of.contains_key(&i.id) {
            Status::InProgress
        } else {
            Status::Planned
        }
    };
    let milestones = plan
        .milestone
        .iter()
        .map(|m| MilestoneReport {
            id: m.id.clone(),
            title: m.title.clone(),
            demo: m.demo.clone(),
            items: plan
                .item
                .iter()
                .filter(|i| i.milestone == m.id)
                .map(|i| {
                    let mut commits: Vec<String> = [&d.completes, &d.part_of, &d.verified]
                        .iter()
                        .filter_map(|map| map.get(&i.id))
                        .flatten()
                        .map(|s| s.get(..12).unwrap_or(s).to_owned())
                        .collect();
                    commits.dedup();
                    ItemReport {
                        id: i.id.clone(),
                        title: i.title.clone(),
                        status: status(i),
                        hardware: i.hardware,
                        deps: i.deps.clone(),
                        commits,
                    }
                })
                .collect(),
        })
        .collect();
    let known: BTreeSet<&str> = plan.item.iter().map(|i| i.id.as_str()).collect();
    let unknown = d
        .completes
        .keys()
        .chain(d.part_of.keys())
        .chain(d.verified.keys())
        .filter(|k| !known.contains(k.as_str()))
        .cloned()
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect();
    Report {
        milestones,
        unknown,
    }
}

fn cell(s: &str) -> String {
    crate::docs::generate::escape(s)
}

/// The Markdown table: one summary row per milestone, then each milestone's items.
pub fn markdown(r: &Report) -> String {
    let mut out = String::from(
        "| Milestone | Done | In progress | Hardware run pending | Planned |\n|---|---|---|---|---|\n",
    );
    for m in &r.milestones {
        let n = |s: Status| m.items.iter().filter(|i| i.status == s).count();
        let _ = writeln!(
            out,
            "| {}: {} | {} of {} | {} | {} | {} |",
            cell(&m.id),
            cell(&m.title),
            n(Status::Done),
            m.items.len(),
            n(Status::InProgress),
            n(Status::Hardware),
            n(Status::Planned)
        );
    }
    for m in &r.milestones {
        let _ = write!(
            out,
            "\n<details>\n<summary>{}: {}</summary>\n\nDemo: {}\n\n| Item | Title | Status | Depends on |\n|---|---|---|---|\n",
            cell(&m.id),
            cell(&m.title),
            cell(&m.demo)
        );
        for i in &m.items {
            let _ = writeln!(
                out,
                "| {} | {} | {} | {} |",
                cell(&i.id),
                cell(&i.title),
                i.status.label(),
                cell(&i.deps.join(", "))
            );
        }
        out.push_str("\n</details>\n");
    }
    out
}

/// The inside of the `progress` block, derived from the history of `revs`; `None` when
/// there is no plan file.
pub fn block_for(l: &crate::docs::Loaded, revs: &[&str]) -> Result<Option<String>> {
    if !l.root.join(&l.config.docs.progress).exists() {
        return Ok(None);
    }
    let plan = Plan::load(&l.root, &l.config.docs.progress)?;
    let msgs = messages(&l.root, revs).unwrap_or_default();
    Ok(Some(markdown(&report(&plan, &derive(&msgs)))))
}

/// `repo progress [--format markdown|json]`.
pub fn run(args: &[String]) -> Outcome {
    let a = Args::parse(args, &["format"], &[])?;
    a.exactly(0, "repo progress [--format markdown|json]")?;
    let repo = Repo::open()?;
    let plan = Plan::load(&repo.root, &repo.config.docs.progress)?;
    let r = report(&plan, &derive(&messages(&repo.root, &["HEAD"])?));
    match a.value("format").unwrap_or("markdown") {
        "markdown" => {
            print!("{}", markdown(&r));
            for u in &r.unknown {
                println!(
                    "\nwarning: a trailer names `{u}`, which is not an item in {}",
                    repo.config.docs.progress
                );
            }
        }
        "json" => println!("{}", serde_json::to_string_pretty(&r)?),
        other => bail!("unknown format `{other}` (markdown or json)"),
    }
    Ok(true)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn msg(sha: &str, body: &str) -> Message {
        Message {
            sha: sha.into(),
            body: body.into(),
        }
    }

    const PLAN: &str = "[[milestone]]\nid = \"M0\"\ntitle = \"T\"\ndemo = \"D\"\n\n[[item]]\nid = \"A\"\nmilestone = \"M0\"\ntitle = \"a\"\n\n[[item]]\nid = \"B\"\nmilestone = \"M0\"\ntitle = \"b\"\ndeps = [\"A\"]\nhardware = true\n\n[[item]]\nid = \"C\"\nmilestone = \"M0\"\ntitle = \"c\"\n";

    #[test]
    fn plan_is_validated() {
        assert!(Plan::parse(PLAN).is_ok());
        assert!(Plan::parse(&format!("{PLAN}extra = 1\n")).is_err());
        assert!(Plan::parse(&PLAN.replace("deps = [\"A\"]", "deps = [\"Z\"]")).is_err());
        assert!(
            Plan::parse(&PLAN.replace(
                "milestone = \"M0\"\ntitle = \"c\"",
                "milestone = \"M9\"\ntitle = \"c\""
            ))
            .is_err()
        );
        let cyc = PLAN.replace("title = \"a\"\n", "title = \"a\"\ndeps = [\"B\"]\n");
        let err = Plan::parse(&cyc).unwrap_err();
        assert!(format!("{err:#}").contains("cycle"), "{err:#}");
    }

    #[test]
    fn trailers_drive_status_and_reverts_cancel() {
        let plan = Plan::parse(PLAN).unwrap();
        let msgs = vec![
            msg("a1", "feat: a (#1)\n\nCompletes: A"),
            msg("b1", "feat: b (#2)\n\nCompletes: B"),
            msg("c1", "feat: c part (#3)\n\nPart-of: C, Z"),
            msg("c2", "feat: c (#4)\n\nCompletes: C"),
            msg(
                "r1",
                "Revert \"feat: c (#4)\" (#5)\n\nThis reverts commit c2.",
            ),
        ];
        let r = report(&plan, &derive(&msgs));
        let st: Vec<Status> = r.milestones[0].items.iter().map(|i| i.status).collect();
        assert_eq!(st, vec![Status::Done, Status::Hardware, Status::InProgress]);
        assert_eq!(r.unknown, vec!["Z".to_owned()]);
        let mut more = msgs.clone();
        more.push(msg("v1", "chore: hw (#6)\n\nVerified-on-hardware: B"));
        more.push(msg(
            "r2",
            "Revert \"Revert \"feat: c (#4)\" (#5)\"\n\nThis reverts commit r1.",
        ));
        let r = report(&plan, &derive(&more));
        let st: Vec<Status> = r.milestones[0].items.iter().map(|i| i.status).collect();
        assert_eq!(st, vec![Status::Done, Status::Done, Status::Done]);
        let md = markdown(&r);
        assert!(md.contains("| M0: T | 3 of 3 | 0 | 0 | 0 |"), "{md}");
    }

    fn statuses(plan: &Plan, msgs: &[Message]) -> Vec<Status> {
        report(plan, &derive(msgs)).milestones[0]
            .items
            .iter()
            .map(|i| i.status)
            .collect()
    }

    #[test]
    fn only_trailer_paragraphs_count() {
        let plan = Plan::parse(PLAN).unwrap();
        let prose = msg(
            "p1",
            "docs: plan (#1)\n\nThis PR prepares the item. Next:\nCompletes: A will follow in another PR.\n\n```\nCompletes: C\n```\n",
        );
        assert_eq!(statuses(&plan, &[prose]), vec![Status::Planned; 3]);
        let template = msg(
            "t1",
            "feat: a (#2)\n\n## Summary\nx\n\n<!-- Trailers: keep the one that applies.\n     Claude-Session: always kept. -->\nCompletes: A\nClaude-Session: https://claude.ai/code/session_1\n",
        );
        assert_eq!(
            statuses(&plan, &[template]),
            vec![Status::Done, Status::Planned, Status::Planned]
        );
    }

    #[test]
    fn revert_chains_settle_by_length() {
        let plan = Plan::parse(PLAN).unwrap();
        let mut msgs = vec![msg("a1", "feat: a (#1)\n\nCompletes: A")];
        let mut subject = "feat: a (#1)".to_owned();
        let mut target = "a1".to_owned();
        for n in 1..=4 {
            let sha = format!("r{n}");
            subject = format!("Revert \"{subject}\"");
            msgs.push(msg(
                &sha,
                &format!("{subject}\n\nThis reverts commit {target}."),
            ));
            target = sha;
            let want = if n % 2 == 1 {
                Status::Planned
            } else {
                Status::Done
            };
            assert_eq!(statuses(&plan, &msgs)[0], want, "after {n} reverts");
        }
    }

    #[test]
    fn conventional_revert_titles() {
        let msgs = vec![
            msg("a1", "feat: a (#1)\n\nCompletes: A"),
            msg("r1", "revert: feat: a (#2)\n\nThe item broke main."),
        ];
        assert!(derive(&msgs).completes.is_empty());
        let scoped = vec![
            msg("a1", "feat(x): a (#1)\n\nCompletes: A"),
            msg("r1", "revert(x): feat(x): a"),
        ];
        assert!(derive(&scoped).completes.is_empty());
        assert_eq!(conventional_revert("revert: x (#3)"), Some("x"));
        assert_eq!(conventional_revert("reverted: x"), None);
        assert_eq!(conventional_revert("refactor: x"), None);
    }

    #[test]
    fn verification_counts_only_after_the_latest_completion() {
        let plan = Plan::parse(PLAN).unwrap();
        let msgs = vec![
            msg("a1", "feat: a (#1)\n\nCompletes: A"),
            msg("b1", "feat: b (#2)\n\nCompletes: B"),
            msg("v1", "chore: hw (#3)\n\nVerified-on-hardware: B"),
        ];
        assert_eq!(statuses(&plan, &msgs)[1], Status::Done);
        let mut again = msgs.clone();
        again.push(msg(
            "r1",
            "Revert \"feat: b (#2)\"\n\nThis reverts commit b1.",
        ));
        assert_eq!(statuses(&plan, &again)[1], Status::Planned);
        again.push(msg("b2", "feat: b again (#5)\n\nCompletes: B"));
        assert_eq!(statuses(&plan, &again)[1], Status::Hardware);
        again.push(msg("v2", "chore: hw (#6)\n\nVerified-on-hardware: B"));
        assert_eq!(statuses(&plan, &again)[1], Status::Done);
    }

    #[test]
    fn revert_by_subject() {
        let msgs = vec![
            msg("a1", "feat: a (#1)\n\nCompletes: A"),
            msg("r1", "Revert \"feat: a\" (#2)\n\nReverts owner/repo#1"),
        ];
        assert!(derive(&msgs).completes.is_empty());
    }
}
