//! `repo heads-changed --state <file> [--refs <glob,…>]`: whether any watched ref on `origin`
//! moved since the last run. Prints `changed` or `unchanged`, writes the watched refs to the
//! state file, and exits 0 either way. The watched refs are `main` and the wave branches
//! unless `--refs` names others. The state file is replaced in one step, through a temporary
//! file beside it and a rename, so a run that dies midway leaves the old state whole.

use std::path::Path;

use anyhow::{Context, Result};

use crate::Outcome;
use crate::args::Args;
use crate::glob::Globs;

/// The refs watched when `--refs` is not given.
pub const DEFAULT_REFS: &[&str] = &["refs/heads/main", "refs/heads/wave/**"];

/// The `git ls-remote` listing, only the refs `watch` matches, sorted, one `<sha> <ref>` per
/// line.
pub fn normalize(listing: &str, watch: &Globs) -> String {
    let mut lines: Vec<String> = listing
        .lines()
        .filter_map(|l| {
            let (sha, r) = l.split_once(char::is_whitespace)?;
            let r = r.trim();
            watch.is_match(r).then(|| format!("{sha} {r}"))
        })
        .collect();
    lines.sort();
    lines.dedup();
    let mut out = lines.join("\n");
    out.push('\n');
    out
}

/// Whether `now` differs from the saved state.
pub fn changed(saved: Option<&str>, now: &str) -> bool {
    saved != Some(now)
}

/// Writes `text` to `path` through a temporary file in the same directory and a rename.
pub fn write_atomic(path: &Path, text: &str) -> Result<()> {
    let dir = path.parent().filter(|d| !d.as_os_str().is_empty());
    if let Some(dir) = dir {
        std::fs::create_dir_all(dir).with_context(|| format!("creating {}", dir.display()))?;
    }
    let name = path
        .file_name()
        .with_context(|| format!("{} names no file", path.display()))?;
    let tmp = path.with_file_name(format!(
        ".{}.tmp-{}",
        name.to_string_lossy(),
        std::process::id()
    ));
    std::fs::write(&tmp, text).with_context(|| format!("writing {}", tmp.display()))?;
    std::fs::rename(&tmp, path).with_context(|| {
        let _ = std::fs::remove_file(&tmp);
        format!("replacing {}", path.display())
    })
}

fn listing() -> Result<String> {
    let dir = crate::git::root()
        .or_else(|_| std::env::current_dir().context("reading the current directory"))?;
    crate::git::text(&dir, &["ls-remote", "origin"])
}

pub fn run(args: &[String]) -> Outcome {
    let usage = "repo heads-changed --state <file> [--refs <glob,…>]";
    let a = Args::parse(args, &["state", "refs"], &[])?;
    a.exactly(0, usage)?;
    let state = a.required("state", usage)?;
    let refs: Vec<String> = match a.value("refs") {
        Some(list) => list
            .split(',')
            .map(str::trim)
            .filter(|g| !g.is_empty())
            .map(str::to_owned)
            .collect(),
        None => DEFAULT_REFS.iter().map(|g| (*g).to_owned()).collect(),
    };
    let watch = Globs::new(&refs)?;
    let now = normalize(&listing()?, &watch);
    let saved = std::fs::read_to_string(state).ok();
    let moved = changed(saved.as_deref(), &now);
    if moved {
        write_atomic(Path::new(state), &now)?;
    }
    println!("{}", if moved { "changed" } else { "unchanged" });
    Ok(true)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn watch() -> Globs {
        Globs::new(DEFAULT_REFS).unwrap()
    }

    #[test]
    fn order_and_spacing_do_not_matter() {
        let a = normalize("b\trefs/heads/main\na\trefs/heads/wave/x\n", &watch());
        let b = normalize("a refs/heads/wave/x\nb refs/heads/main", &watch());
        assert_eq!(a, b);
        assert!(!changed(Some(&a), &b));
        assert!(changed(None, &a));
        assert!(changed(
            Some(&a),
            &normalize("c\trefs/heads/main\n", &watch())
        ));
    }

    #[test]
    fn only_watched_refs_count() {
        let base = "a\trefs/heads/main\nb\trefs/heads/wave/w1\n";
        let noisy = format!(
            "{base}c\tHEAD\nd\trefs/heads/claude/row-1\ne\trefs/pull/7/head\nf\trefs/tags/v1\n"
        );
        assert_eq!(normalize(base, &watch()), normalize(&noisy, &watch()));
        assert_eq!(
            normalize(&noisy, &watch()),
            "a refs/heads/main\nb refs/heads/wave/w1\n"
        );
        let pulls = Globs::new(&["refs/pull/*/head"]).unwrap();
        assert_eq!(normalize(&noisy, &pulls), "e refs/pull/7/head\n");
    }

    #[test]
    fn writes_replace_the_file_whole() {
        let dir = std::env::temp_dir().join(format!("heads-atomic-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let path = dir.join("sub/state.txt");
        write_atomic(&path, "one\n").unwrap();
        write_atomic(&path, "two\n").unwrap();
        assert_eq!(std::fs::read_to_string(&path).unwrap(), "two\n");
        let left: Vec<_> = std::fs::read_dir(dir.join("sub"))
            .unwrap()
            .map(|e| e.unwrap().file_name())
            .collect();
        assert_eq!(left.len(), 1, "{left:?}");
        let _ = std::fs::remove_dir_all(&dir);
    }
}
