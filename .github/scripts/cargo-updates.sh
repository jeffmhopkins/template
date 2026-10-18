#!/usr/bin/env bash
# cargo-updates: prints, as Markdown, the compatible updates each Cargo manifest's lockfile
# would take, and the ones a version requirement holds back. Used by the weekly sweep: for the
# repo tool from sweep.yml, and for a Rust project from the Rust stack pack's [[sweep]].
# Usage: cargo-updates.sh <Cargo.toml>...  It never fails.
# Editing this file is a maintainer-only change.
set -uo pipefail

tmp=$(mktemp)
trap 'rm -f "$tmp"' EXIT
any=false
for manifest in "$@"; do
  [ -f "$manifest" ] || continue
  any=true
  cargo update --dry-run --verbose --manifest-path "$manifest" > "$tmp" 2>&1 || true
  changes=$(grep -E '^ +(Updating|Adding|Removing|Downgrading) [A-Za-z0-9_-]+ v' "$tmp" | sed 's/^ *//' || true)
  held=$(grep -E '^ +Unchanged [A-Za-z0-9_-]+ v.*\(available: ' "$tmp" | sed 's/^ *Unchanged //' || true)
  echo "**\`$manifest\`**"
  echo
  if [ -n "$changes" ]; then
    echo "Compatible updates the lockfile would take (Dependabot proposes them):"
    echo; echo '```text'; printf '%s\n' "$changes" | head -c 10000; echo '```'
  else
    echo "The lockfile takes every compatible update already."
  fi
  if [ -n "$held" ]; then
    echo; echo "Held back by a version requirement:"
    echo; echo '```text'; printf '%s\n' "$held" | head -c 10000; echo '```'
  fi
  echo
done
$any || echo "No Cargo manifests."
exit 0
