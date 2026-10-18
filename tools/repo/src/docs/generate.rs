//! Generated blocks: `adr-status` (the banner under each ADR's title), `adr-index` (the
//! table of every ADR) and `progress` (the roadmap table derived by `repo progress`).
//! `docs generate` writes them; `docs check` regenerates the ADR blocks in memory and
//! reports drift. The `progress` block depends on history that changes with every merge,
//! so `docs check` does not compare it.

use std::fmt::Write as _;
use std::path::{Component, Path};

use super::parse::{Diag, Doc, begin_marker, end_marker, outside_fences};
use super::{Index, Loaded};

pub const STATUS: &str = "adr-status";
pub const INDEX: &str = "adr-index";
pub const PROGRESS: &str = "progress";
/// Every block name a generator owns.
pub const NAMES: &[&str] = &[STATUS, INDEX, PROGRESS];

/// `to` as a link relative to the directory of `from` (both repo paths).
pub fn relative(from: &str, to: &str) -> String {
    let dir: Vec<Component<'_>> = Path::new(from)
        .parent()
        .map(|p| p.components().collect())
        .unwrap_or_default();
    let target: Vec<Component<'_>> = Path::new(to).components().collect();
    let common = dir.iter().zip(&target).take_while(|(a, b)| a == b).count();
    let mut parts: Vec<String> = vec!["..".to_owned(); dir.len() - common];
    parts.extend(
        target[common..]
            .iter()
            .map(|c| c.as_os_str().to_string_lossy().into_owned()),
    );
    parts.join("/")
}

/// Text safe inside a Markdown link label or table cell.
pub fn escape(text: &str) -> String {
    text.replace('\\', "\\\\")
        .replace('[', "\\[")
        .replace(']', "\\]")
        .replace('|', "\\|")
}

/// Joins lines, keeping `like`'s final newline.
fn join(lines: &[&str], like: &str) -> String {
    let mut out = lines.join("\n");
    if like.ends_with('\n') {
        out.push('\n');
    }
    out
}

/// `text` without its `name` blocks (markers included) outside code fences, and without one
/// of the two blank lines that then meet.
pub fn strip_block(text: &str, name: &str) -> String {
    let (b, e) = (begin_marker(name), end_marker(name));
    let mut lines: Vec<&str> = text.lines().collect();
    let mut i = 0;
    loop {
        let open = outside_fences(&lines);
        let Some(start) = (i..lines.len()).find(|&k| open[k] && lines[k].trim_end() == b) else {
            break;
        };
        let Some(stop) = (start + 1..lines.len()).find(|&k| lines[k].trim_end() == e) else {
            break;
        };
        lines.drain(start..=stop);
        if start > 0
            && start < lines.len()
            && lines[start].trim().is_empty()
            && lines[start - 1].trim().is_empty()
        {
            lines.remove(start);
        }
        i = start;
    }
    join(&lines, text)
}

/// `doc`'s text with the inside of its `name` block replaced by `body`, and the 1-based
/// line of the begin marker.
pub fn with_block(doc: &Doc, name: &str, body: &str) -> Result<(String, usize), String> {
    let g = doc
        .generated
        .iter()
        .find(|g| g.name == name)
        .ok_or_else(|| format!("no `{}` line", begin_marker(name)))?;
    let lines: Vec<&str> = doc.text.lines().collect();
    let mut out: Vec<&str> = lines[..=g.begin].to_vec();
    out.extend(body.lines());
    out.extend(&lines[g.end..]);
    Ok((join(&out, &doc.text), g.begin + 1))
}

/// The banner line of an ADR.
fn status_line(doc: &Doc, index: &Index<'_>) -> Result<String, String> {
    match doc.meta_str("status") {
        Some("superseded") => {
            let next_id = doc
                .meta_str("superseded_by")
                .ok_or("a superseded ADR needs `superseded_by`")?;
            let next = index
                .get(next_id)
                .filter(|d| d.is_adr())
                .ok_or_else(|| format!("`superseded_by` `{next_id}` is not a tracked ADR"))?;
            Ok(format!(
                "> **Status:** superseded by [{}]({})",
                escape(next.id()),
                relative(&doc.path, &next.path)
            ))
        }
        Some(status) => Ok(format!("> **Status:** {status}")),
        None => Err("the `@doc` block has no `status`".into()),
    }
}

/// An ADR's text with its banner regenerated: any `adr-status` block removed, then one
/// written under the `# ` title line, between single blank lines.
fn with_status(doc: &Doc, line: &str) -> Result<String, String> {
    let stripped = strip_block(&doc.text, STATUS);
    let mut lines: Vec<&str> = stripped.lines().collect();
    let open = outside_fences(&lines);
    let title = (doc.content_start..lines.len())
        .find(|&k| open[k] && lines[k].starts_with("# "))
        .ok_or("no `# ` title line to put the status banner under")?;
    let rest = (title + 1..lines.len())
        .find(|&k| !lines[k].trim().is_empty())
        .unwrap_or(lines.len());
    let (b, e) = (begin_marker(STATUS), end_marker(STATUS));
    let tail: Vec<&str> = lines.split_off(rest);
    lines.truncate(title + 1);
    lines.extend(["", &b, line, &e]);
    if !tail.is_empty() {
        lines.push("");
        lines.extend(tail);
    }
    Ok(join(&lines, &doc.text))
}

/// The inside of the `adr-index` block in `at`.
fn index_body(at: &str, adrs: &[&Doc]) -> String {
    let mut out = String::from("| ADR | Title | Tier | Status |\n|---|---|---|---|\n");
    for d in adrs {
        let status = match (d.status(), d.meta_str("superseded_by")) {
            ("superseded", Some(next)) => format!("superseded by {next}"),
            (s, _) => s.to_owned(),
        };
        let _ = writeln!(
            out,
            "| [{}]({}) | {} | {} | {} |",
            escape(d.id()),
            relative(at, &d.path),
            escape(d.meta_str("title").unwrap_or_default()),
            d.meta_str("tier").unwrap_or("—"),
            status
        );
    }
    out
}

