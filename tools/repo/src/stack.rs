//! `repo stack <run|setup|sweep>`: the stack pack's side of CI.
//!
//! - `stack run <job>` runs the pack's command for a CI job from the repository root.
//! - `stack setup <dir>` puts the pack's setup action (`[ci].setup`) at `<dir>`, or a no-op
//!   action there, so a workflow step `uses: ./<dir>` runs whichever the pack names.
//! - `stack sweep` runs the pack's `[[sweep]]` items and prints each one's Markdown under its
//!   title. It always exits 0.
//!
//! Each prints `skip: no <detect>`, or a skip line, and does nothing while the pack's detect
//! file is absent.

use std::path::Path;
use std::process::Command;

use anyhow::{Context, Result, bail};

use crate::Outcome;
use crate::config::Repo;

/// Output past this many bytes is cut, keeping the end, where the errors are.
const TAIL: usize = 15_000;

/// The action written when the pack names no setup action, or the stack isn't in use.
const NOOP_ACTION: &str = "\
# Written by `scripts/repo stack setup`: the stack pack names no setup action, or the stack
# is not in use yet.
name: stack-setup
description: No stack setup.
inputs:
  job:
    description: The CI job the setup is for.
    required: false
runs:
  using: composite
  steps:
    - shell: bash
      run: echo \"No stack setup.\"
";

pub fn run(sub: &str, args: &[String]) -> Outcome {
    let repo = Repo::open()?;
    match sub {
        "run" => run_job(&repo, args),
        "setup" => setup(&repo, args),
        "sweep" => sweep(&repo, args),
        _ => bail!("usage: repo stack run <job> | stack setup <dir> | stack sweep"),
    }
}

fn in_use(repo: &Repo) -> bool {
    repo.root.join(&repo.pack.detect).exists()
}

fn run_job(repo: &Repo, args: &[String]) -> Outcome {
    let [job] = args else {
        bail!("usage: repo stack run <job>");
    };
    let Some(cmd) = repo.pack.jobs.get(job.as_str()) else {
        let known: Vec<&str> = repo.pack.jobs.keys().map(String::as_str).collect();
        bail!(
            "the stack pack {} has no job `{job}` (it has: {})",
            repo.config.pack_path(),
            known.join(", ")
        );
    };
    if !in_use(repo) {
        println!("skip: no {}", repo.pack.detect);
        return Ok(true);
    }
    println!("stack run {job}: {cmd}");
    let status = Command::new("sh")
        .args(["-c", cmd])
        .current_dir(&repo.root)
        .status()
        .with_context(|| format!("running `{cmd}`"))?;
    Ok(status.success())
}

/// Whether `dir` is a plain repo-relative path.
fn relative(dir: &str) -> bool {
    !dir.is_empty()
        && !dir.starts_with('/')
        && !dir.split('/').any(|c| c == ".." || c == ".")
        && !dir.contains('\\')
}

fn setup(repo: &Repo, args: &[String]) -> Outcome {
    let [dir] = args else {
        bail!("usage: repo stack setup <dir>");
    };
    let dir = dir.trim_end_matches('/');
    if !relative(dir) {
        bail!("`{dir}` must be a path inside the repository, such as .stack-setup");
    }
    let tracked = crate::git::text(&repo.root, &["ls-files", "--", dir])?;
    if !tracked.trim().is_empty() {
        bail!("`{dir}` holds tracked files; give a directory of its own, such as .stack-setup");
    }
    let target = repo.root.join(dir);
    if target.exists() {
        std::fs::remove_dir_all(&target)
            .with_context(|| format!("removing the earlier {}", target.display()))?;
    }
    std::fs::create_dir_all(&target).with_context(|| format!("creating {}", target.display()))?;
    match &repo.pack.ci.setup {
        Some(src) if in_use(repo) => {
            copy_dir(&repo.root.join(src.trim_end_matches('/')), &target)?;
            println!("stack setup: {src} → {dir}");
        }
        Some(_) => {
            std::fs::write(target.join("action.yml"), NOOP_ACTION)?;
            println!("skip: no {}; a no-op action → {dir}", repo.pack.detect);
        }
        None => {
            std::fs::write(target.join("action.yml"), NOOP_ACTION)?;
            println!("stack setup: the pack names no [ci].setup; a no-op action → {dir}");
        }
    }
    Ok(true)
}

fn copy_dir(from: &Path, to: &Path) -> Result<()> {
    let entries = std::fs::read_dir(from).with_context(|| format!("reading {}", from.display()))?;
    for e in entries {
        let e = e?;
        let dest = to.join(e.file_name());
        if e.file_type()?.is_dir() {
            std::fs::create_dir_all(&dest)?;
            copy_dir(&e.path(), &dest)?;
        } else {
            std::fs::copy(e.path(), &dest)
                .with_context(|| format!("copying {}", e.path().display()))?;
        }
    }
    Ok(())
}

/// The last `TAIL` bytes of `text`, on a character boundary.
fn tail(text: &str) -> &str {
    let mut start = text.len().saturating_sub(TAIL);
    while !text.is_char_boundary(start) {
        start += 1;
    }
    &text[start..]
}

fn sweep(repo: &Repo, args: &[String]) -> Outcome {
    if !args.is_empty() {
        bail!("usage: repo stack sweep");
    }
    if repo.pack.sweep.is_empty() {
        println!("The stack pack has no sweep items.");
        return Ok(true);
    }
    for item in &repo.pack.sweep {
        println!("### {}\n", item.title);
        if !in_use(repo) {
            println!("Skipped: no `{}`.\n", repo.pack.detect);
            continue;
        }
        match Command::new("sh")
            .args(["-c", &item.run])
            .current_dir(&repo.root)
            .output()
        {
            Err(e) => println!("**Could not run** `{}`: {e}\n", item.run),
            Ok(o) if o.status.success() => {
                let text = String::from_utf8_lossy(&o.stdout);
                if text.trim().is_empty() {
                    println!("Passed: `{}`.\n", item.run);
                } else {
                    println!("{}\n", tail(text.trim_end()));
                }
            }
            Ok(o) => {
                let code = o
                    .status
                    .code()
                    .map_or_else(|| "a signal".to_owned(), |c| c.to_string());
                let text = format!(
                    "{}{}",
                    String::from_utf8_lossy(&o.stdout),
                    String::from_utf8_lossy(&o.stderr)
                );
                println!(
                    "**Failed** (exit {code}): `{}`\n\n```text\n{}\n```\n",
                    item.run,
                    tail(text.trim_end()).replace("```", "'''")
                );
            }
        }
    }
    Ok(true)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn paths_and_tails() {
        assert!(relative(".stack-setup"));
        assert!(relative("a/b"));
        for bad in ["", "/abs", "../up", "a/../b", "./a", "a\\b"] {
            assert!(!relative(bad), "{bad}");
        }
        assert_eq!(tail("short"), "short");
        let long = format!("{}é{}", "x".repeat(TAIL), "end");
        let t = tail(&long);
        assert!(t.ends_with("end") && t.len() <= TAIL);
    }
}
