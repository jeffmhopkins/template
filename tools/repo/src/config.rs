//! `repo.toml`, the stack pack (`stacks/<pack>/pack.toml`) and `repo config check`.

use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};

use anyhow::{Context, Result, bail};
use serde::Deserialize;

use crate::Outcome;

/// The config file at the repository root.
pub const FILE: &str = "repo.toml";

/// `repo.toml`.
#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Config {
    pub template: Template,
    pub stack: Stack,
    pub docs: Docs,
    pub checks: Checks,
    pub governance: Governance,
    pub labels: Labels,
    pub review: Review,
    pub lanes: Lanes,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Template {
    /// The template's GitHub repository, as `<owner>/<repo>`. In the template itself it names
    /// the repository it is in. Optional when parsing, so an older revision's config still
    /// reads; `config check` requires it.
    #[serde(default)]
    pub source: Option<String>,
    pub version: String,
    /// The template commit the project's files came from, as a full SHA. A project records
    /// it at intake and moves it on each upgrade, which diffs from it. Absent in the template.
    #[serde(default)]
    pub commit: Option<String>,
}

/// Why `commit` is not a full commit SHA, if it isn't.
pub fn commit_problem(commit: &str) -> Option<String> {
    (commit.len() != 40
        || !commit
            .bytes()
            .all(|b| matches!(b, b'0'..=b'9' | b'a'..=b'f')))
    .then(|| format!("{FILE}: [template].commit `{commit}` is not a full 40-character commit SHA"))
}

