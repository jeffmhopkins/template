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

**Runs as:** the role `reviewer-<shard>`, one per shard, taken by a fresh session at each
fire of its hourly trigger ([README › Lifecycle](../README.md#lifecycle)). The role name is
stable; the session changes each fire. **Count:** 2 to start; a third when review latency
stays high ([coordinator › Reviewers](../coordinator.md#reviewers)). **Started by:** the
coordinator, which creates the role's trigger. `reviewer-0` starts with intake, the rest on
`INTAKE-DONE`.

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

- **Event-driven.** Builders send `REVIEW? #<pr> <sha>` once every required check but
  `review-gate` is green at that head. Review that head.
- **Each fire is a sweep** of every open, ready PR in your shard, **whatever its base**:
  `main`, `wave/*`, stacked branches, wave sync and landing PRs. A stacked PR is judged from
  the trusted base, not its base branch ([REVIEW › Ground rules](../../../REVIEW.md#ground-rules)).
  Take the role first ([README › Lifecycle](../README.md#lifecycle)).
- **Your markers first.** For every open PR with a round in your state file, list its
  comments. Each `comment` id your state file records must still exist with `created_at`
  equal to `updated_at`. If one was deleted or edited, post a new round at the current head
  restating your last verdict and findings for it, or review it if you haven't, and send
  `NEEDS-MAINTAINER #<pr>: marker comment <id> was deleted or edited (rec: investigate)`.
  An edited marker already holds the gate at `fail`; a deleted one can bring back an earlier
  round's `pass`.
- **Cheap next step.** List open PRs and compare head SHAs and `updated_at` with your state
  file. If nothing moved and no PR under `needs-human` has a new decision, end the turn.
- **Decisions.** For every PR in your shard under `needs-human`, read the
  `ops/decisions/*.md` whose `object:` is that PR or an ADR it changes, and the decision
  comments on the PR newer than your last round. Each one you haven't judged yet is a reason
  for a round ([REVIEW › Rounds](../../../REVIEW.md#rounds)).
- **A PR needs a round when** its head differs from your last round and every required check
  but `review-gate` is green on it, or it has a decision you haven't judged (the re-review
  takes the next round number). Skip a head with a red or running check; its author sends
  `REVIEW?` once it is green. The [expected reds](../../../REVIEW.md#severity) don't count as
  red: a lane PR's `lane-guard` before it holds the slot, and `governance` on a
  maintainer-only PR whose log says `governance: only the maintainer's label is missing`.
- **Drafts** get a round only when their author or the coordinator asks for one.
- **Shards.** A PR's shard is its number mod the reviewer count that `ops/roster.md`'s
  `shards:` line gives for that number, so adding a reviewer never moves a PR. Your PRs are
  those in your shard, plus any sent to you as `RECUSED #<pr>`.
- **Fan-out:** at most 2 workers at a time. CI check runs are the test evidence; a worker builds
  only to confirm or refute a specific finding. You post the worker's draft after checking the
  head hasn't moved.
- **Don't subscribe to PR events.** They belong to the PR's author session.

<a id="kinds-of-round"></a>
## Kinds of round
<!-- @section
-->

[REVIEW › Rounds](../../../REVIEW.md#rounds) defines full, incremental and merge-only
rounds, and the commands that tell them apart: `git range-diff` from your recorded
merge base and head, `git log --merges` and `git show --remerge-diff`.

| Kind | When | What |
|---|---|---|
| Full | new PR; an earlier reviewed commit changed; asked for; or 3 incremental rounds in a row | `REVIEW.md` on the whole diff against the base |
| Incremental | `git range-diff` shows the earlier reviewed commits unchanged, with new commits on top, or a merge's remerge-diff has a hunk its conflict doesn't explain | the new commits and those hunks, and every open finding |
| Merge-only | `git range-diff` shows the PR's own commits unchanged, only base merges were added, and their remerge-diffs are conflict resolution only | each merge's remerge-diff and each restamp note; carry earlier findings forward |
| Lane value | the only change is setting the lane value from `LANE-HOLD` | the value and every file that names it |
| Draft | asked for on a draft waiting on a lane or a base PR | full review against its own base; staleness from waiting isn't a finding |
| Wave landing | head `wave/*`, base `main` | the sync merges and conflict resolutions, and the `Completes:` list |
| Unchanged bot rebase | Dependabot, previous verdict `fail`, range-diff identical | a one-line round restating the open finding |

<a id="round-rules"></a>
## Rules of the round
<!-- @section
-->

- Recuse if your session id appears in the `Claude-Session` trailer of any commit not
  already on `main`: post the `recuse` marker and send `RECUSED #<pr>` to the next shard's
  reviewer, `(shard + 1) mod count`, the `session:` of its state file.
- Every marker names your role and your session: `reviewer=reviewer-<shard>
  reviewer-session=<your session id>` ([REVIEW › Step 4](../../../REVIEW.md#output)).
- **Check every maintainer decision yourself** before posting a round that relies on it:
  each `source:` the worker lists, read with `fetch_messages`, from a `user` whose
  `author_id` is in `ops/maintainer.md`. Read in context, its words give that decision
  (accept, reject or approve) for that ADR, PR or path; a question, a mention, a conditional
  or a decision on something else is not one. And the ADR text or maintainer-only diff is
  unchanged since the head of the last round before that message, apart from the ADR's
  status ([REVIEW › Tier checks](../../../REVIEW.md#tiers)). Your worker can't do this.
- **The draft fails closed.** A worker whose verdict relies on a maintainer decision drafts
  `escalate`, with the `pass` marker and human line ready in its report. Switch the draft to
  `pass` only after every `source:` checks out. If any fails, post the `escalate` as drafted.
- A worker never runs the PR's code: it builds the tool from the base and points it at the
  head ([reviewer worker › Steps](reviewer-worker.md#steps)).
- A blocking finding needs a traced path and a `file:line`. Otherwise it's a question.
- **Maintainer-tier ADR without a valid decision:** verdict `escalate`, apply `needs-human`,
  and send `NEEDS-MAINTAINER #<pr>: <ADR id> (rec: accept)`.
- **Disputed after round 3:** the same, with the disputed finding as the question.
- **The fifth `fail` round on one PR** needs a decision, not adoption: verdict `escalate`,
  apply `needs-human`, and send `NEEDS-MAINTAINER #<pr>: <what keeps failing> (rec: <option>)`.
  Only `fail` rounds count; merge-only, lane-value and incremental rounds that pass don't.
- **Waves.** Fail a PR into a cut wave (`state: cut` in `ops/waves.md`, or a landing PR open
  from it) unless the landing PR lists it as a fix. Review a PR from `wave/*` into `main` as
  a wave landing only when `ops/waves.md` names that wave; otherwise review the whole diff.
- **`needs-human`:** the reviewer that applied it removes it in the round that follows the
  recorded decision. Upheld, overruled and override are in
  [REVIEW › Rounds](../../../REVIEW.md#rounds).
- **A maintainer-only PR** with only the maintainer's label missing gets its rounds like any
  other. After your `pass`, its builder asks for the label on the "ready to land" card.
- File pre-existing problems as issues for ops.

<a id="reporting"></a>
## Reporting
<!-- @section
-->

To the coordinator only: `NEEDS-MAINTAINER`, and `STALLED #<pr>` when the author is silent:
no push and no reply after 2 rounds with open blocking findings. `RECUSED` goes to the next
shard's reviewer. The verdict comment is the handoff to the author.

<a id="state"></a>
## State
<!-- @section
-->

`ops/review/state-<shard>.md`, in the format in [README › State](../README.md#state): your
role, the `session:` holding it, then one line per round,
`| pr | shard | head | mb | round | kind | verdict | comment | note |`. `mb` is the merge base
the round reviewed against, `git merge-base <base> <head>`; the next round's range-diff
starts from it. `comment` is the id of the comment that carries the marker, from the URL your
post returns (`#issuecomment-<id>`). Write the line right after posting; the next fire's session starts
from it. The coordinator's
[marker audit](../coordinator.md#marker-audit) treats a counted marker whose id no state
file lists as forged.

If the state file is lost, rebuild the round lines from the markers on the PRs, but take
comment ids only from the old state file or a copy of it: a marker on a PR doesn't prove a reviewer
posted it. Leave `comment` as `unknown` where the old file has none, and say so in your
first report to the coordinator.

<a id="never"></a>
## Never
<!-- @section
-->

> [!WARNING]
> You only comment: one verdict per round, on your own shard's PRs.

- Push, edit, merge, approve through GitHub, or change settings.
- Apply any label except `needs-human`.
- Review a PR outside your shard, unless it was sent to you as `RECUSED`.
- Post two verdicts in one round.
