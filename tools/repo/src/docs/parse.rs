//! Parses one tracked Markdown doc into its metadata, anchors, sections, rules, generated
//! blocks, links and fact markers. Structure is scanned line by line, as the format is
//! strict; links come from a CommonMark parser, so text in code is never a reference.

use pulldown_cmark::{Event, Parser, Tag};

/// A problem found in a doc, reported as `path:line: message`. Warnings never fail a check.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct Diag {
    pub path: String,
    pub line: usize,
    pub msg: String,
    pub warning: bool,
}

impl Diag {
    pub fn error(path: &str, line: usize, msg: impl Into<String>) -> Self {
        Self {
            path: path.to_owned(),
            line,
            msg: msg.into(),
            warning: false,
        }
    }

    pub fn warning(path: &str, line: usize, msg: impl Into<String>) -> Self {
        Self {
            warning: true,
            ..Self::error(path, line, msg)
        }
    }
}

/// A `##` section with its anchor and `@section` metadata.
#[derive(Debug, Clone)]
pub struct Section {
    pub anchor: String,
    /// 1-based line of the heading.
    pub line: usize,
    pub meta: Option<toml::Table>,
    /// 0-based lines of the `<!-- @section` and closing `-->` lines.
    pub block: Option<(usize, usize)>,
    /// 0-based line range of the body: after the heading or metadata block, up to the next
    /// section's anchor line or the end of the file.
    pub body: (usize, usize),
}

/// A `### R-…` rule with its `@rule` metadata.
#[derive(Debug, Clone)]
pub struct Rule {
    pub id: String,
    /// 1-based line of the heading.
    pub line: usize,
    pub meta: Option<toml::Table>,
    pub block: Option<(usize, usize)>,
    /// 0-based line range of the rule text, up to the next rule's or section's anchor.
    pub body: (usize, usize),
}

/// A `<!-- @generated begin <name> -->` … `<!-- @generated end <name> -->` block.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Generated {
    pub name: String,
    /// 0-based lines of the begin and end markers.
    pub begin: usize,
    pub end: usize,
}

/// A Markdown link destination, outside code.
#[derive(Debug, Clone)]
pub struct Link {
    pub dest: String,
    pub line: usize,
}

/// A `value<!--fact:id-->` marker: the fact id and the text on its line before the marker.
#[derive(Debug, Clone)]
pub struct FactRef {
    pub id: String,
    pub before: String,
    pub line: usize,
}

/// One parsed doc.
#[derive(Debug, Clone)]
pub struct Doc {
    pub path: String,
    pub meta: Option<toml::Table>,
    pub anchors: Vec<(String, usize)>,
    pub sections: Vec<Section>,
    pub rules: Vec<Rule>,
    pub generated: Vec<Generated>,
    pub links: Vec<Link>,
    pub facts: Vec<FactRef>,
    pub bare_urls: Vec<(String, usize)>,
    pub text: String,
    /// 0-based line where content starts (just after the `@doc` block).
    pub content_start: usize,
    /// 0-based lines of the `<!-- @doc` and closing `-->` lines.
    pub doc_block: Option<(usize, usize)>,
}

/// The line that opens a generated block.
pub fn begin_marker(name: &str) -> String {
    format!("<!-- @generated begin {name} -->")
}

/// The line that closes a generated block.
pub fn end_marker(name: &str) -> String {
    format!("<!-- @generated end {name} -->")
}

impl Doc {
    pub fn is_adr(&self) -> bool {
        self.meta_str("kind") == Some("adr")
    }

    /// A string value from the `@doc` block.
    pub fn meta_str(&self, key: &str) -> Option<&str> {
        self.meta.as_ref()?.get(key)?.as_str()
    }

    /// The doc's id, or its path when it has none.
    pub fn id(&self) -> &str {
        self.meta_str("id").unwrap_or(&self.path)
    }

    pub fn status(&self) -> &str {
        self.meta_str("status").unwrap_or_default()
    }

    pub fn has_anchor(&self, anchor: &str) -> bool {
        self.anchors.iter().any(|(a, _)| a == anchor)
    }

    pub fn section(&self, anchor: &str) -> Option<&Section> {
        self.sections.iter().find(|s| s.anchor == anchor)
    }

