//! Hash-bound verification. A section has one hash per input, each stamped separately in
//! the ledger:
//! 1. `body`: its own text, normalized (metadata blocks and generated blocks removed, LF
//!    endings, trailing whitespace stripped, outer blank lines dropped);
//! 2. `file:<path>`: every tracked file its `covers` globs match, by content;
//! 3. `dep:<ref>`: every `depends_on` target's normalized body (a section, a rule, or a
//!    whole doc).
//!
//! Wherever Markdown is hashed, well-formed metadata blocks and generated blocks are left
//! out, so recording a verification or regenerating a block never stales a section. In
//! covered workflow files, the ref after `@` in `uses:` lines is ignored, so action pin
//! bumps don't stale the section that describes the workflow.

use std::collections::BTreeMap;
use std::path::Path;

use anyhow::{Context, Result};

use super::Index;
use super::parse::Doc;

/// Prefix of every hash.
pub const PREFIX: &str = "b3:";

/// The ledger key of a section's own text.
pub const BODY: &str = "body";

/// A short hash of `bytes`: the prefix and 16 hex digits of blake3.
pub fn short(bytes: &[u8]) -> String {
    format!("{PREFIX}{}", &blake3::hash(bytes).to_hex().as_str()[..16])
}

/// Trailing whitespace stripped, outer blank lines dropped.
fn normalize(lines: &[&str]) -> String {
    let trimmed: Vec<&str> = lines.iter().map(|l| l.trim_end()).collect();
    let start = trimmed
        .iter()
        .position(|l| !l.is_empty())
        .unwrap_or(trimmed.len());
    let end = trimmed
        .iter()
        .rposition(|l| !l.is_empty())
        .map_or(start, |e| e + 1);
    trimmed[start..end].join("\n")
}

/// The normalized text of a section or rule (by anchor), or with `anchor = None` of the
/// whole doc after its `@doc` block. `None` when the anchor names neither.
pub fn body(doc: &Doc, anchor: Option<&str>) -> Option<String> {
    let lines: Vec<&str> = doc.text.lines().collect();
    let (start, end) = match anchor {
        None => (doc.content_start, lines.len()),
        Some(a) => doc
            .section(a)
            .map(|s| s.body)
            .or_else(|| doc.rule(a).map(|r| r.body))?,
    };
    let (start, end) = (start.min(lines.len()), end.min(lines.len()));
    let meta = doc.metadata_lines();
    let generated = doc.generated_lines();
    let kept: Vec<&str> = (start..end)
        .filter(|&k| !meta[k] && !generated[k])
        .map(|k| lines[k])
        .collect();
    Some(normalize(&kept))
}

/// Markdown content without its metadata and generated blocks. Kept lines are
/// byte-identical.
fn strip_markdown(path: &str, text: &str) -> String {
    let doc = super::parse::parse(path, text, &mut Vec::new());
    let meta = doc.metadata_lines();
    let generated = doc.generated_lines();
    text.split_inclusive('\n')
        .enumerate()
        .filter(|(k, _)| {
            !meta.get(*k).copied().unwrap_or(false) && !generated.get(*k).copied().unwrap_or(false)
        })
        .map(|(_, l)| l)
        .collect()
}

/// File content as hashed: CRLF folded to LF; Markdown metadata and generated blocks
/// removed; workflow `uses:` refs dropped.
pub fn file_content(path: &str, bytes: &[u8]) -> Vec<u8> {
    let mut text = String::from_utf8_lossy(bytes).replace("\r\n", "\n");
    if path.ends_with(".md") {
        text = strip_markdown(path, &text);
    }
    if !path.starts_with(".github/workflows/") {
        return text.into_bytes();
    }
    text.lines()
        .map(|l| {
            match l
                .find("uses:")
                .and_then(|u| l[u..].find('@').map(|at| u + at))
            {
                Some(at) => &l[..at],
                None => l,
            }
        })
        .collect::<Vec<_>>()
        .join("\n")
        .into_bytes()
}

/// Repository files, for glob matching and hashing.
pub struct Files<'a> {
    root: &'a Path,
    paths: &'a [String],
    /// The ledger, which no section can cover.
    ledger: &'a str,
}

impl<'a> Files<'a> {
    pub fn new(root: &'a Path, paths: &'a [String], ledger: &'a str) -> Self {
        Self {
            root,
            paths,
            ledger,
        }
    }

    pub fn paths(&self) -> &'a [String] {
        self.paths
    }

    /// Files matched by any of `globs`, sorted.
    pub fn matching(&self, globs: &[String]) -> Result<Vec<&'a str>> {
        let set = crate::glob::Globs::new(globs)?;
        Ok(self
            .paths
            .iter()
            .filter(|p| p.as_str() != self.ledger && set.is_match(p))
            .map(String::as_str)
            .collect())
    }

    pub fn content_hash(&self, path: &str) -> Result<String> {
        let bytes =
            std::fs::read(self.root.join(path)).with_context(|| format!("reading {path}"))?;
        Ok(short(&file_content(path, &bytes)))
    }
}

/// The text a `depends_on` entry names: a section's or rule's body, or a whole doc's.
/// `None` when it doesn't resolve.
pub fn dep_text(dep: &str, index: &Index<'_>) -> Option<String> {
    let (doc_ref, anchor) = match dep.split_once('#') {
        Some((d, a)) => (d, Some(a)),
        None => (dep, None),
    };
    body(index.get(doc_ref)?, anchor)
}

