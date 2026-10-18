//! `repo`: the repository's checking tool, configured by `repo.toml` and the stack pack.
//!
//! Docs: DOCS#tool, STACKS#pack-toml

#![forbid(unsafe_code)]

mod args;
mod config;
mod docs;
mod git;
mod glob;
mod governance;
mod heads;
mod lanes;
mod plan;
mod pr_title;
mod progress;
mod stack;

use std::process::ExitCode;

use anyhow::{Result, bail};

/// What a command found: `Ok(true)` when clean, `Ok(false)` on findings.
type Outcome = Result<bool>;

const USAGE: &str = "\
usage: repo [--root <dir>] <command> [args]

  --root <dir>                              run on the repository at <dir>, not the current
                                            directory's (a reviewer points a base build at a
                                            PR head's worktree)

commands:
  config check                              schema-check repo.toml, lanes.toml, the stack pack, CI job names;
                                            compare the copies of repo.toml values with it
  docs check [--draft]                      metadata, references, ledger freshness, generated drift, rule enforcers
  docs verify <DOC#anchor> --by <who> --note <text>
                                            restamp a section after a re-read
  docs for <path>                           the sections, rules and ADRs that govern a file
  docs generate                             rewrite @generated blocks
  docs log <DOC[#anchor]> [--rev <rev>]     a doc's or section's history, from git
  docs review <DOC> --by <who> --note <text>
                                            stamp a doc's time-based review
  docs merge-main [--from <branch>]         merge origin/<branch> (default main), resolve ledger, @generated and lockfile conflicts
  docs due                                  docs past review_every, external URLs past recheck_every
  governance --base <rev> [--head <rev>] [--pr-body <file>] [--labels a,b] [--author <login>]
             [--into <branch>] [--main <rev>]
                                            list governance changes; enforce label and section;
                                            into a wave/* branch, skip files as on main and
                                            fail on protected paths
  pr-title <title>                          Conventional Commit check
  lane-guard --base <rev> --head <rev> --pr <n> [--wave --head-ref <branch> --base-ref <branch>]
                                            check a PR's serial-lane claims and lane files
  progress [--format markdown|json]         status derived from trailers on HEAD's history
  progress --trailers <range>               the Completes/Part-of/Verified-on-hardware lines in <range>
  plan check <launch.md> <roster.md> [--row <id>]
                                            no `owns` overlap between running rows
  heads-changed --state <file>              print `changed` or `unchanged` for origin's refs
  stack run <job>                           run the stack pack's command for a CI job
  stack setup <dir>                         put the pack's CI setup action, or a no-op one, at <dir>
  stack sweep                               run the pack's sweep items; print Markdown; exits 0";

fn run(args: &[String]) -> Outcome {
    let args = match args {
        [flag, dir, rest @ ..] if flag == "--root" => {
            git::set_root(std::path::Path::new(dir))?;
            rest
        }
        _ => args,
    };
    let rest = |n: usize| &args[n.min(args.len())..];
    match (
        args.first().map(String::as_str),
        args.get(1).map(String::as_str),
    ) {
        (Some("config"), Some("check")) => config::run_check(rest(2)),
        (Some("docs"), Some(sub)) => docs::run(sub, rest(2)),
        (Some("governance"), _) => governance::run(rest(1)),
        (Some("pr-title"), _) => pr_title::run(rest(1)),
        (Some("lane-guard"), _) => lanes::run(rest(1)),
        (Some("progress"), _) => progress::run(rest(1)),
        (Some("plan"), Some("check")) => plan::run(rest(2)),
        (Some("heads-changed"), _) => heads::run(rest(1)),
        (Some("stack"), Some(sub)) => stack::run(sub, rest(2)),
        (Some("help" | "--help" | "-h"), _) => {
            println!("{USAGE}");
            Ok(true)
        }
        _ => bail!("unknown command\n\n{USAGE}"),
    }
}

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    match run(&args) {
        Ok(true) => ExitCode::SUCCESS,
        Ok(false) => ExitCode::FAILURE,
        Err(e) => {
            eprintln!("repo: {e:#}");
            ExitCode::from(2)
        }
    }
}