    pub fn rule(&self, id: &str) -> Option<&Rule> {
        self.rules.iter().find(|r| r.id == id)
    }

    /// One flag per line: whether it belongs to a well-formed `@doc`, `@section` or `@rule`
    /// block (opening line exactly the marker, body parsed as TOML).
    pub fn metadata_lines(&self) -> Vec<bool> {
        let lines: Vec<&str> = self.text.lines().collect();
        let mut mask = vec![false; lines.len()];
        let doc = (self.doc_block, self.meta.is_some(), "<!-- @doc");
        let sections = self
            .sections
            .iter()
            .map(|s| (s.block, s.meta.is_some(), "<!-- @section"));
        let rules = self
            .rules
            .iter()
            .map(|r| (r.block, r.meta.is_some(), "<!-- @rule"));
        for (block, parsed, marker) in std::iter::once(doc).chain(sections).chain(rules) {
            let Some((start, end)) = block else { continue };
            if parsed && lines.get(start).is_some_and(|l| l.trim_end() == marker) {
                for m in mask.iter_mut().take(end + 1).skip(start) {
                    *m = true;
                }
            }
        }
        mask
    }

    /// One flag per line: whether it is inside or on the markers of a generated block.
    pub fn generated_lines(&self) -> Vec<bool> {
        let n = self.text.lines().count();
        let mut mask = vec![false; n];
        for g in &self.generated {
            for m in mask.iter_mut().take(g.end + 1).skip(g.begin) {
                *m = true;
            }
        }
        mask
    }
}

/// Flags, per line, whether the line is outside fenced code blocks.
pub fn outside_fences(lines: &[&str]) -> Vec<bool> {
    let mut fence: Option<&str> = None;
    lines
        .iter()
        .map(|l| {
            let t = l.trim_start();
            if let Some(f) = fence {
                if t.starts_with(f) {
                    fence = None;
                }
                return false;
            }
            if t.starts_with("```") || t.starts_with("~~~") {
                fence = Some(&t[..3]);
                return false;
            }
            true
        })
        .collect()
}

