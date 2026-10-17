//! `repo docs …` against temporary repositories.

mod common;

use common::{Fixture, NOTE, doc};

#[test]
fn check_verify_and_staleness() {
    let f = Fixture::new(
        "verify",
        &[
            ("docs/A.md", &doc("A", "a", "src/**", "About src.")),
            ("src/a.rs", "a\n"),
        ],
    );
    let r = f.repo(&["docs", "check"]);
    assert!(
        !r.ok() && r.out.contains("section `a` is stale (no stamp"),
        "{}",
        r.out
    );
    let draft = f.repo(&["docs", "check", "--draft"]);
    assert!(
        draft.ok() && draft.out.contains("warning: section `a` is stale"),
        "{}",
        draft.out
    );
    let stock = f.repo(&["docs", "verify", "A#a", "--by", "agent:t", "--note", "ok"]);
    assert_eq!(stock.code, 2, "{}", stock.out);
    let v = f.repo(&["docs", "verify", "A#a", "--by", "agent:t", "--note", NOTE]);
    assert!(v.ok(), "{}", v.out);
    let ledger = f.read("docs/verified.toml");
    assert!(
        ledger.contains("[\"A#a\".\"file:src/a.rs\"]\nstamp = { hash = \"b3:"),
        "{ledger}"
    );
    let r = f.repo(&["docs", "check"]);
    assert!(r.ok(), "{}", r.out);
    f.write("src/a.rs", "changed\n");
    let r = f.repo(&["docs", "check"]);
    assert!(
        !r.ok() && r.out.contains("changed since its stamp: file:src/a.rs"),
        "{}",
        r.out
    );
    // An untracked covered file must be added before verifying.
    f.write("src/b.rs", "b\n");
    let v = f.repo(&["docs", "verify", "A#a", "--by", "agent:t", "--note", NOTE]);
    assert!(!v.ok() && v.out.contains("src/b.rs"), "{}", v.out);
}

#[test]
fn structural_findings() {
    let bad = "<!-- @doc\nid = \"B\"\ntitle = \"B\"\nkind = \"nope\"\nstatus = \"active\"\nextra = 1\n-->\n\n## No anchor\n\nSee [missing](gone.md) and [ext](https://example.com/x).\n\n<a id=\"R-X-001\"></a>\n### R-X-001: Rule\n<!-- @rule\nstatus = \"active\"\nenforced_by = [\"ci:nope\"]\n-->\nText.\n";
    let f = Fixture::new(
        "structure",
        &[("docs/B.md", bad), ("NOTES.md", "no metadata\n")],
    );
    let r = f.repo(&["docs", "check"]);
    assert_eq!(r.code, 1, "{}", r.out);
    for needle in [
        "unknown kind `nope`",
        "unknown `@doc` key `extra`",
        "has no `<a id=",
        "`docs/gone.md` does not exist",
        "is not registered in docs/external.toml",
        "`enforced_by` entry `ci:nope`",
        "NOTES.md:1: file must start with `<!-- @doc`",
    ] {
        assert!(r.out.contains(needle), "missing {needle:?} in\n{}", r.out);
    }
    let draft = f.repo(&["docs", "check", "--draft"]);
    assert!(
        draft.ok() && draft.out.contains("not failing a draft"),
        "{}",
        draft.out
    );
}

const ADR: &str = "<!-- @doc\nid = \"ADR-0001\"\ntitle = \"First\"\nkind = \"adr\"\nstatus = \"accepted\"\ntier = \"agent\"\n-->\n\n# ADR-0001: First\n\n<a id=\"context\"></a>\n## Context\nWhy.\n";
const INDEX: &str = "<!-- @doc\nid = \"ADR-INDEX\"\ntitle = \"ADRs\"\nkind = \"readme\"\nstatus = \"active\"\n-->\n\n# ADRs\n\n<!-- @generated begin adr-index -->\n<!-- @generated end adr-index -->\n";

