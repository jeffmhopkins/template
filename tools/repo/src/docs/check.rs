//! Cross-document checks: identity, metadata schema, references, rules, facts, external
//! URLs, `covers` bounds and ledger freshness.

use std::collections::{BTreeMap, BTreeSet};
use std::path::{Component, Path, PathBuf};

use super::hash::{self, Files};
use super::ledger::{self, Ledger};
use super::parse::{Diag, Doc};
use super::{Index, Loaded, strings};

const DOC_KEYS: &[&str] = &[
    "id",
    "title",
    "kind",
    "status",
    "review_every",
    "reviewed",
    "tier",
    "superseded_by",
];
const SECTION_KEYS: &[&str] = &["covers", "depends_on"];
const RULE_KEYS: &[&str] = &["status", "enforced_by", "applies_to"];
pub const KINDS: &[&str] = &[
    "normative",
    "guide",
    "runbook",
    "brief",
    "design",
    "adr",
    "product",
    "readme",
];
const DOC_STATUSES: &[&str] = &["active", "draft", "retired"];
const ADR_STATUSES: &[&str] = &["proposed", "accepted", "superseded", "rejected"];
const TIERS: &[&str] = &["agent", "maintainer"];
const EXTERNAL_KEYS: &[&str] = &[
    "id",
    "url",
    "recheck_every",
    "relied_on",
    "title",
    "note",
    "checked",
];
const FACT_KEYS: &[&str] = &["id", "value", "note", "check", "source"];

/// Everything the checks read, besides the docs.
pub struct Ctx<'a> {
    pub l: &'a Loaded,
    pub index: Index<'a>,
    pub files: Files<'a>,
    pub ledger: Ledger,
    /// `--draft`: stale sections are warnings.
    pub draft: bool,
}

/// Days in a duration such as `"90d"`.
pub fn days(s: &str) -> Option<i64> {
    s.strip_suffix('d')?.parse().ok().filter(|&d| d > 0)
}

/// Whether `id` has the shape `ADR-NNNN`.
pub fn is_adr_id(id: &str) -> bool {
    id.strip_prefix("ADR-")
        .is_some_and(|n| n.len() == 4 && n.bytes().all(|b| b.is_ascii_digit()))
}

/// Runs every cross-document check.
pub fn check(cx: &Ctx<'_>) -> Vec<Diag> {
    let mut out = Vec::new();
    let ext_urls = check_external(cx, &mut out);
    let facts = check_facts(cx, &mut out);
    let required: BTreeSet<&str> =
        cx.l.config
            .checks
            .required
            .iter()
            .map(String::as_str)
            .collect();
    let mut ids: BTreeMap<&str, &str> = BTreeMap::new();
    let mut rule_ids: BTreeMap<&str, &str> = BTreeMap::new();
    for doc in &cx.l.docs {
        check_doc_meta(doc, &cx.index, &mut out);
        if let Some(id) = doc.meta_str("id") {
            if let Some(other) = ids.insert(id, &doc.path) {
                out.push(Diag::error(
                    &doc.path,
                    1,
                    format!("duplicate doc id `{id}` (also in {other})"),
                ));
            }
        }
        let mut seen = BTreeSet::new();
        for (a, line) in &doc.anchors {
            if !seen.insert(a.as_str()) {
                out.push(Diag::error(
                    &doc.path,
                    *line,
                    format!("duplicate anchor `{a}`"),
                ));
            }
        }
        for s in &doc.sections {
            if let Some(meta) = &s.meta {
                check_section(cx, doc, s.line, &s.anchor, meta, &mut out);
            }
        }
        for r in &doc.rules {
            if let Some(other) = rule_ids.insert(&r.id, &doc.path) {
                if other != doc.path {
                    out.push(Diag::error(
                        &doc.path,
                        r.line,
                        format!("rule `{}` is also defined in {other}", r.id),
                    ));
                }
            }
            if let Some(meta) = &r.meta {
                check_rule(doc, r.line, &r.id, meta, &required, &mut out);
            }
        }
        for link in &doc.links {
            check_link(cx, doc, &link.dest, link.line, &ext_urls, &mut out);
        }
        for (url, line) in &doc.bare_urls {
            out.push(Diag::error(
                &doc.path,
                *line,
                format!(
                    "bare URL `{url}`: write it as a Markdown link registered in {}",
                    cx.l.config.docs.external
                ),
            ));
        }
        for f in doc.facts.iter().filter(|_| !doc.is_adr()) {
            match facts.get(f.id.as_str()) {
                None => out.push(Diag::error(
                    &doc.path,
                    f.line,
                    format!("unknown fact `{}`", f.id),
                )),
                Some(value) if !f.before.trim_end().ends_with(value.as_str()) => {
                    out.push(Diag::error(
                        &doc.path,
                        f.line,
                        format!(
                            "fact `{}` must be written right before its marker as `{value}`",
                            f.id
                        ),
                    ));
                }
                Some(_) => {}
            }
        }
    }
    out
}

