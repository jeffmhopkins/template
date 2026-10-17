//! `repo docs …`: checks and maintains the documentation graph.

mod backrefs;
mod check;
mod due;
pub mod generate;
mod govern;
pub mod hash;
pub mod history;
pub mod ledger;
pub mod merge;
pub mod parse;

use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};

use anyhow::{Context, Result, bail};

use crate::Outcome;
use crate::args::Args;
use crate::config::{Config, Pack, Repo};
use crate::git;
pub use parse::{Diag, Doc};

/// Notes that say nothing about what was checked.
const STOCK_NOTES: &[&str] = &[
    "ok",
    "lgtm",
    "verified",
    "checked",
    "re-read",
    "reread",
    "no change",
    "no changes",
    "still accurate",
    "still holds",
    "looks good",
    "n/a",
];
const MIN_NOTE: usize = 20;

/// Which Markdown files are tracked docs.
pub struct Tracked {
    include: crate::glob::Globs,
    exclude: crate::glob::Globs,
}

impl Tracked {
    pub fn new(config: &Config) -> Result<Self> {
        Ok(Self {
            include: crate::glob::Globs::new(&config.docs.tracked)?,
            exclude: crate::glob::Globs::new(&config.docs.exclude)?,
        })
    }

    pub fn is_tracked(&self, path: &str) -> bool {
        path.ends_with(".md") && self.include.is_match(path) && !self.exclude.is_match(path)
    }
}

/// Docs looked up by id or by path.
pub struct Index<'a> {
    by_path: BTreeMap<&'a str, &'a Doc>,
    by_id: BTreeMap<&'a str, &'a Doc>,
}

impl<'a> Index<'a> {
    pub fn new(docs: &'a [Doc]) -> Self {
        Self {
            by_path: docs.iter().map(|d| (d.path.as_str(), d)).collect(),
            by_id: docs
                .iter()
                .filter_map(|d| Some((d.meta_str("id")?, d)))
                .collect(),
        }
    }

    /// A doc by id, else by repo path.
    pub fn get(&self, r: &str) -> Option<&'a Doc> {
        self.by_id.get(r).or_else(|| self.by_path.get(r)).copied()
    }

    pub fn by_path(&self, p: &str) -> Option<&'a Doc> {
        self.by_path.get(p).copied()
    }
}

/// The repository's files and parsed docs.
pub struct Loaded {
    pub root: PathBuf,
    pub config: Config,
    pub pack: Pack,
    /// Tracked files present in the working tree, sorted.
    pub files: Vec<String>,
    pub docs: Vec<Doc>,
    pub parse_diags: Vec<Diag>,
}

pub fn load(repo: &Repo) -> Result<Loaded> {
    let files: Vec<String> = git::ls_files(&repo.root, &[])?
        .into_iter()
        .filter(|p| repo.root.join(p).is_file())
        .collect();
    let tracked = Tracked::new(&repo.config)?;
    let mut parse_diags = Vec::new();
    let mut docs = Vec::new();
    for p in files.iter().filter(|p| tracked.is_tracked(p)) {
        let text = std::fs::read_to_string(repo.root.join(p))
            .with_context(|| format!("reading {p}"))?
            .replace("\r\n", "\n");
        docs.push(parse::parse(p, &text, &mut parse_diags));
    }
    Ok(Loaded {
        root: repo.root.clone(),
        config: repo.config.clone(),
        pack: repo.pack.clone(),
        files,
        docs,
        parse_diags,
    })
}

/// String values of `key` in a metadata table.
pub fn strings(meta: &toml::Table, key: &str) -> Vec<String> {
    meta.get(key)
        .and_then(|v| v.as_array())
        .map(|a| {
            a.iter()
                .filter_map(|x| x.as_str().map(str::to_owned))
                .collect()
        })
        .unwrap_or_default()
}

/// The keys of every section that needs a stamp: one with `covers` or `depends_on`.
pub fn stamped_sections(docs: &[Doc]) -> BTreeSet<String> {
    docs.iter()
        .flat_map(|d| {
            d.sections.iter().filter_map(move |s| {
                let meta = s.meta.as_ref()?;
                (!strings(meta, "covers").is_empty() || !strings(meta, "depends_on").is_empty())
                    .then(|| format!("{}#{}", d.id(), s.anchor))
            })
        })
        .collect()
}

