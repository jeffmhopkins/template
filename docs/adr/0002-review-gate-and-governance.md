<!-- @doc
id = "ADR-0002"
title = "Review gate and governance"
kind = "adr"
status = "accepted"
tier = "maintainer"
-->

# ADR-0002: Review gate and governance

<!-- @generated begin adr-status -->
> **Status:** accepted
<!-- @generated end adr-status -->

> A reviewer agent's verdict for the exact head SHA is the merge gate, and a change that
> could loosen a check needs a label and a justification.

<a id="context"></a>
## Context
Every PR is written by an agent and pushed from one GitHub identity. GitHub never lets a
PR's author approve it, so native required reviews don't apply. An agent under pressure
could also pass the checks by weakening them.

<a id="decision"></a>
## Decision
- **The review instructions live in the repo** as [REVIEW.md](../../REVIEW.md), read from the
  PR's base branch. They pull the applicable rules from the repo and define the finding
  format and the verdict marker.
- **Verdict marker:** each review round ends with
  `<!-- review v1 verdict=<verdict> sha=<head> round=<n> reviewer-session=<id> -->`.
- **`review-gate`** is a required check. It counts only markers for the current head SHA,
  posted by the configured login, from a session listed in the `REVIEWER_SESSIONS`
  repository variable, and not edited. The highest round decides, and within it any other
  verdict beats `pass`. A marker from a session named in a commit's `Claude-Session:`
  trailer is ignored, and the gate fails while a non-merge commit lacks that trailer.
  Commits already on the default branch, and Dependabot's GitHub-signed commits, are
  exempt. The PR template ends with the trailer, so squash commits carry it into a wave and
  its landing PR. It runs from the default branch and never checks out PR code.
  Its decision function is tested in CI (`gate-test`) against the workflow file itself.
- **Merging:** PRs merge only through GitHub auto-merge (squash) once every required check
  is green. No agent merges by hand.
- **Disputes:** a blocking finding still disputed after round 3 is escalated with
  `needs-human`. The maintainer resolves it, by a fix or by `review-override` with a
  justification that links their message. Only the maintainer applies `review-override`.
- **Governance:** the changes listed in [DOCS › Governance changes](../DOCS.md#governance)
  need the `governance-approved` label and a "Governance change" section. The `governance`
  check builds its tool from the base commit. Agents may apply the label after examining
  what was flagged, except on maintainer-only paths. The weekly sweep lists every PR merged
  with it or with `review-override`.
- **Labels follow the head:** a push removes `governance-approved` and `review-override`, so
  each counts only for the head it was applied to.
- **Maintainer-only paths:** changes to the workflows and composite actions, `repo.toml`, the
  tool's source, `REVIEW.md`, the reviewer briefs, this ADR,
  [ADR-0003](0003-decision-tiers.md) and the project instructions always need the
  maintainer's own words, and only the maintainer applies the label on them. The list is in
  `repo.toml` and includes `repo.toml` and the code that reads it, so it protects itself.
- **Maintainer decisions are checked at their source:** every decision comment names the
  maintainer's message by id, and the reviewer reads it with `fetch_messages`, comparing its
  author with the maintainer's user id recorded on day 0 ([ADR-0003](0003-decision-tiers.md)).

<a id="consequences"></a>
## Consequences

### Positive
- An outsider's comment can't pass a PR, and a parallel `pass` can't hide a `fail`.

### Negative
- Every push needs a new review round, since the gate counts only markers for the current
  head SHA.
- Agents share the maintainer's GitHub login. A label, comment or `/accept` from that login
  can't be shown to be the maintainer's, so "the maintainer applied the label" proves
  nothing by itself. The rule that agents never apply those labels, the `source:` check
  against the maintainer's project user id, and the weekly audit make misuse visible, not
  impossible.
- A PR that changes a workflow runs that workflow as the GitHub Actions app, which can post
  any commit status or check name, `review-gate` and `governance` included. Making the
  workflows maintainer-only fails the real `governance` check and makes the reviewer
  escalate, but the PR's own workflow can still post a passing value. Only a check posted by
  an identity no PR controls closes this.
- Recusal relies on each session writing its own id in its `Claude-Session:` trailers.
- [ADR-0006](0006-separate-agent-identities.md) proposes separate identities for the agents
  and the gate.
- `REVIEW.md`, the gate workflow and the governance tool are the trust base, and are
  protected.