fn check_doc_meta(doc: &Doc, index: &Index<'_>, out: &mut Vec<Diag>) {
    let Some(meta) = &doc.meta else { return };
    let p = doc.path.as_str();
    for k in meta.keys() {
        if !DOC_KEYS.contains(&k.as_str()) {
            out.push(Diag::error(p, 1, format!("unknown `@doc` key `{k}`")));
        }
    }
    for k in ["id", "title", "kind", "status"] {
        if doc.meta_str(k).is_none_or(|v| v.trim().is_empty()) {
            out.push(Diag::error(
                p,
                1,
                format!("`@doc` needs a non-empty string `{k}`"),
            ));
        }
    }
    let kind = doc.meta_str("kind").unwrap_or_default();
    if !kind.is_empty() && !KINDS.contains(&kind) {
        out.push(Diag::error(
            p,
            1,
            format!("unknown kind `{kind}` (one of {})", KINDS.join(", ")),
        ));
    }
    let status = doc.status();
    let adr = kind == "adr";
    let allowed = if adr { ADR_STATUSES } else { DOC_STATUSES };
    if !status.is_empty() && !allowed.contains(&status) {
        out.push(Diag::error(
            p,
            1,
            format!(
                "status `{status}` is not valid for kind `{kind}` (one of {})",
                allowed.join(", ")
            ),
        ));
    }
    if let Some(v) = meta.get("review_every") {
        if adr {
            out.push(Diag::error(p, 1, "ADRs have no `review_every`"));
        } else if v.as_str().and_then(days).is_none() {
            out.push(Diag::error(p, 1, "`review_every` must look like \"90d\""));
        }
    }
    if let Some(v) = meta.get("reviewed") {
        let ok = v.as_table().is_some_and(|t| {
            t.len() == 2
                && ["by", "note"]
                    .iter()
                    .all(|k| t.get(*k).and_then(|x| x.as_str()).is_some())
        });
        if !ok {
            out.push(Diag::error(
                p,
                1,
                "`reviewed` must be `{ by = \"…\", note = \"…\" }`, written by `repo docs review`",
            ));
        }
    }
    if adr {
        let id = doc.meta_str("id").unwrap_or_default();
        if !is_adr_id(id) {
            out.push(Diag::error(
                p,
                1,
                format!("ADR id `{id}` must look like `ADR-0001`"),
            ));
        }
        match doc.meta_str("tier") {
            Some(t) if TIERS.contains(&t) => {}
            Some(t) => out.push(Diag::error(
                p,
                1,
                format!("tier `{t}` must be `agent` or `maintainer`"),
            )),
            None => out.push(Diag::error(
                p,
                1,
                "ADRs need `tier` (`agent` or `maintainer`)",
            )),
        }
        match (status == "superseded", doc.meta_str("superseded_by")) {
            (true, None) | (false, Some(_)) => out.push(Diag::error(
                p,
                1,
                "`superseded_by` is required exactly when status is `superseded`",
            )),
            (true, Some(next)) => {
                if !index.get(next).is_some_and(Doc::is_adr) {
                    out.push(Diag::error(
                        p,
                        1,
                        format!("`superseded_by` `{next}` is not a tracked ADR id"),
                    ));
                }
            }
            (false, None) => {}
        }
    } else {
        for k in ["tier", "superseded_by"] {
            if meta.contains_key(k) {
                out.push(Diag::error(p, 1, format!("`{k}` is for ADRs only")));
            }
        }
    }
    for g in &doc.generated {
        if !super::generate::NAMES.contains(&g.name.as_str()) {
            out.push(Diag::error(
                p,
                g.begin + 1,
                format!(
                    "unknown `@generated` block `{}` (one of {})",
                    g.name,
                    super::generate::NAMES.join(", ")
                ),
            ));
        }
        if g.name == super::generate::STATUS && !adr {
            out.push(Diag::error(
                p,
                g.begin + 1,
                "`adr-status` blocks belong in ADRs only",
            ));
        }
    }
}