/// Every docs finding in the repository.
pub fn check_repo(l: &Loaded, draft: bool) -> Vec<Diag> {
    let mut diags = l.parse_diags.clone();
    let ledger = check::check_ledger(l, &mut diags);
    let cx = check::Ctx {
        l,
        index: Index::new(&l.docs),
        files: hash::Files::new(&l.root, &l.files, &l.config.docs.ledger),
        ledger,
        draft,
    };
    diags.extend(check::check(&cx));
    diags.extend(backrefs::check(l, &cx.index, &cx.files));
    diags.extend(generate::check(l, draft));
    if let Err(e) = crate::progress::Plan::load(&l.root, &l.config.docs.progress) {
        diags.push(Diag::error(&l.config.docs.progress, 1, format!("{e:#}")));
    }
    diags.sort();
    diags.dedup();
    diags
}

/// Checks a stamp's `--by` and `--note`; returns the trimmed note.
fn check_stamp<'a>(by: &str, note: &'a str) -> Result<&'a str> {
    let note = note.trim();
    let plain = note.to_lowercase();
    let plain = plain.trim_end_matches(['.', '!']);
    if note.chars().count() < MIN_NOTE || STOCK_NOTES.contains(&plain) {
        bail!(
            "the note must say what you checked and why it still holds (at least {MIN_NOTE} characters, no stock phrases)"
        );
    }
    if note.contains("-->") || by.contains("-->") {
        bail!("`-->` is not allowed in a stamp");
    }
    let name = by
        .strip_prefix("agent:")
        .or_else(|| by.strip_prefix("human:"));
    if name.is_none_or(|n| n.trim().is_empty()) {
        bail!("--by must be `agent:<name>` or `human:<github-user>`");
    }
    Ok(note)
}

/// `docs verify <DOC#anchor>`: writes the section's current input hashes into the ledger.
pub fn verify(repo: &Repo, target: &str, by: &str, note: &str) -> Result<Vec<String>> {
    let note = check_stamp(by, note)?;
    let (doc_ref, anchor) = target
        .split_once('#')
        .context("target must be `DOC#anchor`")?;
    let l = load(repo)?;
    let index = Index::new(&l.docs);
    let doc = index
        .get(doc_ref)
        .with_context(|| format!("no tracked doc `{doc_ref}`"))?;
    let section = doc
        .section(anchor)
        .with_context(|| format!("no section `{anchor}` in {}", doc.path))?;
    let Some(meta) = &section.meta else {
        bail!("section `{anchor}` has no `@section` block");
    };
    let (covers, deps) = (strings(meta, "covers"), strings(meta, "depends_on"));
    if covers.is_empty() && deps.is_empty() {
        bail!("section `{anchor}` has no `covers` or `depends_on`, so it has nothing to verify");
    }
    let ledger_path = l.config.docs.ledger.as_str();
    let untracked = git::ls_files(&l.root, &["--others", "--exclude-standard"])?;
    let pending = hash::Files::new(&l.root, &untracked, ledger_path).matching(&covers)?;
    if !pending.is_empty() {
        bail!(
            "untracked files match this section's `covers`; `git add` them first: {}",
            pending.join(", ")
        );
    }
    let files = hash::Files::new(&l.root, &l.files, ledger_path);
    let inputs = hash::inputs(doc, anchor, &covers, &deps, &files, &index)?;
    let key = format!("{}#{anchor}", doc.id());
    let mut ledger = ledger::Ledger::read(&l.root, ledger_path)?;
    let changed = ledger::diff(ledger.sections.get(&key), &inputs);
    let stamp = ledger::restamp(ledger.sections.get(&key), &inputs, by, note);
    ledger.sections.insert(key, stamp);
    let keep = stamped_sections(&l.docs);
    ledger.prune(&|k| keep.contains(k));
    ledger.write(&l.root, ledger_path)?;
    Ok(changed)
}

/// `docs review <DOC>`: writes `reviewed = { by, note }` into the `@doc` block.
pub fn review(repo: &Repo, target: &str, by: &str, note: &str) -> Result<String> {
    let note = check_stamp(by, note)?;
    if target.contains('#') {
        bail!("`docs review` stamps a whole doc; a section is re-read with `docs verify`");
    }
    let l = load(repo)?;
    let index = Index::new(&l.docs);
    let doc = index
        .get(target)
        .with_context(|| format!("no tracked doc `{target}`"))?;
    if doc.is_adr() {
        bail!(
            "ADRs have no review interval; a decision that no longer holds gets a superseding ADR"
        );
    }
    if doc.meta_str("review_every").is_none() {
        bail!("{} has no `review_every`", doc.path);
    }
    let Some((start, end)) = doc.doc_block.filter(|_| doc.meta.is_some()) else {
        bail!("{} has no valid `@doc` block", doc.path);
    };
    let line = format!(
        "reviewed = {{ by = {}, note = {} }}",
        hash::toml_str(by),
        hash::toml_str(note)
    );
    let mut lines: Vec<String> = doc.text.lines().map(str::to_owned).collect();
    let is_key = |l: &str| l.split('=').next().unwrap_or_default().trim() == "reviewed";
    match (start + 1..end).find(|&k| is_key(&lines[k])) {
        Some(k) if lines[k] == line => bail!(
            "{} already records this review; say what you re-read this time",
            doc.path
        ),
        Some(k) => lines[k] = line,
        None => lines.insert(end, line),
    }
    let mut out = lines.join("\n");
    if doc.text.ends_with('\n') {
        out.push('\n');
    }
    std::fs::write(l.root.join(&doc.path), out).with_context(|| format!("writing {}", doc.path))?;
    Ok(doc.id().to_owned())
}

