//! `repo progress`: roadmap status derived from commit trailers.
//!
//! `[docs].progress` (by default `docs/progress.toml`) holds milestones and items only.
//! Status comes from trailers in the commit messages reachable from HEAD:
//! `Completes: <id>`, `Part-of: <id>` and `Verified-on-hardware: <id>` (several ids may be
//! comma-separated). A commit whose message says `This reverts commit <sha>`, or whose
//! subject is `Revert "<subject>"` naming an earlier commit's subject, cancels that commit's
//! trailers; reverting the revert restores them.

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

/// `(kind, id)` trailers in a message.
fn trailers(body: &str) -> Vec<(&'static str, String)> {
    let mut out = Vec::new();
    for line in body.lines() {
        let Some((key, value)) = line.trim().split_once(':') else {
            continue;
        };
        let kind = [COMPLETES, PART_OF, VERIFIED]
            .into_iter()
            .find(|k| k.eq_ignore_ascii_case(key.trim()));
        if let Some(kind) = kind {
            for id in value.split(',').map(str::trim).filter(|v| !v.is_empty()) {
                out.push((kind, id.trim_matches('`').to_owned()));
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
    let rest = m.subject().strip_prefix("Revert \"")?;
    let inner = rest.rfind('"').map(|k| &rest[..k])?;
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
    let mut cancelled: BTreeSet<&str> = BTreeSet::new();
    for (k, m) in messages.iter().enumerate() {
        if let Some(t) = reverted(m, &messages[..k]) {
            if cancelled.contains(m.sha.as_str()) {
                continue;
            }
            cancelled.insert(&t.sha);
            if let Some(u) = reverted(t, messages) {
                cancelled.remove(u.sha.as_str());
            }
        }
    }
    let mut d = Derived::default();
    for m in messages
        .iter()
        .filter(|m| !cancelled.contains(m.sha.as_str()))
    {
        for (kind, id) in trailers(&m.body) {
            let map = match kind {
                COMPLETES => &mut d.completes,
                PART_OF => &mut d.part_of,
                _ => &mut d.verified,
            };
            map.entry(id).or_default().push(m.sha.clone());
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

    #[test]
    fn revert_by_subject() {
        let msgs = vec![
            msg("a1", "feat: a (#1)\n\nCompletes: A"),
            msg("r1", "Revert \"feat: a\" (#2)\n\nReverts owner/repo#1"),
        ];
        assert!(derive(&msgs).completes.is_empty());
    }
}
