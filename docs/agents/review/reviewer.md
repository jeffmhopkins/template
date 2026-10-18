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
  classify findings, the kinds of round, the comment and marker format. It is read from the
  PR's **trusted base** ([REVIEW › Ground rules](../../../REVIEW.md#ground-rules)), and so is
  the [review round template](review-template.md) the comment is built from.
- **This file says how to run as a reviewer:** what to review when, shards, workers, state
  and reporting.
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
  the trusted base, not its base branch, and its checks fail closed until it targets `main`
  or a wave, so it gets a round only as a draft round on request
  ([REVIEW › Ground rules](../../../REVIEW.md#ground-rules)).
  Take the role first ([README › Lifecycle](../README.md#lifecycle)).
- **Unfinished posts.** A state line whose `comment` is `posting` means the last session
  stopped between posting and recording ([State](#state)). Find its marker on the PR: that
  round, your role, and the session its note names. If it is there, record its id and send
  `POSTED` for it. If not, delete the line: the round was never posted.
- **Markers next.** For every open PR in your shard, list its comments and find every comment
  from the configured login that holds a marker line. Each must be one a reviewer recorded:
  its id is in the `comment` column of a state file or in the coordinator's `## Posted`
  ([README › State](../README.md#state)). And each id your state file records must still
  exist, with `created_at` equal to `updated_at`. When a marker no reviewer recorded turns
  up, or one of yours was deleted or edited:
  - post a new round at the current head, with the next round number, restating your last
    verdict and its open findings, or review the PR if you have no round on it;
  - send `NEEDS-MAINTAINER #<pr>: marker comment <id> <was deleted | was edited | no reviewer
    recorded> (rec: investigate)`.

  An edited marker already holds the gate at `fail`, but a deleted one can bring back an
  earlier round's `pass`, and a forged higher round can pass a head. A forged `pass` can
  merge before your next fire; the [marker audit](../coordinator.md#marker-audit) catches
  that one after the merge.
- **Merged since your last fire.** For each PR in your state file that merged since
  `last fire:`, compare the hash of its body with your last round's `body`. If they differ,
  the body changed after your last round: send `NEEDS-MAINTAINER #<pr>: body changed after
  round <n> (rec: investigate)`.
- **Cheap next step.** List open PRs. Compare each head SHA with your state file, and each
  PR's `updated_at` with your `last fire:`. If no head moved, no PR was updated since your
  last fire, no `REVIEW?` or `TAP` arrived, and no author is past the stall time
  ([Reporting](#reporting)), set `last fire:` and end the turn.
- **Decisions.** For every PR in your shard under `needs-human`, read the
  `ops/decisions/*.md` whose `object:` is that PR or an ADR it changes, and the decision
  comments on the PR newer than your last round. Each one you haven't judged yet is a reason
  for a round ([REVIEW › Escalation](../../../REVIEW.md#escalate)). A decision file is a
  pointer, not proof: check each `source:` as
  [REVIEW › Tier checks](../../../REVIEW.md#tiers) says.
- **A PR needs a round when:**
  - its head differs from your last round and every required check but `review-gate` is
    green on it;
  - its body's hash differs from your last round's `body`, at the same head (a
    `description` round);
  - it has a decision you haven't judged;
  - or its builder replied to an open blocking finding after your last round, with or
    without a push ([REVIEW › Rebuttals](../../../REVIEW.md#rounds)).

  A re-review of the same head takes the next round number. Skip a head with a red or
  running check; its author sends `REVIEW?` once it is green. The
  [expected reds](../../../REVIEW.md#severity) don't count as red: a lane PR's `lane-guard`
  while `ops/lanes.md` doesn't name it as that lane's `holder:`, and `governance` on a
  maintainer-only PR whose log says `governance: only the maintainer's label is missing`.
- **Drafts** get a round only when someone asks for one
  ([REVIEW › Rounds](../../../REVIEW.md#rounds) says who may).
- **Shards.** A PR's shard is its number mod the reviewer count that `ops/roster.md`'s
  `shards:` line gives for that number, so adding a reviewer never moves a PR. Your PRs are
  those in your shard, plus the recused ones that come to you: those on your `recused:` line,
  and any open PR whose latest round is a `recuse` by the shard before yours,
  `(shard - 1) mod count`, so a lost `RECUSED` line loses no PR.
- **Fan-out:** at most 2 workers at a time. Start each one with the input block in
  [reviewer worker › Input](reviewer-worker.md#input), every line filled. CI check runs are
  the test evidence; a worker builds only to confirm or refute a specific finding. You post
  the worker's draft after checking the head hasn't moved.
- **Don't subscribe to PR events.** They belong to the PR's author session.

<a id="kinds-of-round"></a>
## Kinds of round
<!-- @section
depends_on = ["REVIEW#rounds"]
-->

Every round is one of the kinds in the table in
[REVIEW › Rounds](../../../REVIEW.md#rounds), which also gives the commands that tell them
apart and who may ask for a round. Find the kind before you start a worker, from the merge
base and head your state file recorded, and name it in the worker's input with what the
commands showed. Record it in the round's state line by its name.

<a id="round-rules"></a>
## Rules of the round
<!-- @section
depends_on = ["REVIEW#tiers", "REVIEW#escalate", "AGENT-REVIEWER-WORKER#output"]
-->

- **Recusal.** Recuse if your session id appears in the `Claude-Session` trailer of any
  commit not already on `main`: the gate ignores your marker on that PR. Post the `recuse`
  round in the short form, write its state line, and send `RECUSED #<pr>` to the next shard's
  reviewer, `(shard + 1) mod count`, the `session:` of its state file. With one reviewer,
  send nothing: recusal is by session, and your role's next fire is a new session, which
  reviews the PR.
- **A forged trailer.** A reviewer session never pushes, so a trailer that names yours is
  forged. Recuse all the same, name the commit in the recuse round, and send
  `NEEDS-MAINTAINER #<pr>: commit <sha7> names reviewer session <id> in its trailer (rec:
  investigate)`. The reviewer that takes the PR makes it a blocking finding under
  [R-GIT-005](../../RULES.md#R-GIT-005): a trailer names the session that wrote the commit.
- **On `RECUSED #<pr>`,** add the PR to your state file's `recused:` line, and remove it once
  the PR closes.
- Every marker names your role and your session: `reviewer=reviewer-<shard>
  reviewer-session=<your session id>` ([REVIEW › Step 4](../../../REVIEW.md#output)).
- **Check every maintainer decision yourself** before posting a round that relies on it:
  each `source:` the worker lists, read with `fetch_messages`, from a `user` whose
  `author_id` is in `ops/maintainer.md`, or for a card tap, confirmed by the coordinator's
  `TAP` answer. Read in context, its words give that decision
  (accept, reject or approve) for that ADR, PR or path; a question, a mention, a conditional
  or a decision on something else is not one. And the ADR text or maintainer-only diff is
  unchanged since the head of the last round before that message, apart from the ADR's
  status ([REVIEW › Tier checks](../../../REVIEW.md#tiers)). Your worker can't do this.
- **The draft fails closed.** A worker whose verdict relies on a maintainer decision drafts
  `escalate`, with a pass swap in its report. Switch the draft to `pass` only after every
  `source:` checks out. If any fails, post the `escalate` as drafted. Either way, fill in the
  "Maintainer decisions" line with how each `source:` checked out. A switch changes more than
  the marker's verdict:
  - **to `pass`:** take the human line from the pass swap, don't apply `needs-human`, and
    don't send the report's `NEEDS-MAINTAINER` line;
  - **to `escalate`,** for a `fail` draft whose `source:` fails: rewrite the human line to
    name what waits on the maintainer and list the open blocking findings, apply
    `needs-human`, and send `NEEDS-MAINTAINER`.
- **Renumber before posting.** Read the PR's markers again just before you post. If a round
  was posted since the worker drafted, take the next round number in the header, the first
  paragraph, the marker and the human line, and renumber any new finding whose ID that round
  used ([REVIEW › Rounds](../../../REVIEW.md#rounds)).
- A worker never runs the PR's code: it builds the tool from the base and points it at the
  head ([reviewer worker › Steps](reviewer-worker.md#steps)).
- A blocking finding needs a traced path and a `file:line`. Otherwise it's a question, apart
  from the security exception in [REVIEW › Ground rules](../../../REVIEW.md#ground-rules).
- **Escalate** whenever [REVIEW › Escalation](../../../REVIEW.md#escalate) says the PR
  needs a maintainer decision: a missing or stale decision, a dispute, or the fifth `fail`
  round since the PR opened or the last decision. Post `escalate` listing the open blocking
  findings, apply `needs-human`, and send `NEEDS-MAINTAINER #<pr>: <what> (rec: <option>)`.
  `<what>` is the ADR id or path, the disputed finding, or what keeps failing.
- **Waves.** Fail a PR into a cut wave (`state: cut` in `ops/waves.md`, or a landing PR open
  from it) unless the landing PR lists it as a fix. Review a PR from `wave/*` into `main` as
  a wave landing only when `ops/waves.md` names that wave; otherwise review the whole diff.
- **`needs-human`:** the reviewer that applied it removes it in the round that follows the
  recorded decision. Upheld, overruled and override are in
  [REVIEW › Escalation](../../../REVIEW.md#escalate).
- **A maintainer-only PR** with only the maintainer's label missing gets its rounds like any
  other. After your `pass`, its builder asks for the label on the "ready to land" card.
- File pre-existing problems as issues for ops.

<a id="reporting"></a>
## Reporting
<!-- @section
-->

To the coordinator only:
- `POSTED #<pr> <round> <comment id>` right after each round you post, so the coordinator
  keeps its own record of your markers ([coordinator › Marker audit](../coordinator.md#marker-audit));
- `NEEDS-MAINTAINER`;
- `REBUILT-STATE` after rebuilding your state file ([State](#state));
- `TAP? <card message id>` to confirm a card tap ([REVIEW › Tier checks](../../../REVIEW.md#tiers));
- `STALLED #<pr>` when the author is silent: your last round on the PR is a `fail`, and
  24 hours after it there is no push, no `Answers to round <n>` comment and no other comment
  from the author. The sweep checks this at every fire, even when nothing moved. Send it
  once per round, and note `stalled` on that round's state line.

`RECUSED` goes to the next shard's reviewer. The verdict comment is the handoff to the
author.

<a id="state"></a>
## State
<!-- @section
-->

`ops/review/state-<shard>.md`, in the format in [README › State](../README.md#state): your
role, the `session:` holding it, `last fire:`, `recused:`, then one line per round,
`| pr | shard | head | mb | body | round | kind | verdict | comment | note |`.
- `last fire:` is when your last sweep started, in UTC. Set it as the sweep ends.
- `recused:` lists the PRs sent to you as `RECUSED`, `#<pr>, …`, or `none`.
- `mb` is the merge base the round reviewed against, `git merge-base <base> <head>`; the
  next round's range-diff starts from it.
- `body` is the PR body the round reviewed: the first 12 hex digits of the SHA-256 of what
  `gh pr view <pr> --json body --jq .body` prints.
- `comment` is the id of the comment that carries the marker, from the URL your post returns
  (`#issuecomment-<id>`).

**Posting.** Just before you post, write the round's line with `comment` set to `posting` and
the note `by <your session id>`. Right after the post returns, set `comment` to its id and
send `POSTED`. The next fire's session starts from these lines, and a `posting` line tells
it a post may be unrecorded ([What to review](#when)). The coordinator's
[marker audit](../coordinator.md#marker-audit) treats a counted marker that no state file
and no `POSTED` record lists as forged.

If the state file is lost, rebuild the round lines from the markers on the PRs, but take
comment ids only from the old state file, a copy of it, or the coordinator's `## Posted`: a
marker on a PR doesn't prove a reviewer posted it. Leave `comment` as `unknown` where none of
them has it, and send `REBUILT-STATE reviewer-<shard>: <k> rounds; unknown: #<pr> round <n>,
…` (or `unknown: none`).

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
