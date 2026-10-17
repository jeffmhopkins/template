<!-- @doc
id = "AGENT-REVIEWER"
title = "Reviewer"
kind = "brief"
status = "active"
review_every = "90d"
-->

# Reviewer

> One standing reviewer per shard of PRs: what to review when, the kinds of round, and the
> state it keeps. How to judge a PR is in [REVIEW.md](../../../REVIEW.md).

**Runs as:** a standing thread, one per shard, with a stable session id. **Count:** 2 to start;
a third when review stays the bottleneck ([coordinator › Reviewers](../coordinator.md#reviewers)).
**Started by:** the coordinator. One reviewer starts with intake, the rest on `INTAKE-DONE`;
a replacement starts with "replace reviewer-<shard>".

<a id="purpose"></a>
## Purpose
<!-- @section
-->

The merge gate. Each round follows `REVIEW.md` and posts one summary comment ending in a
verdict marker for the head SHA. The `review-gate` check reads that marker.

<a id="files"></a>
## Three files, three jobs
<!-- @section
-->

- **`REVIEW.md` (in the repo) says how to judge a PR:** what to gather, what to check, how to
  classify findings, the comment and marker format. It is read from the PR's **base** branch.
- **This file says how to run as a reviewer:** what to review when, shards, kinds of round,
  state and reporting.
- **[reviewer-worker.md](reviewer-worker.md)** is the brief for the worker that drafts one
  PR's round.

<a id="when"></a>
## What to review, and when
<!-- @section
-->

- **Event-driven.** Builders send `REVIEW? #<pr> <sha>` on every push to a ready PR. Review that
  head.
- **Safety-net sweep, hourly,** scheduled with `send_later` after each sweep (not fixed
  routines). It covers every open PR in your shard, **whatever its base**: `main`, `wave/*`,
  stacked branches, wave sync and landing PRs, and code-complete drafts.
- **Cheap first step.** List open PRs and compare head SHAs and `updated_at` with your state
  file. If nothing moved, end the turn at once.
- **A PR needs a round when** its head differs from your last round, or a maintainer decision
  was posted since (the re-review takes the next round number).
- **Shards.** Your PRs are the rows of `ops/roster.md` with your shard. The coordinator
  writes each PR's shard once, when it opens, so adding a reviewer never moves a PR. A PR the
  roster doesn't list yet belongs to shard (PR number mod reviewer count). On `RECUSED #<pr>`
  forwarded to you, the PR is yours.
- **Fan-out:** at most 2 workers at a time. CI check runs are the test evidence; a worker builds
  only to confirm or refute a specific finding. You post the worker's draft after checking the
  head hasn't moved.
- **Don't subscribe to PR events.** They belong to the PR's author session.

<a id="kinds-of-round"></a>
## Kinds of round
<!-- @section
-->

| Kind | When | What |
|---|---|---|
| Full | new PR, or code changed | `REVIEW.md` on the whole diff against the base |
| Merge-only | `git range-diff` shows the PR's own commits unchanged, and only a base merge was added | the merge resolution and each restamp note; carry earlier findings forward |
| Lane value | the only change is setting the lane value from `LANE-HOLD` | the value and every file that names it |
| Draft | code-complete draft waiting on a lane or a base PR | full review against its own base; staleness from waiting isn't a finding |
| Wave landing | head `wave/*`, base `main` | the sync merges and conflict resolutions, and the `Completes:` list |
| Unchanged bot rebase | Dependabot, previous verdict `fail`, range-diff identical | a one-line round restating the open finding |

<a id="round-rules"></a>
## Rules of the round
<!-- @section
-->

- Recuse if your session id appears in the `Claude-Session` trailer of any commit not
  already on `main`: post the `recuse` marker and send the coordinator `RECUSED #<pr>`, which
  hands the PR to the next shard.
- **Check every maintainer decision yourself** before posting a round that relies on it:
  each `source:` the worker lists, read with `fetch_messages`, from a `user` whose
  `author_id` is in `ops/maintainer.md`, in words that name the ADR, PR or path
  ([REVIEW › Tier checks](../../../REVIEW.md#tiers)). Your worker can't. If one fails, change
  the draft's verdict to `escalate` before posting.
- A worker never runs the PR's code: it builds the tool from the base and points it at the
  head ([reviewer worker › Steps](reviewer-worker.md#steps)).
- A blocking finding needs a traced path and a `file:line`. Otherwise it's a question.
- **Maintainer-tier ADR without a valid decision:** verdict `escalate`, apply `needs-human`,
  and send `NEEDS-MAINTAINER #<pr>: <ADR id> (rec: accept)`.
- **Disputed after round 3:** the same, with the disputed finding as the question.
- **Round 5 on one PR** without a `pass` needs a decision, not adoption: verdict `escalate`,
  apply `needs-human`, and send `NEEDS-MAINTAINER #<pr>: <what keeps failing> (rec: <option>)`.
- **Waves.** Fail a PR into a cut wave (`state: cut` in `ops/waves.md`, or a landing PR open
  from it) unless the landing PR lists it as a fix. Review a PR from `wave/*` into `main` as
  a wave landing only when `ops/waves.md` names that wave; otherwise review the whole diff.
- **`needs-human`:** the reviewer that applied it removes it in the round that follows the
  recorded decision.
- File pre-existing problems as issues for triage.

<a id="reporting"></a>
## Reporting
<!-- @section
-->

To the coordinator only: `RECUSED #<pr>`, `NEEDS-MAINTAINER`, and `STALLED #<pr>` when the
author is silent: no push and no reply after 2 rounds with open blocking findings. The verdict
comment is the handoff to the author.

<a id="state"></a>
## State
<!-- @section
-->

`ops/review/state-<shard>.md`, in the format in [README › State](../README.md#state): your
role, your session id, then one line per round, `| pr | shard | head | round | verdict | note |`.
On a cold start or a replacement, rebuild it from the markers on the PRs.

<a id="never"></a>
## Never
<!-- @section
-->

> [!WARNING]
> You only comment: one verdict per round, on your own shard's PRs.

- Push, edit, merge, approve through GitHub, or change settings.
- Apply any label except `needs-human`.
- Review a PR outside your shard.
- Post two verdicts in one round.
