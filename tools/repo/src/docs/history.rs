//! History derived from git: `docs log` and the review clocks behind `docs due`. Nothing
//! about history is stored in files. Revisions are the commits that changed a target's
//! normalized body; verifications are the commits that changed its ledger entries; reviews
//! are the commits that changed its doc's `reviewed` value.

use std::fmt::Write as _;
use std::path::Path;

use anyhow::{Context, Result, bail};

use super::ledger::{self, Ledger};
use super::{Loaded, hash, parse};
use crate::git;

/// A commit as `docs log` shows it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Commit {
    pub sha: String,
    /// Committer time, seconds since the Unix epoch.
    pub time: i64,
    pub author: String,
    pub subject: String,
}

impl Commit {
    /// The PR a squash subject names with its trailing `(#n)`.
    pub fn pr(&self) -> Option<u64> {
        pr_of(&self.subject)
    }
}

/// The PR number in a subject's trailing `(#n)`.
pub fn pr_of(subject: &str) -> Option<u64> {
    let rest = subject.trim_end().strip_suffix(')')?;
    let (_, n) = rest.rsplit_once("(#")?;
    n.parse().ok()
}

/// A verification or review: who and why.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Stamp {
    pub by: String,
    pub note: String,
}

/// What a commit did to the target.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Change {
    /// The n-th change to the normalized body; the first is its creation.
    Revision(usize),
    /// Verified (ledger entries) or reviewed (`reviewed` value), with this stamp.
    Stamped(Stamp),
    /// Ledger entries changed without a new verification, as when a merge took one side.
    Restamped,
    /// The target no longer exists after this commit.
    Removed,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Event {
    pub commit: Commit,
    pub change: Change,
}

/// The log of one doc or section.
#[derive(Debug)]
pub struct Log {
    pub name: String,
    pub path: String,
    pub whole_doc: bool,
    pub review_every: Option<String>,
    /// Oldest first.
    pub events: Vec<Event>,
    pub shallow: bool,
    pub uncommitted: bool,
}

/// The target's state in one version.
#[derive(Debug, Default, PartialEq, Eq)]
struct State {
    body: Option<String>,
    reviewed: Option<Stamp>,
    entries: Option<ledger::Stamp>,
}

fn reviewed_of(doc: &parse::Doc) -> Option<Stamp> {
    let t = doc.meta.as_ref()?.get("reviewed")?.as_table()?;
    let s = |k: &str| t.get(k).and_then(|x| x.as_str()).map(str::to_owned);
    Some(Stamp {
        by: s("by").unwrap_or_default(),
        note: s("note").unwrap_or_default(),
    })
}

fn state(path: &str, text: &str, anchor: Option<&str>) -> State {
    let doc = parse::parse(path, text, &mut Vec::new());
    State {
        body: hash::body(&doc, anchor),
        reviewed: if anchor.is_none() {
            reviewed_of(&doc)
        } else {
            None
        },
        entries: None,
    }
}

/// The `by` and `note` of the first changed or added entry that has them.
fn ledger_stamp(prev: Option<&ledger::Stamp>, now: &ledger::Stamp) -> Option<Stamp> {
    now.iter()
        .filter(|(k, e)| prev.and_then(|p| p.get(*k)) != Some(*e))
        .find_map(|(_, e)| {
            Some(Stamp {
                by: e.by.clone()?,
                note: e.note.clone()?,
            })
        })
}

/// Folds successive states (oldest first; `None`: the file did not exist) into events.
fn events(states: Vec<(Commit, Option<State>)>) -> Vec<Event> {
    let mut out = Vec::new();
    let mut revisions = 0;
    let mut prev = State::default();
    for (commit, st) in states {
        let st = st.unwrap_or_default();
        let mut push = |change| {
            out.push(Event {
                commit: commit.clone(),
                change,
            });
        };
        match (&prev.body, &st.body) {
            (Some(_), None) => push(Change::Removed),
            (old, Some(new)) if old.as_ref() != Some(new) => {
                revisions += 1;
                push(Change::Revision(revisions));
            }
            _ => {}
        }
        if st.reviewed != prev.reviewed {
            if let Some(s) = &st.reviewed {
                push(Change::Stamped(s.clone()));
            }
        }
        if st.entries != prev.entries {
            if let Some(now) = &st.entries {
                match ledger_stamp(prev.entries.as_ref(), now) {
                    Some(s) => push(Change::Stamped(s)),
                    None => push(Change::Restamped),
                }
            }
        }
        prev = st;
    }
    out
}

