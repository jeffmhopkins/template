<!-- @doc
id = "ADR-0004"
title = "CI: drafts skip, platform jobs after review, SHA pins"
kind = "adr"
status = "accepted"
tier = "agent"
-->

# ADR-0004: CI: drafts skip, platform jobs after review, SHA pins

<!-- @generated begin adr-status -->
> **Status:** accepted
<!-- @generated end adr-status -->

<a id="context"></a>
## Context
Many agent PRs are open at once, and each push starts CI. Runner time is shared by all of
them. Most pushes answer review findings and start a new review round anyway. Third-party
actions run with the repository's token.

<a id="decision"></a>
## Decision
1. **Drafts skip.** Every job in `ci.yml` is skipped on a draft PR and runs once the PR is
   marked ready.
2. **Fast checks on every push.** `ci.yml` runs `fmt`, `lint`, `test`, `deps` and `docs`
   from the stack pack, and `pr-title`, `gate-test` and `lane-guard`. `governance`,
   `lane-guard` and `pr-title` run on `pull_request_target`, build the tool from the base
   commit and read the head only as git objects. The tool's build cache is written only on
   pushes to `main`.
3. **Expensive jobs after review.** `after-review.yml` runs other operating systems and
   end-to-end jobs on `main`, and on a PR once `review-gate` passes its head SHA. Each job
   posts a commit status under its check name. No check name appears in both workflows.
   It stays dormant until a project needs it.
4. **Pins.** Every third-party action is referenced by full commit SHA with its release in a
   trailing comment (`@<sha> # vX.Y.Z`). Dependabot proposes grouped weekly updates.
5. **Pre-approved re-pins.** A Dependabot PR whose diff only re-pins `uses:` references to
   new commit SHAs needs no `governance-approved` label. The reviewer confirms the diff is
   limited to that.
6. **Rulesets** pin every required check to GitHub Actions. The required list lives in
   `repo.toml`, and `scripts/repo config check` compares it with the workflow job names.

<a id="consequences"></a>
## Consequences
- Runner time per PR drops to one fast run per ready push, and one expensive run per
  reviewed SHA.
- A failure only another platform shows surfaces after review; its fix is a new push and a
  new review round.
- An action's tag moving to new code can't change what CI runs.