/// An array-of-strings metadata value, reporting anything else.
fn string_array(
    meta: &toml::Table,
    key: &str,
    path: &str,
    line: usize,
    out: &mut Vec<Diag>,
) -> Vec<String> {
    match meta.get(key) {
        None => Vec::new(),
        Some(v) => match v.as_array() {
            Some(a) if a.iter().all(toml::Value::is_str) => strings(meta, key),
            _ => {
                out.push(Diag::error(
                    path,
                    line,
                    format!("`{key}` must be an array of strings"),
                ));
                Vec::new()
            }
        },
    }
}

/// The package that holds `file`: the deepest directory below the root that contains the
/// stack pack's detect file.
pub fn package_of<'a>(file: &str, packages: &[&'a str]) -> Option<&'a str> {
    packages
        .iter()
        .copied()
        .filter(|d| d.is_empty() || file.starts_with(&format!("{d}/")))
        .max_by_key(|d| d.len())
}

/// Package directories: every non-root directory holding a tracked `detect` file.
pub fn packages<'a>(paths: &'a [String], detect: &str) -> Vec<&'a str> {
    let suffix = format!("/{detect}");
    paths
        .iter()
        .filter_map(|p| p.strip_suffix(&suffix))
        .collect()
}

fn check_section(
    cx: &Ctx<'_>,
    doc: &Doc,
    line: usize,
    anchor: &str,
    meta: &toml::Table,
    out: &mut Vec<Diag>,
) {
    let p = doc.path.as_str();
    for k in meta.keys() {
        if !SECTION_KEYS.contains(&k.as_str()) {
            out.push(Diag::error(
                p,
                line,
                format!("section `{anchor}`: unknown key `{k}`"),
            ));
        }
    }
    let covers = string_array(meta, "covers", p, line, out);
    let deps = string_array(meta, "depends_on", p, line, out);
    let status = doc.status();
    let mut all = BTreeSet::new();
    for g in covers.iter().filter(|_| status != "retired") {
        match cx.files.matching(std::slice::from_ref(g)) {
            Ok(m) if m.is_empty() => {
                let msg =
                    format!("section `{anchor}`: `covers` glob `{g}` matches no tracked file");
                out.push(if status == "draft" {
                    Diag::warning(p, line, msg)
                } else {
                    Diag::error(p, line, msg)
                });
            }
            Ok(m) => all.extend(m),
            Err(e) => out.push(Diag::error(p, line, format!("section `{anchor}`: {e:#}"))),
        }
    }
    let max = cx.l.config.docs.max_cover_files;
    if all.len() > max {
        let pkgs = packages(cx.files.paths(), &cx.l.pack.detect);
        let owners: BTreeSet<Option<&str>> = all.iter().map(|f| package_of(f, &pkgs)).collect();
        let one_package = owners.len() == 1 && owners.iter().all(Option::is_some);
        if !one_package {
            out.push(Diag::error(
                p,
                line,
                format!(
                    "section `{anchor}`: `covers` matches {} files across more than one package (at most {max}, or one package)",
                    all.len()
                ),
            ));
        }
    }
    for dep in &deps {
        let (doc_ref, frag) = dep
            .split_once('#')
            .map_or((dep.as_str(), None), |(d, a)| (d, Some(a)));
        match cx.index.get(doc_ref) {
            None => out.push(Diag::error(
                p,
                line,
                format!("section `{anchor}`: `depends_on` target `{dep}` is not a tracked doc"),
            )),
            Some(t) if frag.is_some_and(|a| t.section(a).is_none() && t.rule(a).is_none()) => {
                out.push(Diag::error(
                    p,
                    line,
                    format!("section `{anchor}`: `depends_on` `{dep}` names no section or rule"),
                ));
            }
            Some(t)
                if !doc.is_adr() && matches!(t.status(), "superseded" | "retired" | "rejected") =>
            {
                out.push(Diag::error(
                    p,
                    line,
                    format!(
                        "section `{anchor}`: `depends_on` points at {} `{doc_ref}`",
                        t.status()
                    ),
                ));
            }
            Some(_) => {}
        }
    }
    if covers.is_empty() && deps.is_empty() {
        return;
    }
    let key = format!("{}#{anchor}", doc.id());
    let stamp = cx.ledger.sections.get(&key);
    match hash::inputs(doc, anchor, &covers, &deps, &cx.files, &cx.index) {
        Ok(inputs) => {
            let changed = ledger::diff(stamp, &inputs);
            if !changed.is_empty() {
                let why = if stamp.is_none() {
                    format!("no stamp in {}", cx.l.config.docs.ledger)
                } else {
                    format!("changed since its stamp: {}", changed.join(", "))
                };
                let msg = format!(
                    "section `{anchor}` is stale ({why}): re-read it against the change; if it still holds run `repo docs verify {key} --by <who> --note \"<what you checked>\"`, otherwise edit it"
                );
                out.push(if cx.draft {
                    Diag::warning(p, line, msg)
                } else {
                    Diag::error(p, line, msg)
                });
            }
        }
        Err(e) => out.push(Diag::error(p, line, format!("section `{anchor}`: {e:#}"))),
    }
}

