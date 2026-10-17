//! Throwaway git repositories for the integration tests, and a runner for the binary.

#![allow(dead_code, reason = "each test binary uses a different subset")]

use std::path::{Path, PathBuf};
use std::process::Command;

pub const CONFIG: &str = r#"[template]
version = "0.1.0"

[stack]
pack = "rust"

[docs]
tracked = ["*.md", "docs/**/*.md"]
exclude = ["CHANGELOG.md"]
max_cover_files = 12
ledger = "docs/verified.toml"
facts = "docs/facts.toml"
external = "docs/external.toml"
progress = "docs/progress.toml"

[checks]
required = ["fmt", "docs"]

[governance]
protected = [".github/**", "repo.toml", "stacks/**"]
maintainer_only = ["REVIEW.md"]

[labels]
governance = "governance-approved"
override = "review-override"
human = "needs-human"
hardware = "hardware"

[review]
marker = "review v1"

[lanes]
file = "lanes.toml"
"#;

pub const PACK: &str = r#"detect = "Cargo.toml"
[jobs]
fmt = "echo formatting"
[suppressions]
patterns = ['#\[allow\(']
reason_required = true
[backrefs]
roots = ["**/src/lib.rs", "**/src/main.rs"]
prefix = "//! Docs:"
"#;

pub const NOTE: &str = "checked the section text against its covered files";

/// A doc with one section covering `glob`.
pub fn doc(id: &str, anchor: &str, glob: &str, text: &str) -> String {
    format!(
        "<!-- @doc\nid = \"{id}\"\ntitle = \"{id}\"\nkind = \"guide\"\nstatus = \"active\"\n-->\n\n# {id}\n\n<a id=\"{anchor}\"></a>\n## Section\n<!-- @section\ncovers = [\"{glob}\"]\n-->\n{text}\n"
    )
}

/// A temporary git repository, removed on drop.
pub struct Fixture(pub PathBuf);

impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

/// Output of one run of the binary.
#[derive(Debug)]
pub struct Run {
    pub code: i32,
    pub out: String,
}

impl Run {
    pub fn ok(&self) -> bool {
        self.code == 0
    }
}

fn identity(c: &mut Command) -> &mut Command {
    c.env("GIT_AUTHOR_NAME", "t")
        .env("GIT_AUTHOR_EMAIL", "t@t")
        .env("GIT_COMMITTER_NAME", "t")
        .env("GIT_COMMITTER_EMAIL", "t@t")
        .env("GIT_CONFIG_NOSYSTEM", "1")
        .env("GIT_CONFIG_GLOBAL", "/dev/null")
}

impl Fixture {
    /// An empty directory named after the test.
    pub fn dir(name: &str) -> Self {
        let dir = std::env::temp_dir().join(format!("repo-it-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).expect("temp dir is writable");
        Self(dir)
    }

    /// A repository on `main` with the config, the pack, empty registries and `files`,
    /// all committed.
    pub fn new(name: &str, files: &[(&str, &str)]) -> Self {
        let f = Self::dir(name);
        f.git(&["init", "-q", "-b", "main"]);
        f.write("repo.toml", CONFIG);
        f.write("stacks/rust/pack.toml", PACK);
        f.write("docs/external.toml", "");
        f.write("docs/facts.toml", "");
        for (p, t) in files {
            f.write(p, t);
        }
        f.commit("init");
        f
    }

    pub fn path(&self) -> &Path {
        &self.0
    }

    pub fn write(&self, path: &str, text: &str) {
        let p = self.0.join(path);
        std::fs::create_dir_all(p.parent().expect("inside the repo")).expect("writable");
        std::fs::write(p, text).expect("writable");
    }

    pub fn read(&self, path: &str) -> String {
        std::fs::read_to_string(self.0.join(path)).expect("readable")
    }

    /// Runs git; panics unless it succeeds; returns stdout.
    pub fn git(&self, args: &[&str]) -> String {
        let out = identity(&mut Command::new("git"))
            .args(args)
            .current_dir(&self.0)
            .output()
            .expect("git runs");
        assert!(
            out.status.success(),
            "git {args:?} failed: {}",
            String::from_utf8_lossy(&out.stderr)
        );
        String::from_utf8_lossy(&out.stdout).into_owned()
    }

    pub fn commit(&self, msg: &str) {
        self.git(&["add", "-A"]);
        self.git(&["commit", "-q", "--allow-empty", "-m", msg]);
    }

    /// Runs the binary in the repository.
    pub fn repo(&self, args: &[&str]) -> Run {
        let out = identity(&mut Command::new(env!("CARGO_BIN_EXE_repo")))
            .args(args)
            .current_dir(&self.0)
            .output()
            .expect("the binary runs");
        Run {
            code: out.status.code().unwrap_or(-1),
            out: format!(
                "{}{}",
                String::from_utf8_lossy(&out.stdout),
                String::from_utf8_lossy(&out.stderr)
            ),
        }
    }

    /// A clone of this repository, named after the test, with this one as `origin`.
    pub fn clone_as(&self, name: &str) -> Self {
        let c = Self::dir(name);
        std::fs::remove_dir_all(&c.0).expect("removable");
        let ok = identity(&mut Command::new("git"))
            .args(["clone", "-q"])
            .arg(&self.0)
            .arg(&c.0)
            .status()
            .is_ok_and(|s| s.success());
        assert!(ok, "git clone failed");
        c
    }
}
