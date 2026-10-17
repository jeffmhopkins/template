#!/usr/bin/env bash
# Posts an after-review job's state as a commit status named after its check (CONTEXT) on SHA,
# so a run outside the PR's own events counts on the PR (after-review.yml).
# RESULT is `pending` or the tested job's result (success, failure, cancelled, skipped).
# It runs from the default branch, in a job that runs nothing from the PR.
set -euo pipefail
case "$RESULT" in
  pending | success) state="$RESULT" ;;
  cancelled) state=error ;;
  *) state=failure ;;
esac
gh api --silent "repos/$GITHUB_REPOSITORY/statuses/${SHA:-$GITHUB_SHA}" \
  -f state="$state" -f context="$CONTEXT" -f description="after-review: $RESULT" \
  -f target_url="$GITHUB_SERVER_URL/$GITHUB_REPOSITORY/actions/runs/$GITHUB_RUN_ID"
