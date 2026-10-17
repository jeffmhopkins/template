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
  trailer is ignored, and the gate fails while a non-merge commit lacks that trailer
  (Dependabot's excepted). It runs from the default branch and never checks out PR code.
  Its decision function is tested in CI (`gate-test`) against the workflow file itself.
- **Merging:** PRs merge only through GitHub auto-merge (squash) once every required check
  is green. No agent merges by hand.
- **Disputes:** a blocking finding still disputed after round 3 is escalated with
  `needs-human`. The maintainer resolves it, by a fix or by `review-override` with a
  justification that links their message.
- **Governance:** the changes listed in [DOCS › Governance changes](../DOCS.md#governance)
  need the `governance-approved` label and a "Governance change" section. The `governance`
  check builds its tool from the base commit. Agents may apply the label after examining
  what was flagged. The weekly sweep lists every PR merged with it or with
  `review-override`.
- **Maintainer-only paths:** changes to the gate workflow, `REVIEW.md`, this ADR,
  [ADR-0003](0003-decision-tiers.md) and the project instructions always need the
  maintainer's own words.

<a id="consequences"></a>
## Consequences

### Positive
- An outsider's comment can't pass a PR, and a parallel `pass` can't hide a `fail`.

### Negative
- Every push needs a new review round, since the gate counts only markers for the current
  head SHA.
- The gate is a process control against agents holding the maintainer's token: they post
  as the configured login. The session list, the trailer check and the weekly audit make a
  forged verdict visible, not impossible. A separate reviewer identity would close this.
- `REVIEW.md`, the gate workflow and the governance tool are the trust base, and are
  protected.