/// Commits that touched any of `paths` on the first-parent line of `rev`, oldest first.
fn commits(root: &Path, rev: &str, paths: &[&str]) -> Result<Vec<Commit>> {
    let mut args = vec![
        "log",
        "--first-parent",
        "--reverse",
        "--format=%H%x1f%ct%x1f%an%x1f%s%x1e",
        rev,
        "--",
    ];
    args.extend_from_slice(paths);
    let out = git::text(root, &args)?;
    let mut list = Vec::new();
    for rec in out.split('\x1e') {
        let f: Vec<&str> = rec.trim_matches('\n').split('\x1f').collect();
        let [sha, time, author, subject] = f[..] else {
            continue;
        };
        list.push(Commit {
            sha: sha.to_owned(),
            time: time
                .parse()
                .with_context(|| format!("commit time `{time}`"))?,
            author: author.to_owned(),
            subject: subject.to_owned(),
        });
    }
    Ok(list)
}

/// `docs log <DOC>[#anchor] [--rev <rev>]`: the target's history on the first-parent line
/// of `rev`.
pub fn log(l: &Loaded, target: &str, rev: &str) -> Result<Log> {
    let index = super::Index::new(&l.docs);
    let (doc_ref, anchor) = match target.split_once('#') {
        Some((d, a)) => (d, Some(a)),
        None => (target, None),
    };
    let doc = index
        .get(doc_ref)
        .with_context(|| format!("no tracked doc `{doc_ref}`"))?;
    let id = doc.id().to_owned();
    let name = anchor.map_or_else(|| id.clone(), |a| format!("{id}#{a}"));
    let ledger_path = l.config.docs.ledger.as_str();
    let mut paths = vec![doc.path.as_str()];
    if anchor.is_some() {
        paths.push(ledger_path);
    }
    let mut states = Vec::new();
    for commit in commits(&l.root, rev, &paths)? {
        let mut st =
            git::show(&l.root, &commit.sha, &doc.path)?.map(|t| state(&doc.path, &t, anchor));
        if let (Some(st), Some(a)) = (&mut st, anchor) {
            st.entries = git::show(&l.root, &commit.sha, ledger_path)?
                .and_then(|t| Ledger::parse(&t).ok())
                .and_then(|mut led| led.sections.remove(&format!("{id}#{a}")));
        }
        states.push((commit, st));
    }
    let events = events(states);
    if let Some(a) = anchor {
        if events.is_empty() && !doc.has_anchor(a) {
            bail!("no section or rule `{a}` in {}", doc.path);
        }
    }
    let shallow = git::text(&l.root, &["rev-parse", "--is-shallow-repository"])
        .is_ok_and(|o| o.trim() == "true");
    let uncommitted = git::raw(&l.root, &["diff", "--quiet", rev, "--", &doc.path])
        .is_ok_and(|o| o.status.code() == Some(1));
    Ok(Log {
        name,
        path: doc.path.clone(),
        whole_doc: anchor.is_none(),
        review_every: anchor
            .is_none()
            .then(|| doc.meta_str("review_every").map(str::to_owned))
            .flatten(),
        events,
        shallow,
        uncommitted,
    })
}

/// When the value `state` extracts from `path` last changed on HEAD's first-parent line:
/// the time of the newest commit whose version differs from the one before it. `None`
/// when the file has no history.
pub fn last_change<T: PartialEq>(
    root: &Path,
    path: &str,
    state: &dyn Fn(&str) -> T,
) -> Result<Option<i64>> {
    let list = commits(root, "HEAD", &[path])?;
    let mut prev: Option<T> = None;
    let mut last = None;
    for c in list {
        let now = git::show(root, &c.sha, path)?.map(|t| state(&t));
        let differs = match (&prev, &now) {
            (None, Some(_)) => true,
            (Some(p), Some(n)) => p != n,
            _ => false,
        };
        if differs {
            last = Some(c.time);
        }
        prev = now;
    }
    Ok(last)
}

/// `YYYY-MM-DD` (UTC) for seconds since the Unix epoch.
pub fn date(secs: i64) -> String {
    let z = secs.div_euclid(86_400) + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z.rem_euclid(146_097);
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    let y = yoe + era * 400 + i64::from(m <= 2);
    format!("{y:04}-{m:02}-{d:02}")
}

/// Seconds since the Unix epoch, now.
pub fn now() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |d| i64::try_from(d.as_secs()).unwrap_or(i64::MAX))
}

