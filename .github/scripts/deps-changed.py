"""deps-changed: whether a PR changes what ci.yml's `deps` job checks.

Run from the repo root on a PR's merge commit, whose first parent is the base. Writes
`run=true` or `run=false` to $GITHUB_OUTPUT (stdout without it). The paths are the stack
pack's `[ci].deps_paths` and setup action, plus the repo tool's manifests, every pack.toml,
the cargo-deny action and ci.yml. Editing this file is a maintainer-only change.
"""

import os
import subprocess
import sys
import tomllib


def main() -> int:
    with open("repo.toml", "rb") as f:
        pack_name = tomllib.load(f)["stack"]["pack"]
    with open(f"stacks/{pack_name}/pack.toml", "rb") as f:
        ci = tomllib.load(f).get("ci", {})
    globs = list(ci.get("deps_paths", []))
    if ci.get("setup"):
        globs.append(ci["setup"].rstrip("/") + "/**")
    globs += [
        "tools/repo/Cargo.toml",
        "tools/repo/Cargo.lock",
        "stacks/*/pack.toml",
        ".github/actions/cargo-deny/**",
        ".github/workflows/ci.yml",
        ".github/scripts/deps-changed.py",
    ]
    specs = [f":(glob){g}" for g in globs]
    diff = subprocess.run(["git", "diff", "--quiet", "HEAD^1", "HEAD", "--", *specs])
    if diff.returncode not in (0, 1):
        print("git diff failed; deps runs.", file=sys.stderr)
    run = "false" if diff.returncode == 0 else "true"
    line = f"run={run}\n"
    out = os.environ.get("GITHUB_OUTPUT")
    if out:
        with open(out, "a") as f:
            f.write(line)
    print(line, end="")
    if run == "false":
        print("Nothing the deps job checks changed: it passes without running.")
    return 0


if __name__ == "__main__":
    sys.exit(main())