/// A section's inputs with their hashes, keyed as in the ledger. The section is fresh when
/// its stamp records exactly these.
pub fn inputs(
    doc: &Doc,
    anchor: &str,
    covers: &[String],
    depends_on: &[String],
    files: &Files<'_>,
    index: &Index<'_>,
) -> Result<BTreeMap<String, String>> {
    let mut out = BTreeMap::new();
    let text = body(doc, Some(anchor)).unwrap_or_default();
    out.insert(BODY.to_owned(), short(text.as_bytes()));
    for path in files.matching(covers)? {
        out.insert(format!("file:{path}"), files.content_hash(path)?);
    }
    for dep in depends_on {
        let text = dep_text(dep, index).unwrap_or_default();
        out.insert(format!("dep:{dep}"), short(text.as_bytes()));
    }
    Ok(out)
}

/// A string as a TOML basic string, quotes included.
pub fn toml_str(s: &str) -> String {
    toml::Value::String(s.to_owned()).to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn parse(text: &str) -> Doc {
        super::super::parse::parse("t.md", text, &mut Vec::new())
    }

    #[test]
    fn normalize_trims_and_strips() {
        assert_eq!(normalize(&["", "  a  ", "b\t", "", ""]), "  a\nb");
        assert_eq!(normalize(&["", ""]), "");
    }

    #[test]
    fn workflow_refs_are_ignored_other_files_are_not() {
        let a = file_content(
            ".github/workflows/x.yml",
            b"- uses: actions/checkout@aaaa # v7\nrun: x\r\n",
        );
        let b = file_content(
            ".github/workflows/x.yml",
            b"- uses: actions/checkout@bbbb # v8\nrun: x\n",
        );
        assert_eq!(a, b);
        assert_ne!(
            file_content("src/x.rs", b"uses: a@1"),
            file_content("src/x.rs", b"uses: a@2")
        );
    }

    #[test]
    fn section_body_excludes_blocks_and_next_anchor() {
        let text = "<!-- @doc\nid = \"T\"\n-->\n<a id=\"a\"></a>\n## A\n<!-- @section\ncovers = []\n-->\n\nAlpha text.  \n<!-- @generated begin progress -->\nrow\n<!-- @generated end progress -->\n\n<a id=\"b\"></a>\n## B\n<!-- @section\n-->\nBeta.\n";
        let doc = parse(text);
        assert_eq!(body(&doc, Some("a")).as_deref(), Some("Alpha text."));
        assert_eq!(body(&doc, Some("b")).as_deref(), Some("Beta."));
        assert!(body(&doc, Some("nope")).is_none());
    }

    #[test]
    fn rule_body_runs_to_the_next_anchor() {
        let text = "<!-- @doc\nid = \"T\"\n-->\n<a id=\"a\"></a>\n## A\n<!-- @section\n-->\nIntro.\n\n<a id=\"R-X-001\"></a>\n### R-X-001: One\n<!-- @rule\nstatus = \"active\"\n-->\nFirst.\n\n<a id=\"R-X-002\"></a>\n### R-X-002: Two\n<!-- @rule\nstatus = \"active\"\n-->\nSecond.\n";
        let doc = parse(text);
        assert_eq!(body(&doc, Some("R-X-001")).as_deref(), Some("First."));
        assert_eq!(body(&doc, Some("R-X-002")).as_deref(), Some("Second."));
        assert!(
            body(&doc, Some("a")).is_some_and(|b| b.starts_with("Intro.") && b.contains("Second."))
        );
    }

    const MD: &str = "<!-- @doc\nid = \"T\"\nreviewed = { by = \"human:x\", note = \"one\" }\n-->\n<a id=\"a\"></a>\n## A\n<!-- @section\ncovers = [\"src/**\"]\n-->\nAlpha.\n<!-- @generated begin adr-index -->\n| x |\n<!-- @generated end adr-index -->\n";

    #[test]
    fn markdown_metadata_and_generated_blocks_are_not_hashed() {
        let hash = |t: &str| file_content("d/T.md", t.as_bytes());
        let base = hash(MD);
        assert_eq!(
            String::from_utf8(base.clone()).unwrap(),
            "<a id=\"a\"></a>\n## A\nAlpha.\n"
        );
        for edit in [
            MD.replace("note = \"one\"", "note = \"two\""),
            MD.replace("src/**", "lib/**"),
            MD.replace("| x |", "| y |"),
        ] {
            assert_eq!(hash(&edit), base, "{edit}");
        }
        assert_ne!(hash(&MD.replace("Alpha.", "Alpha!")), base);
    }

    #[test]
    fn malformed_metadata_still_counts() {
        let hash = |t: &str| file_content("d/T.md", t.as_bytes());
        for fake in [
            "\nProse.\n<!-- @section\nhidden = \"X\"\n-->\n",
            "\n<a id=\"b\"></a>\n## B\n<!-- @section hidden X\n-->\n",
            "\n```\n<a id=\"c\"></a>\n## C\n<!-- @section\nhidden = \"X\"\n-->\n```\n",
            "\n<a id=\"d\"></a>\n## D\n<!-- @section\nhidden X\n-->\n",
        ] {
            let a = hash(&format!("{MD}{fake}"));
            let b = hash(&format!("{MD}{}", fake.replace('X', "Y")));
            assert_ne!(a, b, "hidden text escaped the hash in {fake:?}");
        }
    }

    #[test]
    fn short_hash_shape_and_toml_strings() {
        let h = short(b"x");
        assert!(h.starts_with("b3:") && h.len() == 19, "{h}");
        for v in [r#"say "hi""#, "back\\slash", "plain"] {
            let t: toml::Table = format!("k = {}", toml_str(v)).parse().unwrap();
            assert_eq!(t["k"].as_str(), Some(v));
        }
    }
}