/// The log as text.
pub fn render(log: &Log, now: i64) -> String {
    let mut s = format!("{} ({})\n", log.name, log.path);
    let stamp = if log.whole_doc {
        "reviewed"
    } else {
        "verified"
    };
    let (mut revisions, mut stamps) = (0, 0);
    let mut last_clock = None;
    for e in &log.events {
        let c = &e.commit;
        let pr = c.pr().map_or_else(String::new, |n| format!("#{n}"));
        let what = match &e.change {
            Change::Revision(n) => {
                revisions += 1;
                last_clock = Some(c.time);
                let created = if *n == 1 { "  (created)" } else { "" };
                format!("r{n:<9}{}: {}{created}", c.author, c.subject)
            }
            Change::Stamped(st) => {
                stamps += 1;
                last_clock = Some(c.time);
                format!("{stamp:<10}by {}: {}", st.by, st.note)
            }
            Change::Restamped => "restamped, no new verification".to_owned(),
            Change::Removed => format!("removed   {}: {}", c.author, c.subject),
        };
        let sha = c.sha.get(..9).unwrap_or(&c.sha);
        let _ = writeln!(s, "  {}  {sha}  {pr:<6} {what}", date(c.time));
    }
    if log.events.is_empty() {
        s.push_str("  (no commits)\n");
    }
    let _ = writeln!(s, "{revisions} revision(s), {stamps} {stamp} stamp(s)");
    if let (Some(every), Some(t)) = (&log.review_every, last_clock) {
        if let Some(days) = super::check::days(every) {
            let due = t + days * 86_400;
            let state = if due <= now { "due now" } else { "not due" };
            let _ = writeln!(
                s,
                "review every {every}: clock restarted {}, due {} ({state})",
                date(t),
                date(due)
            );
        }
    }
    if log.uncommitted {
        s.push_str("note: the working tree has uncommitted changes to this file, not shown\n");
    }
    if log.shallow {
        s.push_str(
            "note: this clone is shallow, so older history is missing (`git fetch --unshallow`)\n",
        );
    }
    s
}

#[cfg(test)]
mod tests {
    use super::*;

    fn commit(n: i64, subject: &str) -> Commit {
        Commit {
            sha: format!("{n:040}"),
            time: n * 86_400,
            author: "a".into(),
            subject: subject.into(),
        }
    }

    #[test]
    fn pr_from_squash_subject() {
        assert_eq!(pr_of("feat: x (#135)"), Some(135));
        assert_eq!(pr_of("fix (#12) and more"), None);
        assert_eq!(pr_of("Merge branch main"), None);
    }

    #[test]
    fn dates() {
        assert_eq!(date(0), "1970-01-01");
        assert_eq!(date(951_782_400), "2000-02-29");
        assert_eq!(date(-86_400), "1969-12-31");
    }

    #[test]
    fn folds_states_into_events() {
        let st = |body: Option<&str>, rev: Option<(&str, &str)>| {
            Some(State {
                body: body.map(str::to_owned),
                reviewed: rev.map(|(by, note)| Stamp {
                    by: by.into(),
                    note: note.into(),
                }),
                entries: None,
            })
        };
        let ev = events(vec![
            (commit(1, "add"), st(Some("A"), None)),
            (commit(2, "review"), st(Some("A"), Some(("agent:x", "n1")))),
            (commit(3, "other"), st(Some("A"), Some(("agent:x", "n1")))),
            (commit(4, "edit"), st(Some("B"), Some(("agent:x", "n1")))),
            (commit(5, "drop"), st(None, None)),
            (commit(6, "deleted"), None),
            (commit(7, "back"), st(Some("C"), None)),
        ]);
        let got: Vec<(i64, Change)> = ev
            .into_iter()
            .map(|e| (e.commit.time / 86_400, e.change))
            .collect();
        assert_eq!(
            got,
            vec![
                (1, Change::Revision(1)),
                (
                    2,
                    Change::Stamped(Stamp {
                        by: "agent:x".into(),
                        note: "n1".into()
                    })
                ),
                (4, Change::Revision(2)),
                (5, Change::Removed),
                (7, Change::Revision(3)),
            ]
        );
    }

    #[test]
    fn ledger_entries_fold_into_stamps() {
        let entry = |h: &str, who: Option<(&str, &str)>| ledger::Entry {
            hash: h.into(),
            by: who.map(|w| w.0.into()),
            note: who.map(|w| w.1.into()),
        };
        let with = |entries: &[(&str, ledger::Entry)]| {
            Some(State {
                body: Some("A".into()),
                reviewed: None,
                entries: Some(
                    entries
                        .iter()
                        .map(|(k, e)| ((*k).to_owned(), e.clone()))
                        .collect(),
                ),
            })
        };
        let ev = events(vec![
            (
                commit(1, "verify"),
                with(&[("body", entry("h1", Some(("agent:x", "n1"))))]),
            ),
            (
                commit(2, "merge"),
                with(&[
                    ("body", entry("h1", Some(("agent:x", "n1")))),
                    ("file:b", entry("h4", None)),
                ]),
            ),
        ]);
        let got: Vec<Change> = ev.into_iter().map(|e| e.change).collect();
        assert_eq!(
            got,
            vec![
                Change::Revision(1),
                Change::Stamped(Stamp {
                    by: "agent:x".into(),
                    note: "n1".into()
                }),
                Change::Restamped,
            ]
        );
    }
}
