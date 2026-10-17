//! Git as a subprocess: the only way the tool reads history and other revisions.

use std::path::{Path, PathBuf};
use std::process::{Command, Output};

use anyhow::{Context, Result, bail};

/// Runs `git <args>` in `dir` and returns its raw output, whatever the exit status.
pub fn raw(dir: &Path, args: &[&str]) -> Result<Output> {
    Command::new("git")
        .args(args)
        .current_dir(dir)
        .output()
        .with_context(|| format!("running `git {}`", args.join(" ")))
}

/// Runs `git <args>` in `dir`; fails with its stderr unless it succeeds.
pub fn run(dir: &Path, args: &[&str]) -> Result<Vec<u8>> {
    let out = raw(dir, args)?;
    if !out.status.success() {
        bail!(
            "`git {}` failed: {}",
            args.join(" "),
            String::from_utf8_lossy(&out.stderr).trim()
        );
    }
    Ok(out.stdout)
}

/// [`run`], as text.
pub fn text(dir: &Path, args: &[&str]) -> Result<String> {
    Ok(String::from_utf8_lossy(&run(dir, args)?).into_owned())
}

/// Whether `git <args>` exits 0.
pub fn succeeds(dir: &Path, args: &[&str]) -> bool {
    raw(dir, args).is_ok_and(|o| o.status.success())
}

/// The full commit id `rev` names.
pub fn resolve(dir: &Path, rev: &str) -> Result<String> {
    let spec = format!("{rev}^{{commit}}");
    Ok(text(dir, &["rev-parse", "--verify", "-q", &spec])
        .with_context(|| format!("`{rev}` does not name a commit"))?
        .trim()
        .to_owned())
}

/// The merge base of two revisions.
pub fn merge_base(dir: &Path, a: &str, b: &str) -> Result<String> {
    Ok(text(dir, &["merge-base", a, b])
        .with_context(|| {
            format!("finding the merge base of `{a}` and `{b}` (full history needed)")
        })?
        .trim()
        .to_owned())
}

/// The contents of `path` at `rev`, read from the object store; `None` when absent there.
pub fn show(dir: &Path, rev: &str, path: &str) -> Result<Option<String>> {
    let out = raw(dir, &["show", &format!("{rev}:{path}")])?;
    Ok(out
        .status
        .success()
        .then(|| String::from_utf8_lossy(&out.stdout).replace("\r\n", "\n")))
}

/// `git ls-files -z <extra>`, deduplicated and sorted.
pub fn ls_files(dir: &Path, extra: &[&str]) -> Result<Vec<String>> {
    let mut args = vec!["ls-files", "-z"];
    args.extend_from_slice(extra);
    let mut paths = split_z(&run(dir, &args)?);
    paths.dedup();
    Ok(paths)
}

/// NUL-separated paths, sorted.
pub fn split_z(bytes: &[u8]) -> Vec<String> {
    let mut paths: Vec<String> = String::from_utf8_lossy(bytes)
        .split('\0')
        .filter(|p| !p.is_empty())
        .map(str::to_owned)
        .collect();
    paths.sort();
    paths.dedup();
    paths
}

/// The directory given with the global `--root <dir>`, if any.
static ROOT: std::sync::OnceLock<PathBuf> = std::sync::OnceLock::new();

/// Sets the directory [`root`] starts from, in place of the current directory. Paths given
/// as other arguments stay relative to the current directory.
pub fn set_root(dir: &Path) -> Result<()> {
    let dir = dir
        .canonicalize()
        .with_context(|| format!("--root {}", dir.display()))?;
    if ROOT.set(dir).is_err() {
        bail!("`--root` given twice");
    }
    Ok(())
}

/// The repository root: the git top level of the start directory when it holds a
/// `repo.toml`, else the nearest ancestor of the start directory that holds one. The start
/// directory is `--root`'s, else the current directory.
pub fn root() -> Result<PathBuf> {
    if let Some(dir) = ROOT.get() {
        return root_from(dir);
    }
    let cwd = std::env::current_dir().context("reading the current directory")?;
    root_from(&cwd)
}

/// [`root`], starting from `start`.
pub fn root_from(start: &Path) -> Result<PathBuf> {
    if let Ok(top) = text(start, &["rev-parse", "--show-toplevel"]) {
        let top = PathBuf::from(top.trim());
        if top.join(crate::config::FILE).is_file() {
            return Ok(top);
        }
    }
    let mut dir = Some(start);
    while let Some(d) = dir {
        if d.join(crate::config::FILE).is_file() {
            return Ok(d.to_path_buf());
        }
        dir = d.parent();
    }
    bail!(
        "no {} found in {} or any parent directory",
        crate::config::FILE,
        start.display()
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn split_z_sorts_and_dedups() {
        assert_eq!(split_z(b"b\0a\0b\0\0"), vec!["a", "b"]);
    }

    #[test]
    fn root_walks_up_to_repo_toml() {
        let dir = std::env::temp_dir().join(format!("repo-root-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(dir.join("a/b")).unwrap();
        std::fs::write(dir.join(crate::config::FILE), "").unwrap();
        let found = root_from(&dir.join("a/b")).unwrap();
        assert_eq!(found.canonicalize().unwrap(), dir.canonicalize().unwrap());
        let _ = std::fs::remove_dir_all(&dir);
    }
}
