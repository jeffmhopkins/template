//! `repo heads-changed --state <file>`: whether any ref on `origin` moved since the last
//! run. Prints `changed` or `unchanged`, writes the current refs to the state file, and
//! exits 0 either way.

use anyhow::{Context, Result};

use crate::Outcome;
use crate::args::Args;

/// The `git ls-remote` listing, sorted, one `<sha> <ref>` per line.
pub fn normalize(listing: &str) -> String {
    let mut lines: Vec<String> = listing
        .lines()
        .filter_map(|l| {
            let (sha, r) = l.split_once(char::is_whitespace)?;
            Some(format!("{sha} {}", r.trim()))
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

fn listing() -> Result<String> {
    let dir = crate::git::root()
        .or_else(|_| std::env::current_dir().context("reading the current directory"))?;
    crate::git::text(&dir, &["ls-remote", "origin"])
}

pub fn run(args: &[String]) -> Outcome {
    let usage = "repo heads-changed --state <file>";
    let a = Args::parse(args, &["state"], &[])?;
    a.exactly(0, usage)?;
    let state = a.required("state", usage)?;
    let now = normalize(&listing()?);
    let saved = std::fs::read_to_string(state).ok();
    let moved = changed(saved.as_deref(), &now);
    if moved {
        if let Some(dir) = std::path::Path::new(state).parent() {
            if !dir.as_os_str().is_empty() {
                std::fs::create_dir_all(dir)
                    .with_context(|| format!("creating {}", dir.display()))?;
            }
        }
        std::fs::write(state, &now).with_context(|| format!("writing {state}"))?;
    }
    println!("{}", if moved { "changed" } else { "unchanged" });
    Ok(true)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn order_and_spacing_do_not_matter() {
        let a = normalize("b\trefs/heads/x\na\tHEAD\n");
        let b = normalize("a HEAD\nb refs/heads/x");
        assert_eq!(a, b);
        assert!(!changed(Some(&a), &b));
        assert!(changed(None, &a));
        assert!(changed(Some(&a), &normalize("c\tHEAD\n")));
    }
}