/// Parses `text` (the contents of `path`). Problems found while parsing go to `diags`.
pub fn parse(path: &str, text: &str, diags: &mut Vec<Diag>) -> Doc {
    let mut d = Doc {
        path: path.to_owned(),
        meta: None,
        anchors: Vec::new(),
        sections: Vec::new(),
        rules: Vec::new(),
        generated: Vec::new(),
        links: Vec::new(),
        facts: Vec::new(),
        bare_urls: Vec::new(),
        text: text.to_owned(),
        content_start: 0,
        doc_block: None,
    };
    let mut err = |line: usize, msg: String| diags.push(Diag::error(path, line, msg));

    let lines: Vec<&str> = text.lines().collect();
    // A leading YAML front-matter block (`---` … `---`) may precede the `@doc` block.
    let mut i = 0;
    if lines.first().is_some_and(|l| l.trim_end() == "---") {
        match (1..lines.len()).find(|&k| lines[k].trim_end() == "---") {
            Some(k) => i = k + 1,
            None => err(
                1,
                "unterminated front matter (missing closing `---`)".into(),
            ),
        }
    }
    if lines.get(i).is_some_and(|l| l.starts_with("<!-- @doc"))
        && (i > 0 || text.starts_with("<!-- @doc"))
    {
        match read_block(&lines, i) {
            Some((body, end)) => {
                d.meta = parse_toml(&body, i + 1, &mut err);
                d.doc_block = Some((i, end));
                i = end + 1;
                d.content_start = i;
            }
            None => err(
                i + 1,
                "unterminated `<!-- @doc` block (missing `-->`)".into(),
            ),
        }
    } else {
        err(
            i + 1,
            "file must start with `<!-- @doc`, after YAML front matter if any (no BOM, blank lines or text before it)".into(),
        );
    }
    let is_adr = d.is_adr();

    let mut fence: Option<&str> = None;
    let mut open_gen: Option<(String, usize)> = None;
    while i < lines.len() {
        let line = lines[i];
        let n = i + 1;
        let trimmed = line.trim_start();
        if let Some(f) = fence {
            if trimmed.starts_with(f) {
                fence = None;
            }
            i += 1;
            continue;
        }
        if trimmed.starts_with("```") || trimmed.starts_with("~~~") {
            fence = Some(&trimmed[..3]);
            i += 1;
            continue;
        }
        let bare = line.trim_end();
        if let Some(rest) = bare.strip_prefix("<!-- @generated ") {
            let rest = rest.strip_suffix(" -->").unwrap_or("");
            match (rest.split_once(' '), &open_gen) {
                (Some(("begin", name)), None) => open_gen = Some((name.to_owned(), i)),
                (Some(("end", name)), Some((open, start))) if name == open => {
                    d.generated.push(Generated {
                        name: name.to_owned(),
                        begin: *start,
                        end: i,
                    });
                    open_gen = None;
                }
                (Some(("begin", _)), Some((open, _))) => err(
                    n,
                    format!("`@generated` block `{open}` is not closed before this one"),
                ),
                _ => err(n, format!("malformed `@generated` marker `{bare}`")),
            }
            i += 1;
            continue;
        }
        if let Some(anchor) = anchor_of(line) {
            d.anchors.push((anchor.to_owned(), n));
        }
        if let Some(title) = line.strip_prefix("## ") {
            let anchor = i.checked_sub(1).and_then(|p| anchor_of(lines[p]));
            let Some(anchor) = anchor else {
                err(
                    n,
                    format!(
                        "`## {}` has no `<a id=\"…\"></a>` anchor on the line before it",
                        title.trim()
                    ),
                );
                i += 1;
                continue;
            };
            let mut meta = None;
            let mut block = None;
            let heading = i;
            if lines
                .get(i + 1)
                .is_some_and(|l| l.starts_with("<!-- @section"))
            {
                match read_block(&lines, i + 1) {
                    Some((body, end)) => {
                        meta = parse_toml(&body, n + 1, &mut err);
                        block = Some((i + 1, end));
                        i = end;
                    }
                    None => err(n + 1, "unterminated `<!-- @section` block".into()),
                }
            } else if !is_adr {
                err(
                    n,
                    format!("section `{anchor}` must be followed by a `<!-- @section` block"),
                );
            }
            let body_start = block.map_or(heading + 1, |(_, end)| end + 1);
            d.sections.push(Section {
                anchor: anchor.to_owned(),
                line: n,
                meta,
                block,
                body: (body_start, lines.len()),
            });
        } else if let Some(rest) = line.strip_prefix("### R-") {
            let id_part = rest.split_once(':').map_or(rest, |(i, _)| i);
            let id = format!("R-{}", id_part.trim());
            if i.checked_sub(1).and_then(|p| anchor_of(lines[p])) != Some(id.as_str()) {
                err(
                    n,
                    format!("rule `{id}` must be preceded by `<a id=\"{id}\"></a>`"),
                );
            }
            let mut meta = None;
            let mut block = None;
            let heading = i;
            if lines
                .get(i + 1)
                .is_some_and(|l| l.starts_with("<!-- @rule"))
            {
                match read_block(&lines, i + 1) {
                    Some((body, end)) => {
                        meta = parse_toml(&body, n + 1, &mut err);
                        block = Some((i + 1, end));
                        i = end;
                    }
                    None => err(n + 1, "unterminated `<!-- @rule` block".into()),
                }
            } else {
                err(
                    n,
                    format!("rule `{id}` must be followed by a `<!-- @rule` block"),
                );
            }
            let body_start = block.map_or(heading + 1, |(_, end)| end + 1);
            d.rules.push(Rule {
                id,
                line: n,
                meta,
                block,
                body: (body_start, lines.len()),
            });
        }
        i += 1;
    }
    if fence.is_some() {
        err(lines.len(), "unterminated code fence".into());
    }
    if let Some((name, start)) = open_gen {
        err(
            start + 1,
            format!("`@generated` block `{name}` has no end marker"),
        );
    }

    // A section's body ends where the next section's anchor line begins.
    for k in 1..d.sections.len() {
        let next_anchor = d.sections[k].line.saturating_sub(2);
        let prev = &mut d.sections[k - 1];
        prev.body.1 = next_anchor.max(prev.body.0);
    }
    // A rule's text ends where the next rule's or section's anchor line begins.
    let starts: Vec<usize> = d
        .sections
        .iter()
        .map(|s| s.line)
        .chain(d.rules.iter().map(|r| r.line))
        .map(|line| line.saturating_sub(2))
        .collect();
    for r in &mut d.rules {
        let next = starts.iter().copied().filter(|&a| a >= r.body.0).min();
        r.body.1 = next.unwrap_or(r.body.1).max(r.body.0);
    }

    scan_inline(text, &mut d);
    d
}

