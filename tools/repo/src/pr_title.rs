//! `repo pr-title "<title>"`: a PR title must be a Conventional Commit header,
//! `type(scope)!: summary`, or a revert, `Revert "<title>"`.

use anyhow::bail;

use crate::Outcome;

pub const TYPES: &[&str] = &[
    "feat", "fix", "docs", "style", "refactor", "perf", "test", "build", "ci", "chore", "revert",
];
const MAX_LEN: usize = 100;

/// Why `title` is not a Conventional Commit header; `None` when it is.
pub fn problem(title: &str) -> Option<String> {
    if title != title.trim() || title.is_empty() {
        return Some("the title is empty or has surrounding whitespace".into());
    }
    if title.chars().count() > MAX_LEN {
        return Some(format!("the title is longer than {MAX_LEN} characters"));
    }
    if let Some(inner) = title.strip_prefix("Revert \"") {
        return (!inner.ends_with('"') || inner.len() < 2)
            .then(|| "a revert title is `Revert \"<original title>\"`".to_owned());
    }
    let Some((head, summary)) = title.split_once(": ") else {
        return Some("expected `type(scope)!: summary`, with a colon and a space".into());
    };
    let head = head.strip_suffix('!').unwrap_or(head);
    let (kind, scope) = match head.split_once('(') {
        Some((k, rest)) => match rest.strip_suffix(')') {
            Some(s) => (k, Some(s)),
            None => return Some("the scope must be closed with `)`".into()),
        },
        None => (head, None),
    };
    if !TYPES.contains(&kind) {
        return Some(format!("type `{kind}` is not one of {}", TYPES.join(", ")));
    }
    if scope.is_some_and(|s| {
        s.is_empty()
            || !s
                .chars()
                .all(|c| c.is_ascii_alphanumeric() || "-_/.,".contains(c))
    }) {
        return Some("the scope must be a non-empty word (letters, digits, - _ / . ,)".into());
    }
    if summary.trim().is_empty() {
        return Some("the summary is empty".into());
    }
    None
}

pub fn run(args: &[String]) -> Outcome {
    let [title] = args else {
        bail!("usage: repo pr-title \"<title>\"");
    };
    match problem(title) {
        None => {
            println!("pr-title: ok");
            Ok(true)
        }
        Some(p) => {
            println!("pr-title: {p}: {title:?}");
            Ok(false)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn accepts_conventional_titles() {
        for t in [
            "feat: add parser",
            "fix(cli): handle empty args",
            "feat(api)!: drop v1",
            "chore(wave): land wave/m1",
            "Revert \"feat: add parser\"",
        ] {
            assert_eq!(problem(t), None, "{t}");
        }
    }

    #[test]
    fn rejects_others() {
        for t in [
            "Add parser",
            "feat:missing space",
            "feature: x",
            "fix(: x",
            "fix(): x",
            "feat: ",
            " feat: x",
            "Revert \"x",
        ] {
            assert!(problem(t).is_some(), "{t}");
        }
        assert!(problem(&format!("feat: {}", "x".repeat(200))).is_some());
    }
}