/// Why `source` is not a GitHub `<owner>/<repo>` name, if it isn't.
pub fn source_problem(source: &str) -> Option<String> {
    let ok_part = |p: &str| {
        !p.is_empty()
            && p != "."
            && p != ".."
            && p.bytes()
                .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'-' | b'_' | b'.'))
    };
    match source.split_once('/') {
        Some((owner, repo)) if ok_part(owner) && ok_part(repo) => None,
        _ => Some(format!(
            "{FILE}: [template].source `{source}` is not a GitHub `<owner>/<repo>` name"
        )),
    }
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Stack {
    /// The pack directory under `stacks/`.
    pub pack: String,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Docs {
    /// Globs of the Markdown files that are tracked docs.
    pub tracked: Vec<String>,
    /// Globs excluded from `tracked`.
    #[serde(default)]
    pub exclude: Vec<String>,
    /// The most files a section's `covers` may match, unless they all lie in one package.
    pub max_cover_files: usize,
    pub ledger: String,
    pub facts: String,
    pub external: String,
    pub progress: String,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Checks {
    /// Required check names; each must be a job in `.github/workflows/`.
    pub required: Vec<String>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Governance {
    /// Globs whose every edit is a governance change.
    pub protected: Vec<String>,
    /// Globs whose edits only the maintainer may approve.
    #[serde(default)]
    pub maintainer_only: Vec<String>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Labels {
    pub governance: String,
    #[serde(rename = "override")]
    pub override_: String,
    pub human: String,
    pub hardware: String,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Review {
    pub marker: String,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Lanes {
    pub file: String,
}

/// `stacks/<pack>/pack.toml`.
#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Pack {
    /// The file whose presence at the repository root means the stack is in use.
    pub detect: String,
    /// CI job name → shell command.
    #[serde(default)]
    pub jobs: BTreeMap<String, String>,
    #[serde(default)]
    pub suppressions: Suppressions,
    /// Manifest glob → dotted table names (`lints`, `workspace.lints`) whose edits are
    /// governance changes.
    #[serde(default)]
    pub protected_tables: BTreeMap<String, Vec<String>>,
    /// Lockfile glob → the shell command that relocks it, run in the lockfile's directory.
    /// `docs merge-main` resolves a conflict in a matching file by taking the merged branch's
    /// side and running the command.
    #[serde(default)]
    pub lockfiles: BTreeMap<String, String>,
    pub backrefs: Backrefs,
}

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Suppressions {
    /// Regular expressions; a line matching one is a lint suppression.
    #[serde(default)]
    pub patterns: Vec<String>,
    /// Whether a suppression line must carry `reason = "..."`.
    #[serde(default)]
    pub reason_required: bool,
    /// Globs of the files scanned; empty means every file that is not Markdown.
    #[serde(default)]
    pub files: Vec<String>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Backrefs {
    /// Globs of package root files, each of which carries a back-reference line.
    pub roots: Vec<String>,
    /// The text that starts a back-reference line, such as `//! Docs:`.
    pub prefix: String,
}

impl Config {
    pub fn parse(text: &str) -> Result<Self> {
        toml::from_str(text).map_err(|e| anyhow::anyhow!("{FILE}: {}", e.message()))
    }

    pub fn load(root: &Path) -> Result<Self> {
        let text =
            std::fs::read_to_string(root.join(FILE)).with_context(|| format!("reading {FILE}"))?;
        Self::parse(&text)
    }

    /// The stack pack's path, relative to the root.
    pub fn pack_path(&self) -> String {
        format!("stacks/{}/pack.toml", self.stack.pack)
    }
}

impl Pack {
    pub fn parse(path: &str, text: &str) -> Result<Self> {
        toml::from_str(text).map_err(|e| anyhow::anyhow!("{path}: {}", e.message()))
    }

    pub fn load(root: &Path, config: &Config) -> Result<Self> {
        let path = config.pack_path();
        let text = std::fs::read_to_string(root.join(&path))
            .with_context(|| format!("reading the stack pack {path}"))?;
        Self::parse(&path, &text)
    }
}

/// The repository root with its config and stack pack.
pub struct Repo {
    pub root: PathBuf,
    pub config: Config,
    pub pack: Pack,
}

impl Repo {
    /// Finds the root from the current directory and loads both files.
    pub fn open() -> Result<Self> {
        Self::at(crate::git::root()?)
    }

    pub fn at(root: PathBuf) -> Result<Self> {
        let config = Config::load(&root)?;
        let pack = Pack::load(&root, &config)?;
        Ok(Self { root, config, pack })
    }
}

/// The check names of each workflow file under `.github/workflows/` at `root`.
fn workflow_jobs(root: &Path) -> Result<BTreeMap<String, BTreeSet<String>>> {
    let dir = root.join(".github/workflows");
    let mut out = BTreeMap::new();
    let Ok(entries) = std::fs::read_dir(&dir) else {
        return Ok(out);
    };
    let mut paths: Vec<PathBuf> = entries
        .filter_map(|e| e.ok().map(|e| e.path()))
        .filter(|p| p.extension().is_some_and(|x| x == "yml" || x == "yaml"))
        .collect();
    paths.sort();
    for p in paths {
        let text =
            std::fs::read_to_string(&p).with_context(|| format!("reading {}", p.display()))?;
        let name = p
            .file_name()
            .unwrap_or_default()
            .to_string_lossy()
            .into_owned();
        out.insert(name, job_names(&text));
    }
    Ok(out)
}

/// A YAML scalar without quotes or a trailing comment.
fn scalar(v: &str) -> String {
    let v = v.trim();
    let v = match v.find(" #") {
        Some(k) if !v.starts_with(['"', '\'']) => &v[..k],
        _ => v,
    };
    v.trim().trim_matches(['"', '\'']).to_owned()
}

/// The check names of the jobs under the top-level `jobs:` key of a workflow: a job's
/// `name:` when it has one, else its id. A job whose `name:` is an expression has no fixed
/// check name and adds none.
pub fn job_names(yaml: &str) -> BTreeSet<String> {
    // (id, name): `None` no `name:` yet; `Some(None)` an expression.
    let mut jobs: Vec<(String, Option<Option<String>>)> = Vec::new();
    let mut in_jobs = false;
    let mut job_indent: Option<usize> = None;
    // Inside a job: the indent of its own keys, once seen.
    let mut key_indent: Option<Option<usize>> = None;
    for line in yaml.lines() {
        let trimmed = line.trim_start();
        if trimmed.is_empty() || trimmed.starts_with('#') {
            continue;
        }
        let indent = line.len() - trimmed.len();
        if indent == 0 {
            in_jobs = trimmed.trim_end() == "jobs:";
            job_indent = None;
            key_indent = None;
            continue;
        }
        if !in_jobs {
            continue;
        }
        let ji = *job_indent.get_or_insert(indent);
        if indent <= ji {
            key_indent = None;
            if indent == ji {
                if let Some((id, rest)) = trimmed.split_once(':') {
                    let rest = rest.trim();
                    if rest.is_empty() || rest.starts_with('#') {
                        jobs.push((scalar(id), None));
                        key_indent = Some(None);
                    }
                }
            }
            continue;
        }
        if let Some(ki) = key_indent.as_mut() {
            let ki = *ki.get_or_insert(indent);
            if indent == ki {
                if let Some(n) = trimmed.strip_prefix("name:").map(scalar) {
                    if let Some(job) = jobs.last_mut() {
                        if !n.is_empty() {
                            job.1 = Some((!n.contains("${{")).then_some(n));
                        }
                    }
                }
            }
        }
    }
    jobs.into_iter()
        .filter_map(|(id, name)| match name {
            None => Some(id),
            Some(n) => n,
        })
        .collect()
}

/// Problems with `config`, `pack` and `lanes.toml` at `root`.
pub fn check(root: &Path) -> Vec<String> {
    let mut out = Vec::new();
    let config = match Config::load(root) {
        Ok(c) => c,
        Err(e) => return vec![format!("{e:#}")],
    };
    let globs = |what: &str, list: &[String], out: &mut Vec<String>| {
        for g in list {
            if let Err(e) = crate::glob::compile(g) {
                out.push(format!("{FILE}: {what}: {e:#}"));
            }
        }
    };
    globs("[docs].tracked", &config.docs.tracked, &mut out);
    globs("[docs].exclude", &config.docs.exclude, &mut out);
    globs(
        "[governance].protected",
        &config.governance.protected,
        &mut out,
    );
    globs(
        "[governance].maintainer_only",
        &config.governance.maintainer_only,
        &mut out,
    );
    if config.template.version.split('.').count() != 3
        || !config
            .template
            .version
            .split('.')
            .all(|p| !p.is_empty() && p.bytes().all(|b| b.is_ascii_digit()))
    {
        out.push(format!(
            "{FILE}: [template].version `{}` is not MAJOR.MINOR.PATCH",
            config.template.version
        ));
    }
    match &config.template.source {
        Some(source) => out.extend(source_problem(source)),
        None => out.push(format!("{FILE}: [template].source is missing")),
    }
    if let Some(commit) = &config.template.commit {
        out.extend(commit_problem(commit));
    }
    if config.docs.max_cover_files == 0 {
        out.push(format!("{FILE}: [docs].max_cover_files must be at least 1"));
    }
    for (what, p) in [
        ("ledger", &config.docs.ledger),
        ("facts", &config.docs.facts),
        ("external", &config.docs.external),
        ("progress", &config.docs.progress),
        ("lanes file", &config.lanes.file),
    ] {
        if p.is_empty() || p.starts_with('/') || p.split('/').any(|c| c == "..") {
            out.push(format!("{FILE}: {what} path `{p}` must be repo-relative"));
        }
    }
    if config.stack.pack.is_empty() || config.stack.pack.contains(['/', '\\', '.']) {
        out.push(format!(
            "{FILE}: [stack].pack `{}` must be a directory name under stacks/",
            config.stack.pack
        ));
    }
    let mut seen = BTreeSet::new();
    for c in &config.checks.required {
        if !seen.insert(c) {
            out.push(format!("{FILE}: [checks].required lists `{c}` twice"));
        }
    }
    for (what, label) in [
        ("governance", &config.labels.governance),
        ("override", &config.labels.override_),
        ("human", &config.labels.human),
        ("hardware", &config.labels.hardware),
        ("review marker", &config.review.marker),
    ] {
        if label.trim().is_empty() {
            out.push(format!("{FILE}: {what} must not be empty"));
        }
    }

    match Pack::load(root, &config) {
        Err(e) => out.push(format!("{e:#}")),
        Ok(pack) => {
            let path = config.pack_path();
            for p in &pack.suppressions.patterns {
                if let Err(e) = regex::Regex::new(p) {
                    out.push(format!("{path}: suppression pattern `{p}`: {e}"));
                }
            }
            for (g, tables) in &pack.protected_tables {
                if let Err(e) = crate::glob::compile(g) {
                    out.push(format!("{path}: [protected_tables]: {e:#}"));
                }
                if tables
                    .iter()
                    .any(|t| t.is_empty() || t.split('.').any(str::is_empty))
                {
                    out.push(format!(
                        "{path}: [protected_tables] `{g}`: table names are dotted keys such as `workspace.lints`"
                    ));
                }
            }
            for g in pack.suppressions.files.iter().chain(&pack.backrefs.roots) {
                if let Err(e) = crate::glob::compile(g) {
                    out.push(format!("{path}: {e:#}"));
                }
            }
            if pack.backrefs.prefix.trim().is_empty() {
                out.push(format!("{path}: [backrefs].prefix must not be empty"));
            }
            if pack.detect.is_empty() || pack.detect.contains('/') {
                out.push(format!("{path}: `detect` must be a file name at the root"));
            }
            for (job, cmd) in &pack.jobs {
                if cmd.trim().is_empty() {
                    out.push(format!("{path}: job `{job}` has an empty command"));
                }
            }
            for (g, cmd) in &pack.lockfiles {
                if let Err(e) = crate::glob::compile(g) {
                    out.push(format!("{path}: [lockfiles]: {e:#}"));
                }
                if cmd.trim().is_empty() {
                    out.push(format!("{path}: [lockfiles] `{g}` has an empty command"));
                }
            }
        }
    }

    if let Err(e) = crate::lanes::read_file(root, &config.lanes.file) {
        out.push(format!("{e:#}"));
    }

    match workflow_jobs(root) {
        Err(e) => out.push(format!("{e:#}")),
        Ok(files) => {
            let found: BTreeSet<&String> = files.values().flatten().collect();
            for c in &config.checks.required {
                if !found.contains(c) {
                    out.push(format!(
                        "{FILE}: required check `{c}` is not a check name in .github/workflows/*.yml (a job's `name:`, else its id)"
                    ));
                }
            }
        }
    }
    out.extend(check_constants(root, &config));
    out.extend(check_maintainer_copies(root, &config));
    out
}

/// The `repo.toml` values that files under `.github/` hard-code, by the annotation that marks
/// each copy: `[labels].<key>` or `[review].marker`.
fn constants(config: &Config) -> Vec<(&'static str, &str)> {
    vec![
        ("[labels].governance", config.labels.governance.as_str()),
        ("[labels].override", config.labels.override_.as_str()),
        ("[labels].human", config.labels.human.as_str()),
        ("[labels].hardware", config.labels.hardware.as_str()),
        ("[review].marker", config.review.marker.as_str()),
    ]
}

/// The files that must carry an annotated copy of a constant, and which.
const COPIES: &[(&str, &str)] = &[
    (".github/workflows/governance.yml", "[labels].governance"),
    (".github/workflows/review-gate.yml", "[labels].override"),
    (".github/workflows/review-gate.yml", "[review].marker"),
    (".github/workflows/sweep.yml", "[labels].governance"),
    (".github/workflows/sweep.yml", "[labels].override"),
    (
        ".github/ISSUE_TEMPLATE/hardware-run.yml",
        "[labels].hardware",
    ),
];

/// Whether `value` occurs in `line` with no word character (`[A-Za-z0-9_-]`) on either side.
fn has_word(line: &str, value: &str) -> bool {
    let word =
        |c: Option<char>| c.is_some_and(|c| c.is_ascii_alphanumeric() || c == '_' || c == '-');
    line.match_indices(value).any(|(k, _)| {
        !word(line[..k].chars().next_back()) && !word(line[k + value.len()..].chars().next())
    })
}

/// Whether `value` occurs in `code` as a literal: quoted, or as a whole YAML value.
fn has_literal(code: &str, value: &str) -> bool {
    ['\'', '"', '`']
        .iter()
        .any(|q| code.contains(&format!("{q}{value}{q}")))
        || code
            .split_once(": ")
            .is_some_and(|(_, v)| v.trim().trim_matches(['\'', '"']) == value)
}

/// A line without its trailing comment (` #` or ` //` onward); empty for a comment line.
fn code_part(line: &str) -> &str {
    let t = line.trim_start();
    if t.starts_with('#') || t.starts_with("//") {
        return "";
    }
    let cut = [" #", " //"].iter().filter_map(|m| line.find(m)).min();
    cut.map_or(line, |k| &line[..k])
}

/// Problems with the hard-coded copies of `repo.toml` values in one file's text. A line holds
/// an annotated copy when it, or the comment line right above it, names the key; that line
/// must contain the value. A quoted or bare-value literal of a value on a line not annotated
/// for it is a copy nothing checks.
pub fn constant_problems(path: &str, text: &str, config: &Config) -> Vec<String> {
    let consts = constants(config);
    let mut out = Vec::new();
    let lines: Vec<&str> = text.lines().collect();
    for (i, line) in lines.iter().enumerate() {
        let mut keys: Vec<&str> = consts
            .iter()
            .map(|c| c.0)
            .filter(|k| line.contains(k))
            .collect();
        if code_part(line).is_empty() && !keys.is_empty() {
            // A comment's annotation is for the line below it.
            continue;
        }
        if let Some(above) = i.checked_sub(1).map(|k| lines[k]) {
            if code_part(above).is_empty() {
                keys.extend(consts.iter().map(|c| c.0).filter(|k| above.contains(k)));
            }
        }
        for (key, value) in &consts {
            if keys.contains(key) && !has_word(line, value) {
                out.push(format!(
                    "{path}:{}: annotated with {key}, but doesn't contain its value `{value}` from {FILE}",
                    i + 1
                ));
            } else if !keys.contains(key) && has_literal(code_part(line), value) {
                out.push(format!(
                    "{path}:{}: hard-codes `{value}` without the annotation `{key} in {FILE}`",
                    i + 1
                ));
            }
        }
    }
    out
}

/// Whether `text` has a line annotated with `key` (on it, or on the comment line above).
fn annotated(text: &str, key: &str) -> bool {
    let lines: Vec<&str> = text.lines().collect();
    lines.iter().enumerate().any(|(i, l)| {
        (l.contains(key) && !code_part(l).is_empty())
            || (code_part(l).is_empty() && l.contains(key) && i + 1 < lines.len())
    })
}

/// The files under `.github/` that may hard-code `repo.toml` values: everything but tests and
/// Markdown.
fn github_files(root: &Path) -> Vec<PathBuf> {
    fn walk(dir: &Path, out: &mut Vec<PathBuf>) {
        let Ok(entries) = std::fs::read_dir(dir) else {
            return;
        };
        for e in entries.filter_map(Result::ok) {
            let p = e.path();
            if p.is_dir() {
                walk(&p, out);
            } else if p.extension().is_none_or(|x| x != "md") {
                out.push(p);
            }
        }
    }
    let mut out = Vec::new();
    walk(&root.join(".github"), &mut out);
    out.retain(|p| !p.starts_with(root.join(".github/tests")));
    out.sort();
    out
}

/// Every hard-coded copy of a `repo.toml` value under `.github/` matches it, and the files
/// that need a copy have one.
fn check_constants(root: &Path, config: &Config) -> Vec<String> {
    let mut out = Vec::new();
    for p in github_files(root) {
        let Ok(text) = std::fs::read_to_string(&p) else {
            continue;
        };
        let rel = p
            .strip_prefix(root)
            .unwrap_or(&p)
            .to_string_lossy()
            .replace('\\', "/");
        out.extend(constant_problems(&rel, &text, config));
        for (file, key) in COPIES.iter().filter(|(f, _)| *f == rel) {
            if !annotated(&text, key) {
                out.push(format!(
                    "{file}: no line is annotated `{key} in {FILE}`, so its copy of that value is unchecked"
                ));
            }
        }
    }
    out
}

/// The rule whose `applies_to` copies `[governance].maintainer_only`, and its file.
pub const MAINTAINER_RULE: (&str, &str) = ("docs/RULES.md", "R-GOV-007");
/// The pasted instructions, whose maintainer-only paragraph copies it too.
pub const INSTRUCTIONS: &str = "docs/agents/project-instructions.md";
/// The paragraph of [`INSTRUCTIONS`] that lists the paths.
const INSTRUCTIONS_LEAD: &str = "**Maintainer-only paths**";

/// The backticked paths listed in the instructions' maintainer-only paragraph: the code spans
/// after its first colon outside code spans, up to the first `.` outside one.
pub fn instructions_paths(text: &str) -> Option<Vec<String>> {
    let start = text.find(INSTRUCTIONS_LEAD)?;
    let rest = &text[start..];
    let mut in_code = false;
    let mut listing = false;
    let mut span = String::new();
    let mut out = Vec::new();
    for c in rest.chars() {
        match (c, in_code) {
            ('`', false) => {
                in_code = true;
                span.clear();
            }
            ('`', true) => {
                in_code = false;
                if listing {
                    out.push(span.clone());
                }
            }
            (_, true) => span.push(c),
            (':', false) => listing = true,
            ('.', false) if listing => return Some(out),
            _ => {}
        }
    }
    None
}

/// The copies of `[governance].maintainer_only` list the same paths.
fn check_maintainer_copies(root: &Path, config: &Config) -> Vec<String> {
    let want: BTreeSet<&str> = config
        .governance
        .maintainer_only
        .iter()
        .map(String::as_str)
        .collect();
    let mut out = Vec::new();
    let compare = |what: String, got: BTreeSet<&str>| {
        let missing: Vec<&&str> = want.difference(&got).collect();
        let extra: Vec<&&str> = got.difference(&want).collect();
        (!missing.is_empty() || !extra.is_empty()).then(|| {
            format!(
                "{what} differs from [governance].maintainer_only in {FILE}: missing {missing:?}, extra {extra:?}"
            )
        })
    };
    let (rules_file, rule_id) = MAINTAINER_RULE;
    if let Ok(text) = std::fs::read_to_string(root.join(rules_file)) {
        let doc = crate::docs::parse::parse(rules_file, &text, &mut Vec::new());
        match doc.rule(rule_id) {
            None => out.push(format!("{rules_file}: rule {rule_id} is missing")),
            Some(r) => {
                let list: Vec<String> = r
                    .meta
                    .as_ref()
                    .and_then(|m| m.get("applies_to"))
                    .and_then(toml::Value::as_array)
                    .map(|a| {
                        a.iter()
                            .filter_map(|v| v.as_str().map(str::to_owned))
                            .collect()
                    })
                    .unwrap_or_default();
                out.extend(compare(
                    format!("{rules_file}: {rule_id}'s applies_to"),
                    list.iter().map(String::as_str).collect(),
                ));
            }
        }
    }
    if let Ok(text) = std::fs::read_to_string(root.join(INSTRUCTIONS)) {
        match instructions_paths(&text) {
            None => out.push(format!(
                "{INSTRUCTIONS}: no {INSTRUCTIONS_LEAD} paragraph listing paths after a colon"
            )),
            Some(list) => out.extend(compare(
                format!("{INSTRUCTIONS}: the {INSTRUCTIONS_LEAD} list"),
                list.iter().map(String::as_str).collect(),
            )),
        }
    }
    out
}

/// `repo config check`.
pub fn run_check(args: &[String]) -> Outcome {
    if !args.is_empty() {
        bail!("usage: repo config check");
    }
    let root = crate::git::root()?;
    let problems = check(&root);
    for p in &problems {
        println!("{p}");
    }
    if problems.is_empty() {
        println!("config check: ok");
    } else {
        println!("config check: {} problem(s)", problems.len());
    }
    Ok(problems.is_empty())
}

#[cfg(test)]
pub mod tests {
    use super::*;

    pub const CONFIG: &str = r#"
[template]
source = "owner/template"
version = "0.1.0"

[stack]
pack = "rust"

[docs]
tracked = ["*.md", "docs/**/*.md"]
exclude = ["CHANGELOG.md"]
max_cover_files = 12
ledger = "docs/verified.toml"
facts = "docs/facts.toml"
external = "docs/external.toml"
progress = "docs/progress.toml"

[checks]
required = ["fmt", "docs"]

[governance]
protected = [".github/**", "repo.toml"]
maintainer_only = ["REVIEW.md"]

[labels]
governance = "governance-approved"
override = "review-override"
human = "needs-human"
hardware = "hardware"

[review]
marker = "review v1"

[lanes]
file = "lanes.toml"
"#;

    pub const PACK: &str = r#"
detect = "Cargo.toml"
[jobs]
fmt = "cargo fmt --all --check"
[suppressions]
patterns = ['#\[allow\(']
reason_required = true
[backrefs]
roots = ["**/src/lib.rs", "**/src/main.rs"]
prefix = "//! Docs:"
"#;

    #[test]
    fn parses_and_rejects_unknown_keys() {
        let c = Config::parse(CONFIG).unwrap();
        assert_eq!(c.labels.override_, "review-override");
        assert_eq!(c.pack_path(), "stacks/rust/pack.toml");
        let bad = CONFIG.replace("[review]", "[review]\nextra = 1");
        let err = Config::parse(&bad).unwrap_err();
        assert!(format!("{err:#}").contains("unknown field"), "{err:#}");
        let missing = CONFIG.replace("marker = \"review v1\"", "");
        assert!(Config::parse(&missing).is_err());
        let p = Pack::parse("p", PACK).unwrap();
        assert!(p.suppressions.reason_required);
        assert!(Pack::parse("p", &format!("{PACK}\nnope = 1")).is_err());
    }

    #[test]
    fn template_source_is_checked() {
        let c = Config::parse(CONFIG).unwrap();
        assert_eq!(c.template.source.as_deref(), Some("owner/template"));
        let missing = CONFIG.replace("source = \"owner/template\"\n", "");
        assert_eq!(Config::parse(&missing).unwrap().template.source, None);
        let unknown = CONFIG.replace("[template]", "[template]\nupstream = \"x/y\"");
        assert!(Config::parse(&unknown).is_err());
        for good in ["owner/template", "my-org/repo.name", "a_b/c-1"] {
            assert_eq!(source_problem(good), None, "{good}");
        }
        for bad in [
            "",
            "template",
            "/repo",
            "owner/",
            "a/b/c",
            "owner/..",
            "https://github.com/a/b",
            "a b/c",
        ] {
            assert!(source_problem(bad).is_some(), "{bad}");
        }
    }

    #[test]
    fn template_commit_is_a_full_sha() {
        assert_eq!(Config::parse(CONFIG).unwrap().template.commit, None);
        let sha = "0123456789abcdef0123456789abcdef01234567";
        let with = CONFIG.replace(
            "version = \"0.1.0\"",
            &format!("version = \"0.1.0\"\ncommit = \"{sha}\""),
        );
        assert_eq!(
            Config::parse(&with).unwrap().template.commit.as_deref(),
            Some(sha)
        );
        assert_eq!(commit_problem(sha), None);
        for bad in [
            "",
            "0123456",
            "v0.1.0",
            &sha.to_uppercase(),
            &format!("{sha}0"),
        ] {
            assert!(commit_problem(bad).is_some(), "{bad}");
        }
    }

    #[test]
    fn hard_coded_constants_match_the_config() {
        let c = Config::parse(CONFIG).unwrap();
        let ok = "env:\n  # [labels].governance in repo.toml.\n  GOVERNANCE_LABEL: governance-approved\n  # [labels].governance and [labels].override in repo.toml.\n  AUDIT: governance-approved review-override\n            const MARKER = 'review v1';   // [review].marker in repo.toml\n  # A comment may say review-override freely.\n  - name: Remove governance-approved after a push\n  pull-requests: write   # removes review-override\nlabels: [\"hardware\"]   # [labels].hardware in repo.toml\n";
        assert!(
            constant_problems("w.yml", ok, &c).is_empty(),
            "{:?}",
            constant_problems("w.yml", ok, &c)
        );
        // The config changed and the copy didn't.
        let renamed =
            Config::parse(&CONFIG.replace("\"review-override\"", "\"skip-review\"")).unwrap();
        let got = constant_problems("w.yml", ok, &renamed);
        assert_eq!(got.len(), 1, "{got:?}");
        assert!(
            got[0].starts_with("w.yml:5:") && got[0].contains("skip-review"),
            "{got:?}"
        );
        // A literal nothing annotates.
        let bare =
            "      const applied = events.filter((e) => e.label?.name === 'review-override');\n";
        let got = constant_problems("w.yml", bare, &c);
        assert!(
            got.len() == 1 && got[0].contains("without the annotation `[labels].override"),
            "{got:?}"
        );
        let value = "  LABEL: needs-human\n";
        assert_eq!(constant_problems("w.yml", value, &c).len(), 1);
        // A longer word is not the value.
        let marker = "  // [review].marker in repo.toml\n  const MARKER = 'review v10';\n";
        assert_eq!(constant_problems("w.yml", marker, &c).len(), 1);
        assert!(annotated(ok, "[review].marker"));
        assert!(!annotated("x: 1\n", "[labels].override"));
    }

    #[test]
    fn maintainer_only_copies_are_parsed() {
        let text = "x\n**Maintainer-only paths** (`[governance].maintainer_only` in `repo.toml`), which\nneed the maintainer, even with `governance-approved`: `REVIEW.md`,\n`.github/**` and `docs/a-*.md`. A PR that changes `x`.\n";
        assert_eq!(
            instructions_paths(text).unwrap(),
            vec!["REVIEW.md", ".github/**", "docs/a-*.md"]
        );
        assert_eq!(instructions_paths("no list here"), None);
    }

    #[test]
    fn finds_job_ids_and_names() {
        let yaml = "name: CI\non:\n  pull_request:\njobs:\n  fmt:\n    runs-on: x\n    steps:\n      - name: not a job\n        run: x\n  docs-job:\n    name: docs\n    runs-on: x\n  \"quoted\": # c\n    name: \"${{ matrix.os }}\"\nother:\n  nope:\n";
        let names: Vec<String> = job_names(yaml).into_iter().collect();
        // `docs-job` reports as `docs`; `quoted`'s name is an expression, so no fixed name.
        assert_eq!(names, vec!["docs", "fmt"]);
    }
}
