//! Path globs, as every config and metadata field uses them: `*` and `?` stay inside one
//! path segment, `**` crosses segments, `{a,b}` and `[...]` work as usual.

use anyhow::{Context, Result};
use globset::{GlobBuilder, GlobSet, GlobSetBuilder};

/// Compiles one glob.
pub fn compile(glob: &str) -> Result<globset::Glob> {
    GlobBuilder::new(glob)
        .literal_separator(true)
        .backslash_escape(true)
        .build()
        .with_context(|| format!("invalid glob `{glob}`"))
}

/// Compiles several globs into one matcher.
pub fn set<S: AsRef<str>>(globs: &[S]) -> Result<GlobSet> {
    let mut b = GlobSetBuilder::new();
    for g in globs {
        b.add(compile(g.as_ref())?);
    }
    Ok(b.build()?)
}

/// A compiled list of globs; an empty list matches nothing.
#[derive(Debug, Clone)]
pub struct Globs {
    set: GlobSet,
    empty: bool,
}

impl Globs {
    pub fn new<S: AsRef<str>>(globs: &[S]) -> Result<Self> {
        Ok(Self {
            set: set(globs)?,
            empty: globs.is_empty(),
        })
    }

    pub fn is_match(&self, path: &str) -> bool {
        !self.empty && self.set.is_match(path)
    }
}

/// The literal directory a glob starts with: everything before its first glob character,
/// cut back to the last `/`.
fn literal_dir(glob: &str) -> &str {
    let Some(first) = glob.find(['*', '?', '[', '{', '\\']) else {
        return glob;
    };
    glob[..first].rfind('/').map_or("", |k| &glob[..k])
}

/// Whether `dir` is `other` or one of its ancestors (`""` is every path's ancestor).
fn is_ancestor(dir: &str, other: &str) -> bool {
    dir.is_empty() || other == dir || other.starts_with(&format!("{dir}/"))
}

/// Whether two globs can match the same path: one matches the other's text, a known path
/// matches both, or one is a whole directory tree (`dir/**`) that contains where the
/// other starts.
pub fn overlap(a: &str, b: &str, known: &[String]) -> Result<bool> {
    let (ga, gb) = (compile(a)?.compile_matcher(), compile(b)?.compile_matcher());
    if a == b || ga.is_match(b) || gb.is_match(a) {
        return Ok(true);
    }
    if known.iter().any(|p| ga.is_match(p) && gb.is_match(p)) {
        return Ok(true);
    }
    let tree = |g: &str| g == "**" || g.ends_with("/**");
    Ok((tree(a) && is_ancestor(literal_dir(a), literal_dir(b)))
        || (tree(b) && is_ancestor(literal_dir(b), literal_dir(a))))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn star_stays_in_one_segment() {
        let g = Globs::new(&["*.md", "docs/**/*.md"]).unwrap();
        assert!(g.is_match("README.md"));
        assert!(!g.is_match("tools/x/README.md"));
        assert!(g.is_match("docs/a.md"));
        assert!(g.is_match("docs/adr/0001-x.md"));
        assert!(!Globs::new::<&str>(&[]).unwrap().is_match("a"));
        assert!(compile("a/[").is_err());
    }

    #[test]
    fn literal_dirs() {
        assert_eq!(literal_dir("src/a/**"), "src/a");
        assert_eq!(literal_dir("src/a/*.rs"), "src/a");
        assert_eq!(literal_dir("src/a.rs"), "src/a.rs");
        assert_eq!(literal_dir("**"), "");
        assert_eq!(literal_dir("*.md"), "");
    }

    #[test]
    fn overlapping_globs() {
        let none: Vec<String> = Vec::new();
        assert!(overlap("src/**", "src/a/b.rs", &none).unwrap());
        assert!(overlap("src/a/**", "src/**", &none).unwrap());
        assert!(overlap("src/**", "src/a/*.rs", &none).unwrap());
        assert!(overlap("docs/x.md", "docs/x.md", &none).unwrap());
        assert!(!overlap("src/a/**", "src/b/**", &none).unwrap());
        assert!(!overlap("src/*.rs", "src/*.md", &none).unwrap());
        assert!(!overlap("src/a.rs", "src/b.rs", &none).unwrap());
        let known = vec!["src/x/mod.rs".to_owned()];
        assert!(overlap("src/*/mod.rs", "src/x/*.rs", &known).unwrap());
        assert!(!overlap("src/*/mod.rs", "src/x/*.rs", &none).unwrap());
    }
}
