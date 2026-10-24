<!-- @doc
id = "ADR-0009"
title = "Review gate and governance, v2"
kind = "adr"
status = "proposed"
tier = "maintainer"
-->

# ADR-0009: Review gate and governance, v2

<!-- @generated begin adr-status -->
> **Status:** proposed
<!-- @generated end adr-status -->

> A reviewer agent's verdict for the exact head SHA is the merge gate, and a change that
> could loosen a check needs a label and a justification. It supersedes ADR-0002.

<a id="context"></a>
## Context
Every PR is written by an agent and pushed from one GitHub identity. GitHub never lets a
PR's author approve it, so native required reviews don't apply. An agent under pressure
could also pass the checks by weakening them.

[ADR-0002](0002-review-gate-and-governance.md) set up the gate, but its text no longer
matches it. [ADR-0007](0007-swarm-operating-model.md) replaced its clause on
`REVIEWER_SESSIONS`. The gate and the governance tool have since changed how they treat
tampered markers, override links, stacked PRs and maintainer-only PRs. R-DOC-008 forbids
editing an accepted ADR, so this ADR restates the whole decision as it holds today. It is
the one place that states the review gate and governance.

<a id="decision"></a>
## Decision
- **The review instructions live in the repo** as [REVIEW.md](../../REVIEW.md), read from the
  PR's base branch. They pull the applicable rules from the repo and define the finding
  format and the verdict marker.
- **Verdict marker:** each review round ends with
  `<!-- review v1 verdict=<verdict> sha=<head> round=<n> reviewer=<role> reviewer-session=<id> -->`.
- **`review-gate`** is a required check. It counts only markers for the current head SHA,
  posted by the configured login, from a role listed in the `REVIEWER_ROLES` repository
  variable, and alone in a comment that was not edited. An edited comment, or one with more
  than one marker line, counts as a `fail` in round order, whatever its SHA, role or
  session. The highest round decides, and within it any other verdict beats `pass`. A marker
  from a session named in a commit's `Claude-Session:` trailer is ignored, and the gate
  fails while a non-merge commit lacks that trailer. Commits already on the default branch,
  and Dependabot's GitHub-signed commits, are exempt. The PR template ends with the trailer,
  so squash commits carry it into a wave and its landing PR.
- **Where the gate runs:** from the PR's base branch on PR events (`pull_request_target`)
  and from the default branch on comment events. It never checks out PR code. It fails
  closed unless the PR's base is the default branch or a `wave/*` branch, since a stacked
  PR's base is another PR's unreviewed branch. Its decision function is tested in CI
  (`gate-test`) against the workflow file itself.
- **Merging:** PRs merge only through GitHub auto-merge (squash) once every required check
  is green. No agent merges by hand.
- **Disputes:** a blocking finding still disputed after round 3 is escalated with
  `needs-human`. The maintainer resolves it, by a fix or by `review-override` with a
  justification that links their message. Only the maintainer applies `review-override`.
  The "Review override" section ends at the next heading or the first trailer line.
  `review-gate` checks only that it holds a link other than a session URL. The reviewer
  judges where the link points.
- **Governance:** the changes listed in [DOCS › Governance changes](../DOCS.md#governance)
  need the `governance-approved` label and a "Governance change" section. The `governance`
  check builds its tool from the base commit, and fails closed on the same bases as the
  gate. Agents may apply the label after examining what was flagged, except on
  maintainer-only paths. The weekly sweep lists every PR merged with it or with
  `review-override`.
- **Accepted ADRs:** the `governance` check fails any edit to an accepted ADR beyond
  `status = "superseded"` with a `superseded_by` and its status banner, whatever the labels
  (R-DOC-008).
- **Labels follow the head:** a push removes `governance-approved` and `review-override`, so
  each counts only for the head it was applied to.
- **Maintainer-only paths:** changes to the paths in `[governance].maintainer_only` in
  `repo.toml`, and to the `R-GOV-*` rules, always need the maintainer's own words, and only
  the maintainer applies the label on them. The list includes `repo.toml` and the code that
  reads it, so it protects itself. The maintainer applies the label once review has passed,
  on the "ready to land" card. Until then the `governance` check is red. When the label is
  the only thing missing, the tool prints `governance: only the maintainer's label is
  missing`, and that red is expected, never a finding.
- **Maintainer decisions are checked at their source:** every decision comment names the
  maintainer's message by id, and the reviewer reads it with `fetch_messages`, comparing its
  author with the maintainer's user id recorded on day 0 ([ADR-0003](0003-decision-tiers.md)).

<a id="consequences"></a>
## Consequences

### Positive
- An outsider's comment can't pass a PR, and a parallel `pass` can't hide a `fail`.
- An edited or quoted marker can't bring back a lower round's `pass`, and a stacked PR's
  base can't decide its own checks.
- One ADR states the gate, so REVIEW.md and the workflows have one source to match.

### Negative
- Every push needs a new review round, since the gate counts only markers for the current
  head SHA.
- A maintainer-only PR shows a red `governance` check through review, by design.
- The trust limits of ADR-0002 stand. Agents share the maintainer's login, so a label or
  comment from it proves nothing by itself. A PR that changes a workflow can post any
  status. Any agent can forge a passing marker, since roles and session ids are public;
  the coordinator's daily marker audit detects it after merge
  ([coordinator › Marker audit](../agents/coordinator.md#marker-audit)).
  [ADR-0006](0006-separate-agent-identities.md) proposes the identities that close these.
- `REVIEW.md`, the gate workflow and the governance tool are the trust base, and are
  protected.
