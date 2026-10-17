//! `repo stack run <job>`: runs the stack pack's command for a CI job from the repository
//! root, or prints `skip: no <detect>` and succeeds when the pack's detect file is absent.

use std::process::Command;

use anyhow::{Context, bail};

use crate::Outcome;
use crate::config::Repo;

pub fn run(args: &[String]) -> Outcome {
    let [job] = args else {
        bail!("usage: repo stack run <job>");
    };
    let repo = Repo::open()?;
    let Some(cmd) = repo.pack.jobs.get(job.as_str()) else {
        let known: Vec<&str> = repo.pack.jobs.keys().map(String::as_str).collect();
        bail!(
            "the stack pack {} has no job `{job}` (it has: {})",
            repo.config.pack_path(),
            known.join(", ")
        );
    };
    if !repo.root.join(&repo.pack.detect).exists() {
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
