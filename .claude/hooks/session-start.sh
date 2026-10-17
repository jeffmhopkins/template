#!/usr/bin/env bash
# SessionStart hook: prepares cloud sessions to run the stack's checks and the repo tool,
# then prints a short orientation. It never fails the session.
set -uo pipefail

cd "${CLAUDE_PROJECT_DIR:-.}" 2>/dev/null || exit 0

if [ "${CLAUDE_CODE_REMOTE:-}" = "true" ]; then
  if command -v rustup >/dev/null 2>&1; then
    # Installs the toolchain rust-toolchain.toml pins, if the repo has one.
    rustup show active-toolchain >/dev/null 2>&1 || rustup toolchain install >/dev/null 2>&1 || true
    for component in rustfmt clippy; do
      if ! rustup component list --installed 2>/dev/null | grep -q "^$component"; then
        rustup component add "$component" >/dev/null 2>&1 ||
          echo "session-start: could not install $component" >&2
      fi
    done
  fi
  if command -v cargo >/dev/null 2>&1 && [ -f tools/repo/Cargo.toml ]; then
    # Prebuilds the repo tool so the first `scripts/repo` call is fast.
    cargo build --quiet --release --manifest-path tools/repo/Cargo.toml >/dev/null 2>&1 ||
      echo "session-start: prebuilding tools/repo failed; scripts/repo builds it on first use" >&2
  fi
fi

cat <<'EOF'
Orientation:
- Read CLAUDE.md first. Your role's brief is in docs/agents/.
- Before touching a file, run `scripts/repo docs for <path>` and read what it lists.
- Before pushing, run `scripts/repo docs check`.
EOF

exit 0
