#!/usr/bin/env bash
# Rewrites the body of the one issue titled TITLE that github-actions opened, creating and
# pinning it when absent and reopening it when closed. Needs GH_TOKEN with issues: write.
# Usage: pinned-issue.sh <title> <body-file>
set -euo pipefail
title=$1
body=$2

# GitHub caps an issue body at 65536 characters.
if [ "$(wc -c < "$body")" -gt 65000 ]; then
  cut="$body.cut"
  head -c 64000 "$body" > "$cut"
  printf '\n\n…cut; the full text is in the run log: %s/%s/actions/runs/%s\n' \
    "$GITHUB_SERVER_URL" "$GITHUB_REPOSITORY" "$GITHUB_RUN_ID" >> "$cut"
  body=$cut
fi

number=$(gh issue list --repo "$GITHUB_REPOSITORY" --state all --author 'app/github-actions' \
  --search "in:title \"$title\"" --limit 50 --json number,title \
  --jq "map(select(.title == \"$title\")) | sort_by(.number) | first | .number // empty")
if [ -z "$number" ]; then
  url=$(gh issue create --repo "$GITHUB_REPOSITORY" --title "$title" --body-file "$body")
  number=${url##*/}
  gh issue pin "$number" --repo "$GITHUB_REPOSITORY" || echo "::warning::could not pin issue #$number"
else
  gh issue edit "$number" --repo "$GITHUB_REPOSITORY" --body-file "$body"
  if [ "$(gh issue view "$number" --repo "$GITHUB_REPOSITORY" --json state --jq .state)" = CLOSED ]; then
    gh issue reopen "$number" --repo "$GITHUB_REPOSITORY"
  fi
fi
echo "$title: #$number"