fn check_rule(
    doc: &Doc,
    line: usize,
    id: &str,
    meta: &toml::Table,
    required: &BTreeSet<&str>,
    out: &mut Vec<Diag>,
) {
    let p = doc.path.as_str();
    for k in meta.keys() {
        if !RULE_KEYS.contains(&k.as_str()) {
            out.push(Diag::error(
                p,
                line,
                format!("rule `{id}`: unknown key `{k}`"),
            ));
        }
    }
    let parts: Vec<&str> = id.split('-').collect();
    let shaped = parts.len() == 3
        && !parts[1].is_empty()
        && parts[1].bytes().all(|b| b.is_ascii_uppercase())
        && parts[2].len() == 3
        && parts[2].bytes().all(|b| b.is_ascii_digit());
    if !shaped {
        out.push(Diag::error(
            p,
            line,
            format!("rule id `{id}` must look like `R-DOC-001`"),
        ));
    }
    match meta.get("status").and_then(|v| v.as_str()) {
        Some("active" | "retired") => {}
        other => out.push(Diag::error(
            p,
            line,
            format!("rule `{id}`: status must be `active` or `retired`, got {other:?}"),
        )),
    }
    for g in string_array(meta, "applies_to", p, line, out) {
        if let Err(e) = crate::glob::compile(&g) {
            out.push(Diag::error(
                p,
                line,
                format!("rule `{id}`: `applies_to`: {e:#}"),
            ));
        }
    }
    let enforcers = string_array(meta, "enforced_by", p, line, out);
    if enforcers.is_empty() && meta.get("status").and_then(|v| v.as_str()) == Some("active") {
        out.push(Diag::error(
            p,
            line,
            format!("rule `{id}`: an active rule needs `enforced_by`"),
        ));
    }
    for e in enforcers {
        let ok = match e.split_once(':') {
            Some(("ci", name)) => required.contains(name),
            None => e == "review" || e == "human",
            _ => false,
        };
        if !ok {
            out.push(Diag::error(
                p,
                line,
                format!("rule `{id}`: `enforced_by` entry `{e}` must be `review`, `human` or `ci:<a [checks].required name>`"),
            ));
        }
    }
}

