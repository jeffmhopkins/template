//! `governance`, `lane-guard`, `progress`, `plan check`, `config check`, `heads-changed`,
//! `stack run` and `pr-title` against temporary repositories.

mod common;

use common::{Fixture, doc};

#[test]
fn governance_lists_and_enforces() {
    let f = Fixture::new(
        "governance",
        &[
            ("docs/A.md", &doc("A", "a", "src/**", "About src.")),
            ("src/a.rs", "fn a() {}\n"),
        ],
    );
    f.git(&["tag", "base"]);
    f.git(&["checkout", "-q", "-b", "work"]);
    // Plain prose and code: no governance change.
    f.write("src/a.rs", "fn a() { let _ = 1; }\n");
    f.commit("code");
    let r = f.repo(&["governance", "--base", "base", "--labels", ""]);
    assert!(
        r.ok() && r.out.contains("no governance changes"),
        "{}",
        r.out
    );
    // Narrowed covers, a new suppression, a protected file and a maintainer-only file.
    f.write("docs/A.md", &doc("A", "a", "src/a.rs", "About src."));
    f.write("src/a.rs", "#[allow(dead_code)]\nfn a() {}\n");
    f.write(".github/workflows/ci.yml", "jobs:\n  fmt:\n");
    f.write("REVIEW.md", "x\n");
    f.commit("governance");
    let list = f.repo(&["governance", "--base", "base"]);
    assert!(list.ok(), "listing never fails: {}", list.out);
    for needle in [
        "docs/A.md: section `#a` `covers`",
        "src/a.rs: line 1: new suppression `#[allow(dead_code)]` (missing `reason",
        ".github/workflows/ci.yml: adds a protected path",
        "maintainer-only: REVIEW.md",
    ] {
        assert!(
            list.out.contains(needle),
            "missing {needle:?} in\n{}",
            list.out
        );
    }
    let body = f.path().join("../gov-body.md");
    std::fs::write(
        &body,
        "## Summary\nx\n\n## Governance change\nNarrows covers to the one file it describes.\n",
    )
    .unwrap();
    let body = body.to_string_lossy().into_owned();
    let unlabeled = f.repo(&[
        "governance",
        "--base",
        "base",
        "--pr-body",
        &body,
        "--labels",
        "bug",
    ]);
    assert_eq!(unlabeled.code, 1, "{}", unlabeled.out);
    assert!(
        unlabeled
            .out
            .contains("lacks the `governance-approved` label"),
        "{}",
        unlabeled.out
    );
    let ok = f.repo(&[
        "governance",
        "--base",
        "base",
        "--pr-body",
        &body,
        "--labels",
        "bug,governance-approved",
    ]);
    assert!(ok.ok(), "{}", ok.out);
    let _ = std::fs::remove_file(&body);
    // The head's own config cannot unprotect a path: the base's rules apply.
    f.write("repo.toml", &common::CONFIG.replace("\".github/**\", ", ""));
    f.commit("unprotect");
    let r = f.repo(&["governance", "--base", "base"]);
    assert!(
        r.out
            .contains(".github/workflows/ci.yml: adds a protected path"),
        "{}",
        r.out
    );
    assert!(
        r.out.contains("repo.toml: edits a protected path"),
        "{}",
        r.out
    );
}