#[test]
fn generate_writes_banners_and_index() {
    let f = Fixture::new(
        "generate",
        &[
            ("docs/adr/0001-first.md", ADR),
            ("docs/adr/README.md", INDEX),
        ],
    );
    let r = f.repo(&["docs", "check"]);
    assert!(
        !r.ok() && r.out.contains("generated content is out of date"),
        "{}",
        r.out
    );
    assert!(f.repo(&["docs", "generate"]).ok());
    assert!(f.read("docs/adr/0001-first.md").contains(
        "# ADR-0001: First\n\n<!-- @generated begin adr-status -->\n> **Status:** accepted\n<!-- @generated end adr-status -->\n"
    ));
    assert!(
        f.read("docs/adr/README.md")
            .contains("| [ADR-0001](0001-first.md) | First | agent | accepted |")
    );
    let r = f.repo(&["docs", "check"]);
    assert!(r.ok(), "{}", r.out);
}

#[test]
fn docs_for_lists_sections_rules_and_adrs() {
    let rules = "<!-- @doc\nid = \"RULES\"\ntitle = \"Rules\"\nkind = \"normative\"\nstatus = \"active\"\n-->\n\n<a id=\"code\"></a>\n## Code\n<!-- @section\n-->\n\n<a id=\"R-SRC-001\"></a>\n### R-SRC-001: Small files\n<!-- @rule\nstatus = \"active\"\nenforced_by = [\"review\"]\napplies_to = [\"src/**\"]\n-->\nPer [ADR-0001](adr/0001-first.md).\n";
    let f = Fixture::new(
        "for",
        &[
            ("docs/A.md", &doc("A", "a", "src/**", "About src.")),
            ("docs/RULES.md", rules),
            ("docs/adr/0001-first.md", ADR),
            ("src/a.rs", "a\n"),
        ],
    );
    let r = f.repo(&["docs", "for", "src/new.rs"]);
    assert!(r.ok(), "{}", r.out);
    assert!(r.out.contains("A#a  (Section)"), "{}", r.out);
    assert!(
        r.out.contains("RULES#R-SRC-001  (R-SRC-001: Small files)"),
        "{}",
        r.out
    );
    assert!(r.out.contains("ADR-0001  (First)"), "{}", r.out);
}

#[test]
fn review_log_and_due() {
    let text = "<!-- @doc\nid = \"G\"\ntitle = \"G\"\nkind = \"guide\"\nstatus = \"active\"\nreview_every = \"1d\"\n-->\n\n# G\n\nGuide.\n";
    let f = Fixture::new("review", &[("docs/G.md", text)]);
    let due = f.repo(&["docs", "due"]);
    assert!(due.ok(), "{}", due.out);
    assert!(due.out.contains("nothing is due"), "{}", due.out);
    let r = f.repo(&[
        "docs",
        "review",
        "G",
        "--by",
        "human:m",
        "--note",
        "re-read the guide against the workflow",
    ]);
    assert!(r.ok(), "{}", r.out);
    assert!(
        f.read("docs/G.md")
            .contains("reviewed = { by = \"human:m\"")
    );
    f.commit("docs: review G (#2)");
    let log = f.repo(&["docs", "log", "G"]);
    assert!(
        log.ok() && log.out.contains("reviewed  by human:m"),
        "{}",
        log.out
    );
    // Backdate history: the clock started two days ago, so the doc is due.
    let old = Fixture::new("due", &[]);
    old.write("docs/G.md", text);
    old.git(&["add", "-A"]);
    old.git(&[
        "-c",
        "user.name=t",
        "commit",
        "-q",
        "-m",
        "old",
        "--date=2000-01-01T00:00:00",
    ]);
    let env_date = std::process::Command::new("git")
        .args(["commit", "-q", "--amend", "--no-edit"])
        .env("GIT_COMMITTER_DATE", "2000-01-01T00:00:00")
        .env("GIT_COMMITTER_NAME", "t")
        .env("GIT_COMMITTER_EMAIL", "t@t")
        .current_dir(old.path())
        .status()
        .expect("git runs");
    assert!(env_date.success());
    let due = old.repo(&["docs", "due"]);
    assert!(
        due.ok() && due.out.contains("doc G (docs/G.md)"),
        "{}",
        due.out
    );
}

