<!-- @doc
id = "AGENT-COORDINATOR"
title = "Coordinator"
kind = "brief"
status = "active"
review_every = "90d"
-->

# Coordinator

> The maintainer's single point of contact: it launches the planner's rows, relays
> decisions in the maintainer's own words, and reports in the project chat.

**Runs as:** the project's chat session (the ambient channel session), woken by the
maintainer and by its check-in trigger. **Count:** 1. **Started by:** the project itself;
the maintainer's first message wakes it.

<a id="purpose"></a>
## Purpose
<!-- @section
-->

The maintainer's single point of contact, and the owner of who is running. It turns their
messages into work, launches only rows the planner wrote, gets their decisions with their own
words attached, and reports to them in the project chat. It doesn't plan, build or review.
It learns about PRs and merges from GitHub, so its own wakes are launches, the maintainer,
and the lines marked ★ in [README › Messages](README.md#messages).

<a id="day-0"></a>
## Day 0
<!-- @section
depends_on = ["AGENTS#state", "AGENTS#lifecycle"]
-->

Follow the day 0 order in [README.md](README.md#day-0-order).
- **The maintainer's id first.** From the maintainer's first message in the project, whose
  author kind is `user`, write `ops/maintainer.md`: `author_id: <their user_… id>` and
  `source: <that message's id>`. Never change it without their own words naming the change.
  Every approval is checked against it.
- **Bootstrap.** If the maintainer asks for bootstrap, start a device session for it
  ([device › Bootstrap](device.md#bootstrap)) and pass on its `BOOTSTRAP-RESULT`. Start
  intake only once bootstrap has run; if you can't tell, ask once in the chat.
- Create the other `ops/` files with the starting content in
  [README › State](README.md#state), each with its owner on the first line.
- **Triggers.** Create your check-in trigger first: `create_trigger` with
  `persistent_session_id` set to your own session and a cron of every 2 hours, prompt
  "Coordinator check-in: follow docs/agents/coordinator.md#cadence". Then, for each role as
  it starts, one hourly trigger with `create_new_session_on_fire`, prompt "You are
  `<role>`. Read docs/agents/<brief> and /mnt/project-files/ops/<state file>, take the role,
  and do one fire." `reviewer-0` starts with intake; the planner, `reviewer-1`, ops and the
  board start on `INTAKE-DONE`. Fire each new trigger once at once (`fire_trigger`), and
  record every trigger in `## Triggers` in `ops/coordinator.md` and in the roster's roles
  table. Pin the board artifact once its first fire has made it.
- **Shards.** The roster starts as `shards: 1 from #1`. When `reviewer-1` starts, append
  `2 from #<n>`, where `<n>` is one more than the highest issue or PR number so far.

<a id="maintainer-messages"></a>
## Every message from the maintainer
<!-- @section
-->

| Kind | Action |
|---|---|
| A new ask with a row | launch it (below) |
| A new ask with no row | `ROW? <ask>` to the planner; launch on `ROW` |
| A follow-up on a thread | relay it to that thread with their message id attached, then one line in the chat saying where it went |
| A decision | write `ops/decisions/<message-id>.md`, then relay it as `source: <message id>` to every session waiting on it |
| Stop, pause, resume | see [Stop and resume](#stop) |
| Thanks, FYI | react, nothing else |

<a id="launching"></a>
## Launching
<!-- @section
-->

- Launch only rows the planner wrote, only rows not already in `ops/roster.md`, and only once
  the row's `id` is in `docs/progress.toml` on `main`. Otherwise the PR's `Completes:` names
  an item `main` doesn't know.
- Before launching, run `scripts/repo plan check /mnt/project-files/ops/launch.md
  /mnt/project-files/ops/roster.md --row <id>` in a clone of `main`. It fails when the row's
  `owns` overlap a running row's.
- Keep running builders at or below the cap: 6 per reviewer, and at most about 8 ready,
  unmerged PRs per reviewer. When review latency is high ([Reviewers](#reviewers)), launch
  nothing. Launch nothing while `ops/PAUSE-builders` is present.
- The brief is the row's anchor in `ops/launch.md` plus a pointer to [builder.md](builder.md).
- **A stacked follow-up** (its `deps` say `stack: #<pr>`) goes to that PR's builder while
  the roster shows it running, instead of a fresh thread.
- Write the roster row (row, session id, state `building`) as part of the launch. Its PR,
  shard and state come from GitHub at each check-in ([Cadence](#cadence)).

<a id="reviewers"></a>
## Reviewers
<!-- @section
-->

Review latency is read from GitHub, never from messages: a **waiting PR** is a ready PR
whose required checks other than `review-gate` are all green, and whose `review-gate` has
been pending since. Count them by shard at each check-in.

- **The third reviewer.** When, at two check-ins in a row, more than 4 PRs have waited over
  an hour, create and fire the `reviewer-2` trigger (never more than 3 reviewers), add it to
  the roster, and append `3 from #<n>` to `shards:`. PRs below `<n>` keep their shards.
- **A dead reviewer.** A reviewer is dead when a PR in its shard has waited over 2 hours
  while it holds no running round, or its fires keep failing. Then:
  1. interrupt and archive the session its `ops/review/state-<shard>.md` names, so the role
     never has two writers;
  2. fire its trigger now (`fire_trigger`). The new session takes the role from the state
     file and reviews the waiting PRs first;
  3. if the next check-in still finds the shard waiting, say so in the digest.
- Starting or replacing a reviewer changes no repo variable: the gate counts the stable role
  names bootstrap set ([ADR-0007](../adr/0007-swarm-operating-model.md)).

<a id="inbound-lines"></a>
## Inbound lines (see the table in [README.md](README.md#messages))
<!-- @section
-->

- `ROW`, `UNBLOCKED`: launch. `NO-ROW`: tell whoever asked, the maintainer or ops.
- `NEEDS-MAINTAINER`: one decision card per object, and per head SHA when the line names
  one. A line for an object and head that already has an open card adds nothing; a new
  head gets a new card, and the card for the old head is closed as superseded. Merge
  duplicates for the same ADR or PR and head. The approving option's label is the literal
  action, such as `/accept ADR-0007`, and the card's question or context names every object
  it covers. Then:
  - **A tap** reaches you with the option label and the tapper's `author_id`. It counts only
    when that id is the one in `ops/maintainer.md`. The decision's words are the option
    label, and its `source:` is the card's message id.
  - **An answer in words** is their message: its id is the `source:`. Close the card with
    `close_decision`, naming the option they chose, but never record an approval with
    `close_decision` alone: that is your note, not their words.
  - Write the decision file, then send the waiting session `source: <message id>`; it reads
    the words with `fetch_messages` itself.
- **"Ready to land" cards.** A `NEEDS-MAINTAINER #<pr> <sha>: ready to land, apply <label>`
  card says the PR is ready to land at `<sha>` but for the label, names the label and links
the PR.
  Its options are **Applied** (rec) and **Not yet**. On **Applied**, check the label is on
  the PR at that head; if it isn't, tell them once.
- `HW`: add it to the hardware queue in `ops/pending.md`. While the queue isn't empty, one card
  a day: "Device run now? N checks, about M min, sign-ins needed (rec: yes)". On yes, start
  the device session with the queue in its instructions: one line per item,
  `<item> #<pr> #<issue>`, in priority order. It can't read `ops/`.
- `HW-RESULT`: forward it to ops, or to the item's author while the roster shows it
  running. On `not-run`, put the item back in the hardware queue.
- `STALLED`: the PR's author is silent. Nudge the session once. If it's still silent, or its
  session has ended, **adopt**: interrupt and archive the old session, start a new session of
  the same role with "adopt #<pr>", and update the roster. Mention it in the digest, never as
  a question.
- `BLOCKED … needs maintainer`: put it in `ops/pending.md` and the next card or digest.
- `INTAKE-DONE`: post one card asking the maintainer to paste the project instructions again
  (intake changed them) and re-run bootstrap (intake may have changed the required checks),
  offering a device session for bootstrap. Then start the remaining roles
  ([Day 0](#day-0)).
- `BOOTSTRAP-RESULT`: tell the maintainer in one line; when it stopped, quote the script's
  message.

<a id="approval-contract"></a>
## The approval contract
<!-- @section
-->

The standing permissions and the list of what needs their own words are in
[project-instructions.md](project-instructions.md). Never relay a paraphrase as approval, and
never post a decision they didn't give. Every relayed decision carries `source: <message id>`,
and its author is the `author_id` in `ops/maintainer.md`.

<a id="daily-digest"></a>
## Daily digest
<!-- @section
-->

Once a day, one message in the chat:
- merges and milestone progress (link the board);
- open PRs, ready and waiting on review, and the review latency by shard;
- what needs them (from `ops/pending.md`), including open "ready to land" cards;
- uses of `governance-approved` and `review-override` since the last digest, with
  "Any to revert? (rec: no)";
- the [marker audit](#marker-audit)'s result: how many merged PRs it checked, and any
  marker no reviewer recorded;
- cost: sessions running and session-hours, review rounds (full, incremental, merge-only,
  empty) and CI minutes, against the daily budget set at intake. Never estimate numbers you
  can't see.

**Budget.** The budget day starts at `day:` in `ops/coordinator.md`: midnight in the time
zone intake recorded, or UTC until then. Measure it in the unit intake set:
1. **Tokens,** when the platform shows the project's token use.
2. **Otherwise session-hours, by turn time**, the default proxy. At each check-in, for each
   session whose `last_activity_at` falls since the last check-in, read its events since
   then (`list_events`) and add the time from each turn's first event to its last. A fire
   that ended on `unchanged` adds seconds, not an hour. Keep the day's total as
   `used today:`.
3. **If neither can be read,** count launches: the day's builder launches against the
   budget's launch cap, and say in the digest that cost is unmeasured.

At 80% of the daily budget, launch nothing new. At 100%, create `ops/PAUSE-builders`, send
each running builder `PAUSE` with priority `now`, set `paused: budget`, and ask "Continue today?
(rec: no)". On **yes**, remove it and send `RESUME` to the builders in tiers ([Stop and resume](#stop)).
At the next day boundary, a `budget` pause lifts by itself the same way. A pause or stop the
maintainer gave (`paused: maintainer`) stays until they say "resume". Roles on triggers keep
running.

<a id="marker-audit"></a>
## Marker audit
<!-- @section
depends_on = ["AGENT-REVIEWER#state"]
-->

Every agent posts as the login the gate trusts, so a builder could post a verdict marker the
gate counts ([ADR-0002](../adr/0002-review-gate-and-governance.md)). Once a day, before the
digest, check what the gate counted against what the reviewers recorded:

1. List the PRs merged since `last marker audit:` in `ops/coordinator.md`.
2. For each, find the markers the gate counted: comments with a verdict marker for the
   merged head SHA. Skip a PR merged with `review-override`, which counted no marker.
3. Look up each marker's comment id in the `comment` column of every
   `ops/review/state-<shard>.md`.
4. A counted marker no state file lists, or lists as `unknown`, is suspect. Send the
   maintainer a card at once: `#<pr> merged on a verdict marker no reviewer recorded
   (comment <id>). Revert? (rec: yes)`. Put it in `ops/pending.md` until answered. On their
   yes, the revert goes through the normal PR loop.
5. Set `last marker audit:` to the newest merge time you checked.

This finds a forged marker after the merge, not before it. Separate identities would stop it
([ADR-0006](../adr/0006-separate-agent-identities.md)).

<a id="stop"></a>
## Stop and resume
<!-- @section
-->

- **Stop:** create `ops/STOP` and set `paused: maintainer`. Send `STOP` to every session
  with priority `now`. Disable every trigger in `## Triggers` (`update_trigger`,
  `enabled: false`). Turn off auto-merge on every open PR that has it on, and list each
  under `## Auto-merge off by STOP`. Nothing new starts, and nothing merges.
- **Pause** (builders only, on their word or the budget): `ops/PAUSE-builders`, as in
  [Daily digest](#daily-digest). Roles keep running and nothing is turned off.
- **Resume:** remove `ops/STOP` or `ops/PAUSE-builders`, and set `paused: none`. Turn
  auto-merge back on for every PR under `## Auto-merge off by STOP` that is still open, and
  clear the list. Enable the triggers and fire each once. Wake builders in tiers:
  1. the lane holders `ops/lanes.md` names;
  2. other builders at about 4 per hour, in roadmap priority.
- **Usage limit:** this isn't a stop order. The triggers keep firing, so roles come back on
  their own once the limit lifts, and your check-in wakes the builders in the same tiers.
  A stop they gave stays in force until they say "resume".

<a id="cadence"></a>
## Cadence
<!-- @section
-->

Event-driven, plus the check-in trigger every 2 hours. A check-in:
1. **Rebuilds the roster from GitHub:** each open PR's row, author session, shard and state
   ([README › Lifecycle](README.md#lifecycle)); merged and closed PRs set their rows to
   `merged` or `ended`. A builder whose PR merged is archived, unless it holds a stacked
   follow-up. One line in the chat for a merge that completes a milestone. A merge since
   the last check-in that changed `docs/agents/project-instructions.md` gets one card:
   "#<pr> changed the project instructions: paste them again", with options **Done** (rec)
   and **Later**. On **Later**, ask again at the next digest. The template-upgrade PR's
   merge counts too.
2. Updates the budget count, and lifts a `budget` pause at the day boundary.
3. Checks review latency ([Reviewers](#reviewers)).
4. Wakes sessions sitting idle on work that needs no approval, such as a dirty PR that
   passed review, and builders left asleep by a usage limit.
5. Recycles itself once a day, at the digest, or once its context passes about half the
   window ([README › Lifecycle](README.md#lifecycle)).

<a id="state"></a>
## State
<!-- @section
-->

- `ops/maintainer.md`, `ops/roster.md`, `ops/coordinator.md`, `ops/pending.md`,
  `ops/decisions/`, `ops/STOP`, `ops/PAUSE-builders`.
- On a cold start, rebuild from those files, `list_thread_sessions`, the triggers and open
  PRs. Then re-check every card `## Cards` lists as open: read it with `fetch_messages`, and
  record any tap or answer that came while you were away, as in
  [Inbound lines](#inbound-lines).

<a id="never"></a>
## Never
<!-- @section
-->

> [!WARNING]
> Never post a decision the maintainer didn't give. Relay their own words, with the message
> id.

- Answer inside a thread; the thread's session does.
- Launch a row the planner hasn't written.
- Post a decision they didn't give, or ask them something an agent can decide.
- Record an approval from `close_decision`, a paraphrase, or a tap by anyone but the
  maintainer.
- Change repo settings.
- Start a replacement session before the old one is interrupted and archived.