#[test]
fn lane_guard_checks_claims() {
    let f = Fixture::new(
        "lanes",
        &[("lanes.toml", "protocol = { value = 21, holder = 500 }\n")],
    );
    f.git(&["checkout", "-q", "-b", "pr"]);
    f.write(
        "lanes.toml",
        "protocol = { value = 22, holder = 512 }\nschema = { value = 1, holder = 512 }\n",
    );
    f.commit("claim");
    let ok = f.repo(&[
        "lane-guard",
        "--base",
        "main",
        "--head",
        "HEAD",
        "--pr",
        "512",
    ]);
    assert!(ok.ok(), "{}", ok.out);
    let wrong = f.repo(&[
        "lane-guard",
        "--base",
        "main",
        "--head",
        "HEAD",
        "--pr",
        "513",
    ]);
    assert_eq!(wrong.code, 1, "{}", wrong.out);
    // Main took the slot first: the claim is no longer base + 1.
    f.git(&["checkout", "-q", "main"]);
    f.write("lanes.toml", "protocol = { value = 22, holder = 600 }\n");
    f.commit("other claim");
    let stale = f.repo(&[
        "lane-guard",
        "--base",
        "main",
        "--head",
        "pr",
        "--pr",
        "512",
    ]);
    assert_eq!(stale.code, 1, "{}", stale.out);
    assert!(stale.out.contains("must be 23"), "{}", stale.out);
    let none = f.repo(&[
        "lane-guard",
        "--base",
        "main",
        "--head",
        "main",
        "--pr",
        "1",
    ]);
    assert!(
        none.ok() && none.out.contains("changes no lane"),
        "{}",
        none.out
    );
}

const PLAN: &str = "[[milestone]]\nid = \"M0\"\ntitle = \"Start\"\ndemo = \"It runs.\"\n\n[[item]]\nid = \"M0-1\"\nmilestone = \"M0\"\ntitle = \"One\"\n\n[[item]]\nid = \"M0-2\"\nmilestone = \"M0\"\ntitle = \"Two\"\ndeps = [\"M0-1\"]\nhardware = true\n\n[[item]]\nid = \"M0-3\"\nmilestone = \"M0\"\ntitle = \"Three\"\n";

#[test]
fn progress_from_trailers_with_reverts() {
    let f = Fixture::new("progress", &[("docs/progress.toml", PLAN)]);
    for (file, msg) in [
        ("a", "feat: one (#1)\n\nCompletes: M0-1"),
        ("b", "feat: two (#2)\n\nCompletes: M0-2"),
        ("c", "feat: three part (#3)\n\nPart-of: M0-3"),
        ("d", "feat: three (#4)\n\nCompletes: M0-3"),
    ] {
        f.write(file, msg);
        f.git(&["add", "-A"]);
        f.git(&["commit", "-q", "-m", msg]);
    }
    f.git(&["revert", "--no-edit", "HEAD"]);
    let r = f.repo(&["progress", "--format", "json"]);
    assert!(r.ok(), "{}", r.out);
    let v: serde_json::Value = serde_json::from_str(&r.out).expect("json");
    let statuses: Vec<&str> = v["milestones"][0]["items"]
        .as_array()
        .unwrap()
        .iter()
        .map(|i| i["status"].as_str().unwrap())
        .collect();
    assert_eq!(statuses, vec!["done", "hardware", "in-progress"]);
    f.write("e", "x");
    f.git(&["add", "-A"]);
    f.git(&[
        "commit",
        "-q",
        "-m",
        "chore: run (#5)\n\nVerified-on-hardware: M0-2",
    ]);
    let md = f.repo(&["progress"]);
    assert!(
        md.ok() && md.out.contains("| M0: Start | 2 of 3 | 1 | 0 | 0 |"),
        "{}",
        md.out
    );
    // README progress block is written by `docs generate`, and `docs check` validates the plan.
    let readme = "<!-- @doc\nid = \"README\"\ntitle = \"R\"\nkind = \"readme\"\nstatus = \"active\"\n-->\n\n# R\n\n<!-- @generated begin progress -->\n<!-- @generated end progress -->\n";
    f.write("README.md", readme);
    f.commit("readme");
    assert!(f.repo(&["docs", "generate"]).ok());
    assert!(f.read("README.md").contains("| M0-2 | Two | done | M0-1 |"));
    f.write(
        "docs/progress.toml",
        &format!("{PLAN}\n[[item]]\nid = \"M0-1\"\nmilestone = \"M0\"\ntitle = \"dup\"\n"),
    );
    let r = f.repo(&["docs", "check"]);
    assert!(
        !r.ok() && r.out.contains("duplicate item `M0-1`"),
        "{}",
        r.out
    );
}