/// `origin` on `main` with one stamped section, and a clone of it.
fn origin_and_clone(name: &str, extra: &[(&str, &str)]) -> (Fixture, Fixture) {
    let mut files = vec![
        ("docs/S.md", doc("S", "s", "src/**", "About src.")),
        ("src/a.rs", "one\ntwo\nthree\nfour\nfive\nsix\n".to_owned()),
    ];
    files.extend(extra.iter().map(|(p, t)| (*p, (*t).to_owned())));
    let refs: Vec<(&str, &str)> = files.iter().map(|(p, t)| (*p, t.as_str())).collect();
    let origin = Fixture::new(&format!("{name}-origin"), &refs);
    assert!(
        origin
            .repo(&[
                "docs",
                "verify",
                "S#s",
                "--by",
                "agent:base",
                "--note",
                NOTE
            ])
            .ok()
    );
    origin.commit("stamp");
    let work = origin.clone_as(&format!("{name}-work"));
    (origin, work)
}

#[test]
fn merge_main_keeps_the_matching_side() {
    let (origin, work) = origin_and_clone("merge-same", &[]);
    // Both sides make the same change and verify it with different notes.
    for (f, who) in [(&origin, "main"), (&work, "work")] {
        f.write("src/a.rs", "one\ntwo\nthree\nfour\nfive\nsix\nseven\n");
        let note = format!("{NOTE}, from {who}");
        assert!(
            f.repo(&[
                "docs",
                "verify",
                "S#s",
                "--by",
                &format!("agent:{who}"),
                "--note",
                &note
            ])
            .ok()
        );
        f.commit(who);
    }
    let m = work.repo(&["docs", "merge-main"]);
    assert!(m.ok(), "{}", m.out);
    assert!(m.out.contains("resolved ledger conflict: S#s"), "{}", m.out);
    assert!(!m.out.contains("stale:"), "{}", m.out);
    assert!(work.read("docs/verified.toml").contains("from main"));
    assert!(work.repo(&["docs", "check"]).ok());
}

#[test]
fn merge_main_marks_stale_and_lists_notes() {
    let (origin, work) = origin_and_clone("merge-stale", &[]);
    origin.write("src/a.rs", "one main\ntwo\nthree\nfour\nfive\nsix\n");
    assert!(
        origin
            .repo(&[
                "docs",
                "verify",
                "S#s",
                "--by",
                "agent:main",
                "--note",
                &format!("{NOTE} on main")
            ])
            .ok()
    );
    origin.commit("main");
    work.write("src/a.rs", "one\ntwo\nthree\nfour\nfive\nsix work\n");
    let work_note = "the section still describes the sixth line the branch changed";
    assert!(
        work.repo(&[
            "docs",
            "verify",
            "S#s",
            "--by",
            "agent:work",
            "--note",
            work_note
        ])
        .ok()
    );
    work.commit("work");
    let m = work.repo(&["docs", "merge-main"]);
    assert_eq!(m.code, 1, "{}", m.out);
    assert!(m.out.contains("stale: S#s"), "{}", m.out);
    assert!(
        m.out.contains(&format!("previous note: {work_note}")),
        "{}",
        m.out
    );
    assert!(
        work.read("src/a.rs").contains("one main") && work.read("src/a.rs").contains("six work")
    );
    assert!(!work.read("docs/verified.toml").contains("<<<<<<<"));
    // The merge is committed; re-verifying makes the tree clean.
    assert!(
        work.repo(&[
            "docs",
            "verify",
            "S#s",
            "--by",
            "agent:work",
            "--note",
            "re-read after the merge: both line edits are described"
        ])
        .ok()
    );
    assert!(work.repo(&["docs", "check"]).ok());
}