fn check_link(
    cx: &Ctx<'_>,
    doc: &Doc,
    dest: &str,
    line: usize,
    ext_urls: &BTreeSet<String>,
    out: &mut Vec<Diag>,
) {
    let p = doc.path.as_str();
    if dest.starts_with("http://") || dest.starts_with("https://") {
        if !ext_urls.contains(dest) {
            out.push(Diag::error(
                p,
                line,
                format!(
                    "external URL `{dest}` is not registered in {}",
                    cx.l.config.docs.external
                ),
            ));
        }
        return;
    }
    if dest.starts_with("mailto:") || dest.contains("://") {
        return;
    }
    let (file, frag) = dest.split_once('#').unwrap_or((dest, ""));
    let target_path = if file.is_empty() {
        doc.path.clone()
    } else {
        let base = Path::new(&doc.path).parent().unwrap_or(Path::new(""));
        match normalize(&base.join(file)) {
            Some(t) => t,
            None => {
                out.push(Diag::error(
                    p,
                    line,
                    format!("link `{dest}` escapes the repository"),
                ));
                return;
            }
        }
    };
    if !cx.l.root.join(&target_path).exists() {
        out.push(Diag::error(
            p,
            line,
            format!("link `{dest}`: `{target_path}` does not exist"),
        ));
        return;
    }
    let Some(target) = cx.index.by_path(&target_path) else {
        return;
    };
    if !frag.is_empty() && !target.has_anchor(frag) {
        out.push(Diag::error(
            p,
            line,
            format!("link `{dest}`: no anchor `{frag}` in {target_path}"),
        ));
    }
    let index_doc = doc
        .generated
        .iter()
        .any(|g| g.name == super::generate::INDEX);
    if !doc.is_adr() && !index_doc && target.is_adr() && target.status() == "superseded" {
        out.push(Diag::error(
            p,
            line,
            format!("link `{dest}` cites superseded {target_path}; cite its replacement"),
        ));
    }
}

/// Resolves `.` and `..` lexically; `None` if the path escapes the root.
pub fn normalize(p: &Path) -> Option<String> {
    let mut out = PathBuf::new();
    for c in p.components() {
        match c {
            Component::CurDir => {}
            Component::ParentDir => {
                if !out.pop() {
                    return None;
                }
            }
            other => out.push(other),
        }
    }
    Some(out.to_string_lossy().replace('\\', "/"))
}

/// Reads a TOML registry; a missing file is empty.
fn registry(cx: &Ctx<'_>, path: &str, out: &mut Vec<Diag>) -> Option<toml::Table> {
    match std::fs::read_to_string(cx.l.root.join(path)) {
        Ok(text) => match text.parse::<toml::Table>() {
            Ok(t) => Some(t),
            Err(e) => {
                out.push(Diag::error(
                    path,
                    1,
                    format!("does not parse: {}", e.message()),
                ));
                None
            }
        },
        Err(_) => None,
    }
}