#[test]
fn plan_check_finds_overlaps() {
    let f = Fixture::dir("plan");
    let launch = "# Launch\n\n<a id=\"B1\"></a>\n## B1\n| Field | Value |\n|---|---|\n| owns | `src/a/**` |\n\n<a id=\"B2\"></a>\n## B2\n| owns | `src/b/**`, `docs/B.md` |\n\n<a id=\"B3\"></a>\n## B3\n| owns | `src/**` |\n";
    let roster = "| Row | Session | State |\n|---|---|---|\n| planner | s0 | running |\n| B1 | s1 | running |\n| B2 | s2 | review |\n| B3 | s3 | merged |\n";
    f.write("launch.md", launch);
    f.write("roster.md", roster);
    let (l, r) = (f.path().join("launch.md"), f.path().join("roster.md"));
    let (l, r) = (l.to_string_lossy(), r.to_string_lossy());
    let ok = f.repo(&["plan", "check", &l, &r]);
    assert!(ok.ok(), "{}", ok.out);
    let bad = f.repo(&["plan", "check", &l, &r, "--row", "B3"]);
    assert_eq!(bad.code, 1, "{}", bad.out);
    assert!(
        bad.out.contains("rows `B1` and `B3`") && bad.out.contains("rows `B2` and `B3`"),
        "{}",
        bad.out
    );
    let unknown = f.repo(&["plan", "check", &l, &r, "--row", "B9"]);
    assert_eq!(unknown.code, 1, "{}", unknown.out);
}

#[test]
fn config_check_compares_required_checks_with_workflows() {
    let f = Fixture::new("config", &[("lanes.toml", "# none\n")]);
    let r = f.repo(&["config", "check"]);
    assert_eq!(r.code, 1, "{}", r.out);
    assert!(
        r.out.contains("required check `fmt` is not a job"),
        "{}",
        r.out
    );
    f.write(".github/workflows/ci.yml", "on: push\njobs:\n  fmt:\n    runs-on: x\n  documentation:\n    name: docs\n    runs-on: x\n");
    let r = f.repo(&["config", "check"]);
    assert!(r.ok(), "{}", r.out);
    f.write(
        "repo.toml",
        &format!("{}\n[extra]\nx = 1\n", common::CONFIG),
    );
    let r = f.repo(&["config", "check"]);
    assert_eq!(r.code, 1, "{}", r.out);
    assert!(r.out.contains("unknown field `extra`"), "{}", r.out);
    f.write("repo.toml", common::CONFIG);
    f.write("lanes.toml", "p = { value = 1 }\n");
    let r = f.repo(&["config", "check"]);
    assert!(!r.ok() && r.out.contains("lane `p`"), "{}", r.out);
}

#[test]
fn heads_changed_tracks_origin() {
    let origin = Fixture::new("heads-origin", &[]);
    let work = origin.clone_as("heads-work");
    let state = work.path().join("../heads-state.txt");
    let state = state.to_string_lossy().into_owned();
    let _ = std::fs::remove_file(&state);
    let run = || work.repo(&["heads-changed", "--state", &state]);
    assert_eq!(run().out.trim(), "changed");
    assert_eq!(run().out.trim(), "unchanged");
    origin.write("x", "x");
    origin.commit("move");
    let r = run();
    assert!(r.ok() && r.out.trim() == "changed", "{}", r.out);
    let _ = std::fs::remove_file(&state);
}

#[test]
fn stack_run_skips_without_detect_file() {
    let f = Fixture::new("stack", &[]);
    let r = f.repo(&["stack", "run", "fmt"]);
    assert!(r.ok() && r.out.contains("skip: no Cargo.toml"), "{}", r.out);
    f.write("Cargo.toml", "");
    let r = f.repo(&["stack", "run", "fmt"]);
    assert!(r.ok() && r.out.contains("formatting"), "{}", r.out);
    assert_eq!(f.repo(&["stack", "run", "nope"]).code, 2);
}

#[test]
fn pr_title_exit_codes() {
    let f = Fixture::dir("title");
    assert!(f.repo(&["pr-title", "feat(docs): add the ledger"]).ok());
    assert_eq!(f.repo(&["pr-title", "Add the ledger"]).code, 1);
    assert_eq!(f.repo(&["nope"]).code, 2);
}
