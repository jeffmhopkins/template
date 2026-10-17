//! Code back-references. Every package root file (the stack pack's `[backrefs].roots`)
//! whose package some section covers carries one line starting with `[backrefs].prefix`,
//! listing `DOC#anchor` references: exactly the sections whose `covers` match files of its
//! package, plus any bare `DOC` references (ADRs). Every reference must resolve to a live
//! doc.

use std::collections::BTreeSet;

use super::check::{package_of, packages};
use super::parse::{Diag, Doc};
use super::{Index, Loaded, strings};

/// The references on the back-reference line of `text`, with its 1-based line number.
pub fn refs_line(text: &str, prefix: &str) -> Option<(usize, Vec<String>)> {
    text.lines().enumerate().find_map(|(i, l)| {
        let rest = l.trim_start().strip_prefix(prefix)?;
        let refs = rest
            .split(',')
            .map(|r| r.trim().trim_end_matches('.'))
            .filter(|r| !r.is_empty())
            .map(str::to_owned)
            .collect();
        Some((i + 1, refs))
    })
}

/// Checks one reference: `DOC#anchor` must name a section or rule of a live doc, a bare
/// `DOC` a live doc.
fn resolve(r: &str, index: &Index<'_>) -> Result<(), String> {
    let (doc_ref, anchor) = r.split_once('#').map_or((r, None), |(f, a)| (f, Some(a)));
    let Some(doc) = index.get(doc_ref) else {
        return Err(format!(
            "back-reference `{r}`: `{doc_ref}` is not a tracked doc"
        ));
    };
    if let Some(a) = anchor {
        if !doc.has_anchor(a) {
            return Err(format!(
                "back-reference `{r}`: no anchor `{a}` in {}",
                doc.path
            ));
        }
    }
    match doc.status() {
        s @ ("superseded" | "retired" | "rejected") => {
            Err(format!("back-reference `{r}` cites a {s} doc"))
        }
        _ => Ok(()),
    }
}

/// The section key (`ID#anchor`) a reference names, whatever form its doc part takes.
fn canonical(r: &str, index: &Index<'_>) -> Option<String> {
    let (doc_ref, anchor) = r.split_once('#')?;
    Some(format!("{}#{anchor}", index.get(doc_ref)?.id()))
}

/// Runs the back-reference checks.
pub fn check(l: &Loaded, index: &Index<'_>, files: &super::hash::Files<'_>) -> Vec<Diag> {
    let mut out = Vec::new();
    let prefix = l.pack.backrefs.prefix.as_str();
    let roots = match crate::glob::Globs::new(&l.pack.backrefs.roots) {
        Ok(g) => g,
        Err(e) => return vec![Diag::error(&l.config.pack_path(), 1, format!("{e:#}"))],
    };
    let mut pkgs = packages(&l.files, &l.pack.detect);
    if l.files.contains(&l.pack.detect) {
        pkgs.push("");
    }
    // Each live section with `covers`: its key and the packages its files lie in.
    let mut covered: Vec<(String, BTreeSet<&str>)> = Vec::new();
    for doc in l
        .docs
        .iter()
        .filter(|d| !matches!(d.status(), "retired" | "superseded"))
    {
        for s in &doc.sections {
            let Some(meta) = &s.meta else { continue };
            let globs = strings(meta, "covers");
            if globs.is_empty() {
                continue;
            }
            if let Ok(m) = files.matching(&globs) {
                let owners = m.iter().filter_map(|f| package_of(f, &pkgs)).collect();
                covered.push((format!("{}#{}", doc.id(), s.anchor), owners));
            }
        }
    }
    let is_section = |key: &str| {
        key.split_once('#')
            .and_then(|(d, a)| index.get(d).map(|doc: &Doc| doc.section(a).is_some()))
            .unwrap_or(false)
    };
    for root in l.files.iter().filter(|p| roots.is_match(p)) {
        let Ok(text) = std::fs::read_to_string(l.root.join(root)) else {
            continue;
        };
        let pkg = package_of(root, &pkgs);
        let expected: BTreeSet<&str> = covered
            .iter()
            .filter(|(_, owners)| pkg.is_some_and(|p| owners.contains(p)))
            .map(|(k, _)| k.as_str())
            .collect();
        let Some((line, listed)) = refs_line(&text, prefix) else {
            if !expected.is_empty() {
                out.push(Diag::error(
                    root,
                    1,
                    format!(
                        "package root has no `{prefix}` line; sections covering this package: {}",
                        expected.iter().copied().collect::<Vec<_>>().join(", ")
                    ),
                ));
            }
            continue;
        };
        let mut seen = BTreeSet::new();
        for r in &listed {
            match resolve(r, index) {
                Err(msg) => out.push(Diag::error(root, line, msg)),
                Ok(()) => {
                    if let Some(k) = canonical(r, index) {
                        seen.insert(k);
                    }
                }
            }
        }
        for missing in expected.iter().filter(|e| !seen.contains(**e)) {
            out.push(Diag::error(
                root,
                line,
                format!("`{prefix}` is missing `{missing}`, whose `covers` match this package"),
            ));
        }
        // A listed section with `covers` must cover this package.
        for extra in seen
            .iter()
            .filter(|k| is_section(k) && !expected.contains(k.as_str()))
        {
            out.push(Diag::error(
                root,
                line,
                format!("`{prefix}` lists `{extra}`, which does not cover this package"),
            ));
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_the_first_prefixed_line() {
        let t = "//! Crate.\n//!\n//! Docs: A#x, B#y\n\nfn f() {}\n";
        assert_eq!(
            refs_line(t, "//! Docs:"),
            Some((3, vec!["A#x".to_owned(), "B#y".to_owned()]))
        );
        assert_eq!(refs_line("fn f() {}\n", "//! Docs:"), None);
        assert_eq!(
            refs_line("# Docs: A#x.\n", "# Docs:"),
            Some((1, vec!["A#x".to_owned()]))
        );
    }
}