/// The `[[name]]` tables of a registry.
fn entries<'t>(
    t: &'t toml::Table,
    name: &str,
    path: &str,
    out: &mut Vec<Diag>,
) -> Vec<&'t toml::Table> {
    for k in t.keys().filter(|k| k.as_str() != name) {
        out.push(Diag::error(
            path,
            1,
            format!("unknown top-level key `{k}` (expected `[[{name}]]`)"),
        ));
    }
    match t.get(name) {
        None => Vec::new(),
        Some(v) => match v.as_array() {
            Some(a) => a.iter().filter_map(toml::Value::as_table).collect(),
            None => {
                out.push(Diag::error(
                    path,
                    1,
                    format!("`{name}` must be `[[{name}]]` entries"),
                ));
                Vec::new()
            }
        },
    }
}

/// Validates the external-URL registry; returns the registered URLs.
fn check_external(cx: &Ctx<'_>, out: &mut Vec<Diag>) -> BTreeSet<String> {
    let path = cx.l.config.docs.external.as_str();
    let mut urls = BTreeSet::new();
    let Some(t) = registry(cx, path, out) else {
        return urls;
    };
    let mut ids = BTreeSet::new();
    for e in entries(&t, "external", path, out) {
        let get = |k: &str| e.get(k).and_then(|v| v.as_str());
        let id = get("id").unwrap_or("?");
        for k in e.keys().filter(|k| !EXTERNAL_KEYS.contains(&k.as_str())) {
            out.push(Diag::error(
                path,
                1,
                format!("external `{id}`: unknown key `{k}`"),
            ));
        }
        for k in ["id", "url", "recheck_every"] {
            if get(k).is_none() {
                out.push(Diag::error(
                    path,
                    1,
                    format!("external `{id}`: missing string `{k}`"),
                ));
            }
        }
        if get("recheck_every").is_some_and(|r| days(r).is_none()) {
            out.push(Diag::error(
                path,
                1,
                format!("external `{id}`: `recheck_every` must look like \"180d\""),
            ));
        }
        if !ids.insert(id.to_owned()) {
            out.push(Diag::error(
                path,
                1,
                format!("duplicate external id `{id}`"),
            ));
        }
        if let Some(u) = get("url") {
            if !urls.insert(u.to_owned()) {
                out.push(Diag::error(
                    path,
                    1,
                    format!("URL `{u}` is registered twice"),
                ));
            }
        }
    }
    urls
}

/// Validates the facts registry; returns id → value.
fn check_facts(cx: &Ctx<'_>, out: &mut Vec<Diag>) -> BTreeMap<String, String> {
    let path = cx.l.config.docs.facts.as_str();
    let mut facts = BTreeMap::new();
    let Some(t) = registry(cx, path, out) else {
        return facts;
    };
    for f in entries(&t, "fact", path, out) {
        let (Some(id), Some(value)) = (
            f.get("id").and_then(|v| v.as_str()),
            f.get("value").and_then(|v| v.as_str()),
        ) else {
            out.push(Diag::error(
                path,
                1,
                "every fact needs string `id` and `value`",
            ));
            continue;
        };
        for k in f.keys().filter(|k| !FACT_KEYS.contains(&k.as_str())) {
            out.push(Diag::error(
                path,
                1,
                format!("fact `{id}`: unknown key `{k}`"),
            ));
        }
        if facts.insert(id.to_owned(), value.to_owned()).is_some() {
            out.push(Diag::error(path, 1, format!("duplicate fact `{id}`")));
        }
        let check = f.get("check").and_then(|v| v.as_str());
        match check {
            None | Some("file" | "manual") => {}
            Some(other) => out.push(Diag::error(
                path,
                1,
                format!("fact `{id}`: `check` must be `file` or `manual`, not `{other}`"),
            )),
        }
        let Some(src) = f.get("source") else {
            if check == Some("file") {
                out.push(Diag::error(
                    path,
                    1,
                    format!("fact `{id}`: `check = \"file\"` needs `source`"),
                ));
            }
            continue;
        };
        let file = src.get("file").and_then(|v| v.as_str());
        let key = src.get("key").and_then(|v| v.as_str());
        let (Some(file), Some(key)) = (file, key) else {
            out.push(Diag::error(
                path,
                1,
                format!("fact `{id}`: `source` needs `file` and `key`"),
            ));
            continue;
        };
        let actual = std::fs::read_to_string(cx.l.root.join(file))
            .ok()
            .and_then(|t| t.parse::<toml::Table>().ok())
            .and_then(|t| lookup(&t, key).map(str::to_owned));
        if actual.as_deref() != Some(value) {
            out.push(Diag::error(
                path,
                1,
                format!("fact `{id}` is `{value}` but {file} `{key}` is {actual:?}"),
            ));
        }
    }
    facts
}