/// `arg` as a repo-relative path with `/` separators; absolute paths must lie in `root`.
fn repo_path(root: &Path, arg: &str) -> Result<String> {
    let p = Path::new(arg);
    let rel = if p.is_absolute() {
        let root = root.canonicalize().context("resolving the repo root")?;
        let lexical = check::normalize(p)
            .map(PathBuf::from)
            .unwrap_or_else(|| p.to_path_buf());
        let lexical = if lexical.is_absolute() {
            lexical
        } else {
            Path::new("/").join(lexical)
        };
        let mut existing = lexical.as_path();
        let mut rest = Vec::new();
        let base = loop {
            if let Ok(c) = existing.canonicalize() {
                break c;
            }
            rest.push(existing.file_name().context("no existing ancestor")?);
            existing = existing.parent().context("no existing ancestor")?;
        };
        let full = rest.iter().rev().fold(base, |acc, part| acc.join(part));
        full.strip_prefix(&root)
            .with_context(|| format!("`{arg}` is outside the repository"))?
            .to_path_buf()
    } else {
        p.to_path_buf()
    };
    check::normalize(&rel).with_context(|| format!("`{arg}` escapes the repository"))
}

fn print_diags(diags: &[Diag]) -> usize {
    for d in diags {
        let kind = if d.warning { "warning: " } else { "" };
        println!("{}:{}: {kind}{}", d.path, d.line, d.msg);
    }
    diags.iter().filter(|d| !d.warning).count()
}

/// Writes every generated block; returns the paths written and the problems.
pub fn write_generated(l: &Loaded) -> Result<generate::Generated> {
    let body = crate::progress::block_for(l, &["HEAD"])?;
    let g = generate::generate(l, body.as_deref());
    for (path, _, text) in &g.files {
        std::fs::write(l.root.join(path), text).with_context(|| format!("writing {path}"))?;
    }
    Ok(g)
}

