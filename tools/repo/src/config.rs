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
    fn finds_job_ids_and_names() {
        let yaml = "name: CI\non:\n  pull_request:\njobs:\n  fmt:\n    runs-on: x\n    steps:\n      - name: not a job\n        run: x\n  docs-job:\n    name: docs\n    runs-on: x\n  \"quoted\": # c\n    name: \"${{ matrix.os }}\"\nother:\n  nope:\n";
        let names: Vec<String> = job_names(yaml).into_iter().collect();
        // `docs-job` reports as `docs`; `quoted`'s name is an expression, so no fixed name.
        assert_eq!(names, vec!["docs", "fmt"]);
    }
}