fn lookup<'t>(t: &'t toml::Table, dotted: &str) -> Option<&'t str> {
    let mut parts = dotted.split('.');
    let mut cur = t.get(parts.next()?)?;
    for p in parts {
        cur = cur.as_table()?.get(p)?;
    }
    cur.as_str()
}

/// Reads and checks the ledger: it must parse and be canonical; stamps of sections that no
/// longer need one are warnings. A broken ledger is returned empty.
pub fn check_ledger(l: &Loaded, out: &mut Vec<Diag>) -> Ledger {
    let path = l.config.docs.ledger.as_str();
    let text = match std::fs::read_to_string(l.root.join(path)) {
        Ok(text) => text,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ledger::default(),
        Err(e) => {
            out.push(Diag::error(path, 1, format!("not readable: {e}")));
            return Ledger::default();
        }
    };
    let ledger = match Ledger::parse(&text) {
        Ok(l) => l,
        Err(e) => {
            out.push(Diag::error(path, 1, format!("does not parse: {e}")));
            return Ledger::default();
        }
    };
    if ledger.render() != text {
        out.push(Diag::error(
            path,
            1,
            "not in canonical form: it is written only by `repo docs verify`",
        ));
    }
    let keep = super::stamped_sections(&l.docs);
    for key in ledger
        .sections
        .keys()
        .filter(|k| !keep.contains(k.as_str()))
    {
        out.push(Diag::warning(
            path,
            1,
            format!("stamp `{key}` names no section with `covers` or `depends_on`; the next `repo docs verify` drops it"),
        ));
    }
    ledger
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn normalize_resolves_dots_and_rejects_escape() {
        assert_eq!(
            normalize(Path::new("docs/../README.md")).as_deref(),
            Some("README.md")
        );
        assert_eq!(
            normalize(Path::new("docs/./adr/x.md")).as_deref(),
            Some("docs/adr/x.md")
        );
        assert_eq!(normalize(Path::new("../outside.md")), None);
    }

    #[test]
    fn durations_and_ids() {
        assert_eq!(days("90d"), Some(90));
        assert_eq!(days("0d"), None);
        assert_eq!(days("3w"), None);
        assert!(is_adr_id("ADR-0001"));
        assert!(!is_adr_id("ADR-1"));
    }

    #[test]
    fn packages_and_owners() {
        let paths: Vec<String> = ["Cargo.toml", "a/Cargo.toml", "a/b/Cargo.toml", "a/src/x.rs"]
            .map(str::to_owned)
            .to_vec();
        let pkgs = packages(&paths, "Cargo.toml");
        assert_eq!(pkgs, vec!["a", "a/b"]);
        assert_eq!(package_of("a/src/x.rs", &pkgs), Some("a"));
        assert_eq!(package_of("a/b/src/y.rs", &pkgs), Some("a/b"));
        assert_eq!(package_of("c/z.rs", &pkgs), None);
    }

    #[test]
    fn dotted_lookup() {
        let t: toml::Table = "[toolchain]\nchannel = \"1.85.0\"\n".parse().unwrap();
        assert_eq!(lookup(&t, "toolchain.channel"), Some("1.85.0"));
        assert_eq!(lookup(&t, "toolchain.nope"), None);
    }
}