/// Entry point for `repo docs <sub> <args>`.
pub fn run(sub: &str, args: &[String]) -> Outcome {
    let stamp_args = |usage: &str| -> Result<(String, String, String)> {
        let a = Args::parse(args, &["by", "note"], &[])?;
        let target = a.exactly(1, usage)?[0].clone();
        Ok((
            target,
            a.required("by", usage)?.to_owned(),
            a.required("note", usage)?.to_owned(),
        ))
    };
    match sub {
        "check" => {
            let a = Args::parse(args, &[], &["draft"])?;
            a.exactly(0, "repo docs check [--draft]")?;
            let l = load(&Repo::open()?)?;
            let diags = check_repo(&l, a.switch("draft"));
            let errors = print_diags(&diags);
            let warnings = diags.len() - errors;
            let draft = a.switch("draft");
            if errors == 0 {
                println!("docs check: ok ({warnings} warning(s))");
            } else if draft {
                println!(
                    "docs check --draft: {errors} problem(s), {warnings} warning(s); not failing a draft"
                );
            } else {
                println!("docs check: {errors} problem(s), {warnings} warning(s)");
            }
            Ok(errors == 0 || draft)
        }
        "verify" => {
            let (target, by, note) = stamp_args(
                "repo docs verify <DOC#anchor> --by <agent:..|human:..> --note \"<what you checked>\"",
            )?;
            let changed = verify(&Repo::open()?, &target, &by, &note)?;
            if changed.is_empty() {
                println!("verified {target}: already fresh, nothing to record");
            } else {
                println!("verified {target}: {}", changed.join(", "));
            }
            Ok(true)
        }
        "review" => {
            let (target, by, note) = stamp_args(
                "repo docs review <DOC> --by <agent:..|human:..> --note \"<what you re-read>\"",
            )?;
            let id = review(&Repo::open()?, &target, &by, &note)?;
            println!("reviewed {id}");
            Ok(true)
        }
        "generate" => {
            Args::parse(args, &[], &[])?.exactly(0, "repo docs generate")?;
            let l = load(&Repo::open()?)?;
            let g = write_generated(&l)?;
            let problems = print_diags(&g.problems);
            for (path, _, _) in &g.files {
                println!("generated {path}");
            }
            println!(
                "docs generate: {} file(s) written, {problems} problem(s)",
                g.files.len()
            );
            Ok(problems == 0)
        }
        "log" => {
            let a = Args::parse(args, &["rev"], &[])?;
            let target = &a.exactly(1, "repo docs log <DOC[#anchor]> [--rev <rev>]")?[0];
            let l = load(&Repo::open()?)?;
            let log = history::log(&l, target, a.value("rev").unwrap_or("HEAD"))?;
            print!("{}", history::render(&log, history::now()));
            Ok(true)
        }
        "for" => {
            let a = Args::parse(args, &[], &[])?;
            let arg = &a.exactly(1, "repo docs for <path>")?[0];
            let repo = Repo::open()?;
            let path = repo_path(&repo.root, arg)?;
            let l = load(&repo)?;
            let index = Index::new(&l.docs);
            let g = govern::governing(&l, &index, &path);
            print!("{}", govern::render(&path, &g, &index));
            let parent = Path::new(&path).parent().unwrap_or(Path::new(""));
            if !l.files.contains(&path) && !l.root.join(parent).is_dir() {
                println!(
                    "\nnote: `{path}` is not tracked and its directory does not exist; paths are relative to the repository root"
                );
            }
            Ok(true)
        }
        "due" => {
            Args::parse(args, &[], &[])?.exactly(0, "repo docs due")?;
            let l = load(&Repo::open()?)?;
            print!("{}", due::render(&due::due(&l, history::now())?));
            Ok(true)
        }
        "merge-main" => {
            let a = Args::parse(args, &["from"], &[])?;
            a.exactly(0, "repo docs merge-main [--from <branch>]")?;
            let branch = a.value("from").unwrap_or(merge::DEFAULT_BRANCH);
            let root = git::root()?;
            let m = merge::merge_main(&root, branch)?;
            for path in &m.generated {
                println!("resolved @generated conflicts and regenerated: {path}");
            }
            for s in &m.stamps {
                println!("resolved ledger conflict: {s}");
            }
            for (path, cmd) in &m.relocked {
                println!("relocked: {path}: took the merged side, then ran `{cmd}`");
            }
            for (path, why) in &m.refused {
                println!("left for you: {path}: {why}");
            }
            for s in &m.stale {
                println!("stale: {s}");
                for n in m.notes.get(s).into_iter().flatten() {
                    println!("  previous note: {n}");
                }
            }
            if m.committed {
                println!(
                    "docs merge-main: merged; {} section(s) to re-read, then `scripts/repo docs verify <DOC#anchor> --by <who> --note \"<what you checked>\"`, keeping what the previous note said about this branch and adding what the merge changed",
                    m.stale.len()
                );
            } else {
                println!(
                    "docs merge-main: the ledger and @generated conflicts are resolved and staged; resolve the files above, run `scripts/repo docs generate`, `git commit`, then `scripts/repo docs check`"
                );
            }
            Ok(m.committed && m.stale.is_empty())
        }
        other => bail!("unknown docs command `{other}`; see `repo help`"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn stamps_need_a_real_note_and_who() {
        assert!(check_stamp("agent:a", "ok").is_err());
        assert!(check_stamp("agent:a", "Verified.").is_err());
        assert!(check_stamp("agent:a", "short note").is_err());
        assert!(check_stamp("bob", "checked the table against the code").is_err());
        assert!(check_stamp("agent:", "checked the table against the code").is_err());
        assert!(check_stamp("agent:a", "checked --> the table against code").is_err());
        assert_eq!(
            check_stamp("human:x", " checked the table against the code ").unwrap(),
            "checked the table against the code"
        );
    }

    #[test]
    fn repo_paths_stay_inside() {
        let dir = std::env::temp_dir().join(format!("repo-path-{}", std::process::id()));
        std::fs::create_dir_all(dir.join("a")).unwrap();
        assert_eq!(repo_path(&dir, "./a/../a/x.rs").unwrap(), "a/x.rs");
        assert!(repo_path(&dir, "../outside").is_err());
        let abs = dir.join("a/new/file.rs");
        assert_eq!(
            repo_path(&dir, &abs.to_string_lossy()).unwrap(),
            "a/new/file.rs"
        );
        assert!(repo_path(&dir, "/etc/passwd").is_err());
        let _ = std::fs::remove_dir_all(&dir);
    }
}