/// The id of an `<a id="…"></a>` line, if `line` is one.
pub fn anchor_of(line: &str) -> Option<&str> {
    line.trim()
        .strip_prefix("<a id=\"")?
        .strip_suffix("\"></a>")
}

/// Reads a metadata block starting at `start` (the `<!-- @kind` line): its TOML body and
/// the index of the closing `-->` line.
fn read_block(lines: &[&str], start: usize) -> Option<(String, usize)> {
    let mut body = String::new();
    for (k, l) in lines.iter().enumerate().skip(start + 1) {
        if l.trim() == "-->" {
            return Some((body, k));
        }
        body.push_str(l);
        body.push('\n');
    }
    None
}

fn parse_toml(body: &str, line: usize, err: &mut impl FnMut(usize, String)) -> Option<toml::Table> {
    match body.parse::<toml::Table>() {
        Ok(t) => Some(t),
        Err(e) => {
            err(line, format!("metadata is not valid TOML: {}", e.message()));
            None
        }
    }
}

/// Collects links, fact markers and bare URLs, skipping code.
fn scan_inline(text: &str, d: &mut Doc) {
    let line_of = |off: usize| text[..off].bytes().filter(|&b| b == b'\n').count() + 1;
    for (ev, range) in Parser::new(text).into_offset_iter() {
        match ev {
            Event::Start(Tag::Link { dest_url, .. }) => {
                d.links.push(Link {
                    dest: dest_url.to_string(),
                    line: line_of(range.start),
                });
            }
            Event::InlineHtml(html) | Event::Html(html) => {
                for (k, _) in html.match_indices("<!--fact:") {
                    let rest = &html[k + "<!--fact:".len()..];
                    let Some(end) = rest.find("-->") else {
                        continue;
                    };
                    let at = range.start + k;
                    let line_start = text[..at].rfind('\n').map_or(0, |p| p + 1);
                    d.facts.push(FactRef {
                        id: rest[..end].trim().to_owned(),
                        before: text[line_start..at].to_owned(),
                        line: line_of(at),
                    });
                }
            }
            Event::Text(t) => {
                for word in t.split_whitespace() {
                    if word.starts_with("http://") || word.starts_with("https://") {
                        d.bare_urls.push((word.to_owned(), line_of(range.start)));
                    }
                }
            }
            _ => {}
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn p(text: &str) -> (Doc, Vec<Diag>) {
        let mut diags = Vec::new();
        let d = parse("t.md", text, &mut diags);
        (d, diags)
    }

    pub const HEAD: &str =
        "<!-- @doc\nid = \"T\"\ntitle = \"T\"\nkind = \"normative\"\nstatus = \"active\"\n-->\n";

    #[test]
    fn parses_doc_section_rule_links_and_facts() {
        let text = format!(
            "{HEAD}\n# T\n\n<a id=\"s\"></a>\n## Sec\n<!-- @section\ncovers = [\"a/**\"]\n-->\n\nSee [x](other.md#y) and `[no](code.md)`.\n\n<a id=\"R-X-001\"></a>\n### R-X-001: Thing\n<!-- @rule\nstatus = \"active\"\n-->\nbuild 22000<!--fact:min-build-->\n"
        );
        let (d, diags) = p(&text);
        assert!(diags.is_empty(), "{diags:?}");
        assert_eq!(d.id(), "T");
        assert_eq!(d.sections.len(), 1);
        assert_eq!(d.sections[0].anchor, "s");
        assert!(d.sections[0].meta.as_ref().unwrap().contains_key("covers"));
        assert_eq!(d.rules[0].id, "R-X-001");
        assert_eq!(d.links.len(), 1, "a link inside code is ignored");
        assert_eq!(d.links[0].dest, "other.md#y");
        assert_eq!(d.facts[0].id, "min-build");
        assert!(d.facts[0].before.ends_with("22000"));
    }

    #[test]
    fn requires_doc_block_first() {
        let (_, diags) = p(&format!("\n{HEAD}"));
        assert!(diags.iter().any(|d| d.msg.contains("must start with")));
    }

    #[test]
    fn heading_without_anchor_or_section_block() {
        let (_, diags) = p(&format!(
            "{HEAD}\n## Lonely\n\n<a id=\"x\"></a>\n## Bare\ntext\n"
        ));
        assert!(diags.iter().any(|d| d.msg.contains("no `<a id")));
        assert!(
            diags
                .iter()
                .any(|d| d.msg.contains("`<!-- @section` block"))
        );
    }

    #[test]
    fn adr_sections_need_no_block() {
        let text = "<!-- @doc\nid = \"ADR-0001\"\nkind = \"adr\"\n-->\n<a id=\"c\"></a>\n## Context\nWhy.\n";
        let (d, diags) = p(text);
        assert!(diags.is_empty(), "{diags:?}");
        assert_eq!(d.sections.len(), 1);
    }

    #[test]
    fn headings_inside_fences_are_ignored() {
        let (d, diags) = p(&format!("{HEAD}\n```markdown\n## Not a section\n```\n"));
        assert!(diags.is_empty(), "{diags:?}");
        assert!(d.sections.is_empty());
    }

    #[test]
    fn generated_blocks_pair_up() {
        let text = format!(
            "{HEAD}\n<!-- @generated begin progress -->\nx\n<!-- @generated end progress -->\n"
        );
        let (d, diags) = p(&text);
        assert!(diags.is_empty(), "{diags:?}");
        assert_eq!(
            d.generated,
            vec![Generated {
                name: "progress".into(),
                begin: 7,
                end: 9
            }]
        );
        assert_eq!(
            d.generated_lines(),
            vec![
                false, false, false, false, false, false, false, true, true, true
            ]
        );
        let (_, diags) = p(&format!("{HEAD}<!-- @generated begin a -->\n"));
        assert!(diags.iter().any(|d| d.msg.contains("no end marker")));
        let (_, diags) = p(&format!(
            "{HEAD}<!-- @generated begin a -->\n<!-- @generated end b -->\n"
        ));
        assert!(diags.iter().any(|d| d.msg.contains("malformed")));
    }

    #[test]
    fn front_matter_may_precede_the_doc_block() {
        let text = format!("---\nname: x\ndescription: y\n---\n{HEAD}\nBody.\n");
        let (d, diags) = p(&text);
        assert!(diags.is_empty(), "{diags:?}");
        assert_eq!(d.id(), "T");
        assert_eq!(d.doc_block, Some((4, 9)));
        let (_, diags) = p("---\nname: x\n---\n\n<!-- @doc\nid = \"T\"\n-->\n");
        assert!(diags.iter().any(|d| d.msg.contains("must start with")));
    }

    #[test]
    fn bad_toml_is_reported() {
        let (_, diags) = p("<!-- @doc\nid = \n-->\n");
        assert!(diags.iter().any(|d| d.msg.contains("not valid TOML")));
    }

    #[test]
    fn bare_urls_are_found_outside_code() {
        let (d, _) = p(&format!(
            "{HEAD}\nsee https://example.com now, not `https://code.example`\n"
        ));
        assert_eq!(d.bare_urls.len(), 1);
    }

    #[test]
    fn metadata_mask_marks_only_well_formed_blocks() {
        let text =
            format!("{HEAD}<a id=\"a\"></a>\n## A\n<!-- @section\ncovers = []\n-->\nBody.\n");
        let (d, _) = p(&text);
        let mask = d.metadata_lines();
        assert!(mask[..6].iter().all(|m| *m));
        assert!(!mask[6] && !mask[7]);
        assert!(mask[8] && mask[9] && mask[10]);
        assert!(!mask[11]);
    }
}
