---
name: steward
description: PR conventions for this repo. Read before acting on a CI failure, a review comment, a merge conflict or any other event on a pull request you opened. Covers merging main (never rebasing), restamping docs after a merge, merge-only rounds, answering review findings, and auto-merge.
---
<!-- @doc
id = "SKILL-STEWARD"
title = "Steward skill"
kind = "guide"
status = "active"
review_every = "90d"
-->

# Steward

How to drive your own PR from its first event to its merge. The full loop is in
[CLAUDE.md](../../../CLAUDE.md#pr-loop) and your role's brief; this skill is what to do on
each kind of event.

<a id="every-event"></a>
## On every event
<!-- @section
depends_on = ["AGENT-BUILDER#on-every-wake"]
-->

1. Check `ops/STOP` (stop if present) and `ops/mode.md` (waves, main's health).
2. If `main` is red, read its pinned "main is red" issue before investigating any failure.
   Never push a fix for a failure that issue already covers.
3. Read everything new on the PR: every comment, every review thread, every check run and
   every issue filed against it.
4. Batch what you change into one push. Before it, run the stack's checks
   (`scripts/repo stack run fmt`, `lint`, `test`), `scripts/repo docs check` and
   `scripts/repo governance --base origin/main`.

<a id="ci"></a>
## CI failures
<!-- @section
depends_on = ["RULES#R-GIT-003"]
-->

- Reproduce the failing job locally with `scripts/repo stack run <job>`, the matching
  `scripts/repo` command for `docs` (`config check` and `docs check`), `governance`,
  `pr-title` and `lane-guard`, or `node --test '.github/tests/*.test.mjs'` for `gate-test`.
- Fix the cause in the PR's own files. Never weaken a check, add a suppression without a
  reason, or edit lint or CI config to get green; any of those is a declared governance
  change.
- A failure that isn't yours (main is red, a flake, an outage): say so on the PR in one line
  with the link, and push nothing for it.
- `review-gate` stays pending until the reviewer posts a marker for the current head, and
  fails on a `fail` or `escalate` verdict. Pending is waiting, not broken.

<a id="merge-main"></a>
## Bringing main in
<!-- @section
depends_on = ["DOCS#merge", "DOCS#notes"]
-->

- **Merge, never rebase**, and never force-push. Use `scripts/repo docs merge-main`, or
  `scripts/repo docs merge-main --from wave/<name>` for a PR into a wave.
- It resolves ledger and `@generated` conflicts itself. Resolve every other conflict by
  hand, then finish the merge commit.
- It prints the sections the merge made stale, each with the notes this branch wrote on it.
  Re-read each one against the merged tree and verify it with a **fresh note**: what the
  earlier note said about this PR's change, plus what the merge changed. Never paste the
  old note back. How to verify is in the `verify-docs` skill.
- Merge when the integrator sends `MERGE-MAIN`, when GitHub reports a conflict, or when a
  required check needs the newer base. Otherwise leave the base alone.

<a id="merge-only"></a>
## Merge-only rounds
<!-- @section
depends_on = ["REVIEW#rounds"]
-->

After review has passed, a merge of the base gets its own round:

1. Push the merge commit and its restamps alone, with no other change in the push.
2. Comment on the PR in one line that the round is merge-only: the base merged, the
   conflicts resolved, and the sections restamped.
3. A real change found during the merge goes in a separate, later push, and that push gets a
   full round.

<a id="findings"></a>
## Review findings
<!-- @section
depends_on = ["RULES#R-GOV-004"]
-->

- Read the whole round comment. Near its end, before the human line and the footer, is
  the verdict marker for one head SHA.
- **Every `B<n>`** gets a reply: the commit that fixes it, or a concrete rebuttal (a code
  path, test or doc showing the finding is wrong).
- **`N<n>`**: fix it in the same push when it is cheap and inside the PR's own paths: a
  rename, a missing test case, a doc sentence. Defer anything else in one line each.
  Pre-existing problems become issues, not PR changes.
- Never widen the PR beyond its row to answer a finding; send the planner
  `BLOCKED <item>: needs <path>` instead.
- A `needs-human` label means the maintainer decides that point. Stop arguing it and keep
  working on everything else.

<a id="merging"></a>
## Auto-merge
<!-- @section
depends_on = ["CLAUDE#pr-loop"]
-->

- Turn on auto-merge (squash) when the PR is ready: the GitHub MCP tool
  `enable_pr_auto_merge` with `mergeMethod: SQUASH`, or `gh pr merge <n> --auto --squash`.
  If enabling fails, say so on the PR.
- A PR that changes a serial lane keeps auto-merge off until it holds its slot.
- The PR merges itself when every required check, `review-gate` included, is green.

<a id="never"></a>
## Never
<!-- @section
depends_on = ["CLAUDE#limits"]
-->

- Post a review marker or verdict, approve through GitHub, or merge by hand.
- Rebase, force-push, or push to another row's branch.
- Apply `governance-approved` or `review-override` without the matching section in the PR
  description and without examining what the check flagged.
- Change repo settings or branch protection.
- Edit `docs/verified.toml` by hand, or verify a section you didn't re-read.