/// What the generators would write.
#[derive(Debug, Default)]
pub struct Generated {
    /// Files whose generated text differs from what is on disk: path, 1-based line, text.
    pub files: Vec<(String, usize, String)>,
    pub problems: Vec<Diag>,
}

impl Generated {
    fn update(&mut self, path: &str, line: usize, text: String) {
        match self.files.iter_mut().find(|(p, _, _)| p == path) {
            Some(f) => f.2 = text,
            None => self.files.push((path.to_owned(), line, text)),
        }
    }

    /// The current text of `doc`, with updates made so far.
    fn text_of<'a>(&'a self, doc: &'a Doc) -> &'a str {
        self.files
            .iter()
            .find(|(p, _, _)| *p == doc.path)
            .map_or(doc.text.as_str(), |(_, _, t)| t.as_str())
    }
}

/// Regenerates the blocks of every doc. `progress` is the inside of the progress block,
/// when it is to be written.
pub fn generate(l: &Loaded, progress: Option<&str>) -> Generated {
    let index = Index::new(&l.docs);
    let mut g = Generated::default();
    let mut adrs: Vec<&Doc> = l.docs.iter().filter(|d| d.is_adr()).collect();
    adrs.sort_by(|a, b| (a.id(), &a.path).cmp(&(b.id(), &b.path)));
    for d in &adrs {
        match status_line(d, &index).and_then(|line| with_status(d, &line)) {
            Ok(text) if text == d.text => {}
            Ok(text) => {
                let line = d
                    .text
                    .lines()
                    .position(|l| l.starts_with("# "))
                    .map_or(1, |k| k + 1);
                g.update(&d.path, line, text);
            }
            Err(msg) => g.problems.push(Diag::error(
                &d.path,
                1,
                format!("cannot generate the `{STATUS}` banner: {msg}"),
            )),
        }
    }
    let holders: Vec<&Doc> = l
        .docs
        .iter()
        .filter(|d| d.generated.iter().any(|b| b.name == INDEX))
        .collect();
    if holders.len() > 1 {
        for d in &holders[1..] {
            g.problems.push(Diag::error(
                &d.path,
                1,
                format!(
                    "only one doc may hold the `{INDEX}` block (also in {})",
                    holders[0].path
                ),
            ));
        }
    }
    if holders.is_empty() && !adrs.is_empty() {
        g.problems.push(Diag::error(
            &adrs[0].path,
            1,
            format!("ADRs exist but no doc holds an `{INDEX}` block"),
        ));
    }
    for d in holders.iter().take(1) {
        let body = index_body(&d.path, &adrs);
        regenerate(&mut g, d, INDEX, &body);
    }
    if let Some(body) = progress {
        for d in l
            .docs
            .iter()
            .filter(|d| d.generated.iter().any(|b| b.name == PROGRESS))
        {
            regenerate(&mut g, d, PROGRESS, body);
        }
    }
    g
}

/// Rewrites the inside of `name` in `doc`, on top of the updates already in `g`.
fn regenerate(g: &mut Generated, doc: &Doc, name: &str, body: &str) {
    let current = super::parse::parse(&doc.path, g.text_of(doc), &mut Vec::new());
    match with_block(&current, name, body) {
        Ok((text, _)) if text == current.text => {}
        Ok((text, line)) => g.update(&doc.path, line, text),
        Err(msg) => g.problems.push(Diag::error(&doc.path, 1, msg)),
    }
}

/// Drift findings for `docs check`: ADR banners and the ADR index that differ from what is
/// on disk.
pub fn check(l: &Loaded) -> Vec<Diag> {
    let g = generate(l, None);
    let mut out = g.problems;
    for (path, line, _) in g.files {
        out.push(Diag::error(
            &path,
            line,
            "generated content is out of date: run `scripts/repo docs generate`",
        ));
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn relative_links() {
        assert_eq!(relative("docs/adr/a.md", "docs/adr/b.md"), "b.md");
        assert_eq!(relative("docs/adr/README.md", "docs/DOCS.md"), "../DOCS.md");
        assert_eq!(relative("README.md", "docs/adr/x.md"), "docs/adr/x.md");
    }

    #[test]
    fn strip_block_removes_markers_and_one_blank() {
        let text = "# T\n\n<!-- @generated begin adr-status -->\n> x\n<!-- @generated end adr-status -->\n\nBody.\n";
        assert_eq!(strip_block(text, STATUS), "# T\n\nBody.\n");
        let fenced =
            "```\n<!-- @generated begin adr-status -->\n<!-- @generated end adr-status -->\n```\n";
        assert_eq!(strip_block(fenced, STATUS), fenced);
    }

    #[test]
    fn status_banner_goes_under_the_title() {
        let text = "<!-- @doc\nid = \"ADR-0001\"\nkind = \"adr\"\nstatus = \"accepted\"\n-->\n\n# ADR-0001: T\n\nBody.\n";
        let doc = super::super::parse::parse("docs/adr/0001-t.md", text, &mut Vec::new());
        let out = with_status(&doc, "> **Status:** accepted").unwrap();
        assert!(out.ends_with("# ADR-0001: T\n\n<!-- @generated begin adr-status -->\n> **Status:** accepted\n<!-- @generated end adr-status -->\n\nBody.\n"), "{out}");
        let again = super::super::parse::parse("docs/adr/0001-t.md", &out, &mut Vec::new());
        assert_eq!(with_status(&again, "> **Status:** accepted").unwrap(), out);
    }
}