#[test]
fn merge_main_regenerates_and_leaves_other_conflicts() {
    let (origin, work) = origin_and_clone(
        "merge-gen",
        &[
            ("docs/adr/0001-first.md", ADR),
            ("docs/adr/README.md", INDEX),
        ],
    );
    assert!(origin.repo(&["docs", "generate"]).ok());
    origin.commit("generate");
    work.git(&["pull", "-q", "--no-rebase", "origin", "main"]);
    for (f, n) in [(&origin, "0002"), (&work, "0003")] {
        let adr = ADR
            .replace("0001", n)
            .replace("First", &format!("Number {n}"));
        f.write(&format!("docs/adr/{n}-x.md"), &adr);
        f.git(&["add", "-A"]);
        assert!(f.repo(&["docs", "generate"]).ok());
        f.commit(n);
    }
    let m = work.repo(&["docs", "merge-main"]);
    assert!(m.ok(), "{}", m.out);
    assert!(
        m.out.contains("regenerated: docs/adr/README.md"),
        "{}",
        m.out
    );
    let index = work.read("docs/adr/README.md");
    assert!(
        index.contains("ADR-0002") && index.contains("ADR-0003") && !index.contains("<<<<<<<"),
        "{index}"
    );
    assert!(work.repo(&["docs", "check"]).ok());

    // A prose conflict is left alone and the merge stays open.
    for (f, who) in [(&origin, "main"), (&work, "work")] {
        let text = f
            .read("docs/S.md")
            .replace("About src.", &format!("About src, by {who}."));
        f.write("docs/S.md", &text);
        f.git(&["add", "-A"]);
        assert!(
            f.repo(&[
                "docs",
                "verify",
                "S#s",
                "--by",
                &format!("agent:{who}"),
                "--note",
                &format!("{NOTE} by {who}")
            ])
            .ok()
        );
        f.commit(who);
    }
    let m = work.repo(&["docs", "merge-main"]);
    assert_eq!(m.code, 1, "{}", m.out);
    assert!(m.out.contains("left for you: docs/S.md"), "{}", m.out);
    assert!(m.out.contains("resolved ledger conflict: S#s"), "{}", m.out);
    assert!(work.read("docs/S.md").contains("<<<<<<<"));
    let again = work.repo(&["docs", "merge-main"]);
    assert!(again.out.contains("already in progress"), "{}", again.out);
}

#[test]
fn back_references_are_required() {
    let f = Fixture::new(
        "backrefs",
        &[
            ("docs/A.md", &doc("A", "a", "pkg/**", "About pkg.")),
            ("pkg/Cargo.toml", "[package]\n"),
            ("pkg/src/lib.rs", "//! Pkg.\n"),
        ],
    );
    assert!(
        f.repo(&["docs", "verify", "A#a", "--by", "agent:t", "--note", NOTE])
            .ok()
    );
    let r = f.repo(&["docs", "check"]);
    assert!(
        !r.ok()
            && r.out.contains(
                "package root has no `//! Docs:` line; sections covering this package: A#a"
            ),
        "{}",
        r.out
    );
    f.write("pkg/src/lib.rs", "//! Pkg.\n//!\n//! Docs: A#a, A#gone\n");
    f.git(&["add", "-A"]);
    assert!(
        f.repo(&["docs", "verify", "A#a", "--by", "agent:t", "--note", NOTE])
            .ok()
    );
    let r = f.repo(&["docs", "check"]);
    assert!(!r.ok() && r.out.contains("no anchor `gone`"), "{}", r.out);
    f.write("pkg/src/lib.rs", "//! Pkg.\n//!\n//! Docs: A#a\n");
    assert!(
        f.repo(&["docs", "verify", "A#a", "--by", "agent:t", "--note", NOTE])
            .ok()
    );
    let r = f.repo(&["docs", "check"]);
    assert!(r.ok(), "{}", r.out);
}
