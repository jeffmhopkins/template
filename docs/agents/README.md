<!-- @doc
id = "AGENTS"
title = "Agent types"
kind = "brief"
status = "active"
review_every = "90d"
-->

# Agent types

> Which agents run and how many, the order they start in on day 0, how they wake, the
> one-line messages they send each other, and the shared state each one owns.

There is one brief per agent type. They live at `docs/agents/`. A session starts with its brief's path. Briefs don't
restate repo rules: they point to CLAUDE.md, REVIEW.md and the runbooks. Standing permissions
and the approval contract live only in [project-instructions.md](project-instructions.md).
The operating model is [ADR-0007](../adr/0007-swarm-operating-model.md).

| Type | File | Count | Runs as |
|---|---|---|---|
| Intake | [intake.md](intake.md) | 1, first | a thread that questions the maintainer about the product, then ends |
| Coordinator | [coordinator.md](coordinator.md) | 1 | the project's chat session, woken by the maintainer and by its check-in trigger |
| Planner | [planner.md](planner.md) | 1 | a role taken by a fresh session at each fire of its hourly trigger |
| Builder | [builder.md](builder.md) | at most 6 per reviewer (12 to start) | one thread per roadmap row, woken by its PR's events, ended after its PR merges |
| Reviewer | [reviewer.md](review/reviewer.md) | 2 to start, a 3rd when review latency stays high ([coordinator › Reviewers](coordinator.md#reviewers)) | roles `reviewer-0` to `reviewer-2`, one per shard, each taken by a fresh session at each fire of its hourly trigger |
| Reviewer worker | [reviewer-worker.md](review/reviewer-worker.md) | at most 2 per reviewer at a time | an Agent-tool worker a reviewer starts for one PR |
| Ops | [ops.md](ops.md) | 1 | a role taken by a fresh session at each fire of its hourly trigger |
| Board | [board.md](board.md) | 1 | a role taken by a fresh session at each fire of its hourly trigger, rebuilding an artifact |
| Device session | [device.md](device.md) | 0 or 1 | Remote Control on the maintainer's machine |

<a id="day-0-order"></a>
## Day 0 order
<!-- @section
-->

The maintainer's side of the same steps is in
[Getting started](../GETTING-STARTED.md#start).

1. The maintainer creates the repo from the template, installs the Claude GitHub App on it,
   and adds it to the Claude project.
2. The maintainer pastes the block in [project-instructions.md](project-instructions.md)
   into the project settings, filling in `PROJECT_NAME` and `OWNER/REPO`.
3. Bootstrap runs with template defaults: the maintainer runs `scripts/bootstrap.sh`, or asks
   in the chat and the coordinator starts a device session for it
   ([device › Bootstrap](device.md#bootstrap)). It sets `REVIEWER_ROLES` once.
4. The coordinator records the maintainer's user id in `ops/maintainer.md` from their first
   message, creates the other `ops/` files with their starting content (below), and creates
   its own check-in trigger. It then starts intake, and creates and fires the `reviewer-0`
   trigger, so intake's design PR can be reviewed.
5. On `INTAKE-DONE`, the coordinator asks the maintainer to paste the instructions again and
   re-run bootstrap, since intake may have changed both. It creates and fires the triggers
   for the planner, `reviewer-1`, ops and the board, then launches the first builders from
   the planner's rows.

```mermaid
flowchart LR
    A["Repo from the template, Claude app, Claude project"] --> C["Paste project-instructions.md"]
    C --> B["scripts/bootstrap.sh"]
    B --> D["Coordinator creates ops/ and its check-in trigger, starts intake and reviewer-0"]
    D -->|INTAKE-DONE| G["Maintainer re-pastes the instructions, re-runs bootstrap"]
    D -->|INTAKE-DONE| E["Triggers for planner, reviewer-1, ops, board"]
    E --> F["First builders, from the planner's rows"]
```

<a id="messages"></a>
## Messages
<!-- @section
-->

Agents send each other one-line messages with `send_message`, in these fixed formats.
**Role-to-role lines go direct.** A role's session is the `session:` line of its state file
([State](#state)); a PR's author is the `Claude-Session:` line of its body. The coordinator
gets only the lines marked ★. It learns about PRs and merges from GitHub, not from messages
([Lifecycle](#lifecycle)). Forward a line verbatim; never rewrite it. A line not in this
table is not sent.

```mermaid
flowchart LR
    intake([intake]) -->|INTAKE-DONE| coord([coordinator])
    coord -->|"ROW?"| planner([planner])
    ops([ops]) -->|"ROW?"| planner
    planner -->|"ROW / NO-ROW, UNBLOCKED"| coord
    planner -.->|"copy: ROW / NO-ROW"| ops
    planner -->|ROW-CHANGED| author([PR author])
    author -->|MERGED| planner
    author -->|"MERGED, LANE-READY, DECLINED"| ops
    author -->|"REVIEW?"| reviewer([reviewer])
    author -->|HW| coord
    reviewer -->|"RECUSED"| next([next shard's reviewer])
    reviewer -->|"STALLED, NEEDS-MAINTAINER"| coord
    ops -->|STALLED| coord
    ops -->|"MERGE-MAIN, BASE, LANE-HOLD, ROUTE"| author
    any([any role]) -->|MAIN-RED| ops
    device([device]) -->|"HW-RESULT, BOOTSTRAP-RESULT"| coord
    coord -->|HW-RESULT| ops
    any -->|BLOCKED| planner
    any -->|BLOCKED| ops
    any -->|"BLOCKED, NEEDS-MAINTAINER"| coord
    coord -->|"STOP / PAUSE / RESUME"| all([all roles])
```

"PR author" is whichever session opened the PR ([Lifecycle](#lifecycle)): a builder, intake,
the planner or ops.

<details>
<summary><b>All message formats</b> (21)</summary>

| Line | From → to | Receiver does |
|---|---|---|
| `ROW? <ask>` | coordinator, ops → planner | writes a row |
| `ROW <id> (brief: ops/launch.md#<id>)` / `NO-ROW <ask>: <why>` | planner → coordinator ★, copied to the asker when that is ops | coordinator: launches the row, or reports why not; ops: notes the outcome against the issue |
| `UNBLOCKED <ids>` | planner → coordinator ★ | launches the rows not already in `roster.md`; a stacked follow-up goes to its base PR's builder while that one runs |
| `ROW-CHANGED <id>: <what changed>` | planner → that row's author | re-reads its section in `ops/launch.md` and stays inside the new `owns` |
| `REVIEW? #<pr> <sha>` | PR author → its shard's reviewer, once every required check but `review-gate` is green at `<sha>` | reviews that head now |
| `RECUSED #<pr>` | reviewer → the next shard's reviewer | reviews it in place of the recused shard |
| `MERGED <item or -> #<pr> [wave/<name>]` | PR author → planner and ops | planner: unblocks; ops: lanes and waves |
| `MAIN-RED #<issue>` | any → ops | acts on the red `main` now ([ops › Main is red](ops.md#main-health)) |
| `MERGE-MAIN #<pr>: <what conflicts>` | ops → PR author | merges main, or its wave, next |
| `BASE #<pr> <branch>` | ops → PR author | the PR now targets `<branch>`, moved or after a cut: merge it in now |
| `LANE-READY <lane> #<pr>` / `LANE-HOLD <lane> <value> #<pr>` | PR author ↔ ops | asks for / grants the lane slot |
| `ROUTE #<issue> → <row>` | ops → that row's author | fixes it in the PR, or answers `DECLINED` |
| `DECLINED #<issue>: <why>` | PR author → ops | takes the issue back: fixes it, routes it elsewhere, or asks the planner `ROW?` |
| `BLOCKED <item or #pr>: <what>, needs <path <glob> \| #<pr> \| maintainer>` | any → planner (a path), ops (a PR to merge first), coordinator ★ (the maintainer) | unblocks or reroutes |
| `STALLED #<pr>` | reviewer, ops → coordinator ★ | only when the PR's author is silent: nudges once, then adopts the PR |
| `NEEDS-MAINTAINER <item or #pr>[ <sha>]: <question> (rec: <option>)` | any → coordinator ★ | one decision card per object, and per head SHA when one is given |
| `HW <item> #<pr>: #<issue>` | PR author → coordinator ★ | adds it to the hardware queue in `ops/pending.md` |
| `HW-RESULT <item> #<pr>: pass/fail/not-run #<issue>` | device → coordinator ★ → ops, or the item's author while `roster.md` shows it running | pass: opens the record PR; fail: fixes it; not-run: the coordinator queues it again |
| `INTAKE-DONE #<pr>` | intake → coordinator ★ | asks the maintainer to re-paste the instructions and re-run bootstrap; starts the other roles |
| `BOOTSTRAP-RESULT ok/stopped/failed: <the script's last line or stop message>` | device → coordinator ★ | tells the maintainer the result in the chat, with the script's stop message when it stopped |
| `STOP` / `PAUSE` / `RESUME` | coordinator → all for `STOP`, running builders for `PAUSE`; `STOP` and `PAUSE` with priority `now` | stop at once / a builder ends its turn and waits, as `ops/PAUSE-builders` also says / pick up from state files |

</details>

<a id="state"></a>
## State: `/mnt/project-files/ops/`
<!-- @section
depends_on = ["TOOL#plans"]
-->

> [!WARNING]
> The shared folder is a network mount where the last write wins, so:
> 1. **One writer per file.** The owner's role is the file's first line. A role's own state
>    file names the session that holds the role on its `session:` line, and only that
>    session writes the role's files. Others read them, and message the owner to change one.
>    On day 0 the coordinator creates every file with its starting content.
> 2. **Several producers means one file per event** (`decisions/<message-id>.md`), never a
>    shared file edited by many.
> 3. **Write a temp file, then rename.** Readers ignore `*.tmp`.
> 4. **GitHub is the source of truth where it can be.** Rows, shards, review state, lanes
>    and progress can be rebuilt from PRs, markers and `main`. A file is a cache; on a cold
>    start, rebuild it.
> 5. **Sub-workers never write `ops/`.** They return results; the session that started them
>    writes.

| File | Owner | Holds |
|---|---|---|
| `STOP` | coordinator | present = stopped. Every trigger fire and session checks it first and exits |
| `PAUSE-builders` | coordinator | present = builders pause, for the budget or on the maintainer's word. Every builder checks it at each wake |
| `maintainer.md` | coordinator | the maintainer's user id, which every approval is checked against |
| `roster.md` | coordinator | the shard count and from which PR it holds, each role's trigger, and each row's session, PR, shard and state, rebuilt from GitHub at each check-in |
| `coordinator.md` | coordinator | open cards (by object and head), pending relays, triggers, the budget day and use, PRs whose auto-merge `STOP` turned off |
| `decisions/<message-id>.md` | coordinator | one file per maintainer decision |
| `pending.md` | coordinator | open maintainer questions, the hardware queue, accounts to create |
| `planner.md` | planner | the session holding the role, and its last fire |
| `launch.md` | planner | the shared-append paths, then one anchored section per row ([planner › The row](planner.md#the-row)). Whether a row is launched is in `roster.md` |
| `roadmap.md` | planner | lanes, hot spots, what only the maintainer can do, what's left |
| `review/state-<shard>.md` | that reviewer role | the session holding the role, and one line per round, with the comment id of its marker |
| `ops.md` | ops | the session holding the role, its last fire, routed issues, flakes, the `MERGE-MAIN` count, PRs whose auto-merge it turned off, the PR fixing `main` |
| `mode.md` | ops | waves on or off, the current wave, main's health |
| `lanes.md` | ops | per lane: holder, then ready PRs in order |
| `waves.md` | ops | each wave, its state, what it carries, its landing PR |
| `board.md` | board | the session holding the role, and its last fire |
| `heads/<role>.txt` | that role | the `scripts/repo heads-changed` state for its fire |

**Formats.** Each file's first line is its owner's role. What follows, with the starting
content the coordinator writes on day 0:

- `maintainer.md`: `author_id: <user_…>`, then `source: <message id>`: the maintainer's first
  message in the project, whose author kind is `user`. It changes only on the maintainer's
  own words, with that message's id as the new `source:`.
- `roster.md`: a `shards:` line, then two tables. `scripts/repo plan check` reads the second
  one.

  ```markdown
  shards: 1 from #1, 2 from #14

  | role | trigger | state |
  |---|---|---|
  | reviewer-0 | trig_01… | on |

  | row | session | pr | shard | state |
  |---|---|---|---|---|
  | B4 | session_01… | #12 | 0 | review |
  ```

  `shards:` lists each reviewer count with the first PR number it holds for, so a shard
  never moves when a reviewer is added. Roles are `intake`, `planner`, `reviewer-<shard>`,
  `ops`, `board` and `device`; a role's state is `on`, `off` (its trigger disabled) or
  `ended`. A row is an item id from `launch.md`, or `#<pr>` for a PR with no row (a
  Dependabot PR, a wave PR, a revert). Its state is `building` (draft), `review` (ready),
  `waiting` (on a lane, a base PR, hardware or the maintainer), `merged` or `ended` (closed
  unmerged, or abandoned). `plan check` counts every row as running unless it is `merged`
  or `ended`. It starts as `shards: 1 from #1` and two empty tables, under their header
  lines.
- `coordinator.md`: `## Cards` (`<object> | <head sha or -> | <card message id> | open/answered`),
  `## Relays` (`<message id> → <session>`), `## Triggers`
  (`<role> | <trigger id> | <cron> | on/off`), then `day: 00:00 <IANA time zone>` (from
  intake; `UTC` until then), `launches today: 0`, `used today: 0` (the budget unit,
  [coordinator › Daily digest](coordinator.md#daily-digest)), `paused: none` (`budget`,
  `maintainer` or `none`), `last marker audit: none`
  ([coordinator › Marker audit](coordinator.md#marker-audit)) and `## Auto-merge off by STOP`
  (`#<pr>`), starting empty.
- `decisions/<message-id>.md`: `source: <message id>`, `author_id: <user_…>`,
  `object: <ADR id, #pr or path>`, `words: "<their words, quoted>"`, and
  `relayed to: <sessions>`. For a card tap, `source:` is the card's message id and `words:`
  the option label.
- `pending.md`: `## Questions`, `## Hardware queue` (`<item> #<pr> #<issue>`, in priority
  order) and `## Accounts`, each starting empty.
- `planner.md`, `board.md`: `session: none`, then `last fire: none`.
- `launch.md`: a `<a id="shared"></a>` section whose `owns:` lists the shared-append paths,
  then one section per row, as the planner brief says. Starts with the shared section only,
  its `owns:` empty.
- `roadmap.md`: `## Lanes`, `## Hot spots`, `## Only the maintainer`, `## What's left`.
- `review/state-<shard>.md`: `session: none`, then
  `| pr | shard | head | round | kind | verdict | comment | note |`, starting empty. `kind`
  is `full`, `incremental` or another kind of round; `comment` is the id of the comment that
  carries the marker.
- `ops.md`: `session: none`, `last fire: none`, `merge-mains today: 0`, then `## Routed`
  (`#<issue> → <row>`), `## Flakes` (`<test> | #<issue>`), `## Auto-merge off` (`#<pr>`)
  and `## Fixing main` (`#<pr>`).
- `mode.md`: three lines, starting as `waves: off`, `wave: none`, `main: green`. `waves` is
  `on` or `off`, `wave` the current `wave/<name>` or `none`, `main` is `green` or `red #<issue>`.
- `lanes.md`: one `## <lane>` per lane in `lanes.toml`, with `holder: #<pr>` or `none`, and
  `ready: #<pr>, …` in order.
- `waves.md`: one `## wave/<name>` per wave, with `state: open | cut | landing`,
  `carries: #<pr>, …`, `landing: #<pr>` or `none`, and `fixes: #<pr>, …`. Starts empty.

<a id="permissions"></a>
## Shell permissions
<!-- @section
covers = [".claude/settings.json"]
-->

[`.claude/settings.json`](../../.claude/settings.json) sets which shell commands a session
runs without asking, and which it never runs. It is a maintainer-only path. The lists are a
second line of defence: the rules in the briefs still apply to every allowed command.

**Allowed without a prompt:**
- the repo tool, the stack's format, lint, test, build and dependency commands, and
  `node --test`;
- `scripts/bootstrap.sh --check` only. Bootstrap's write mode changes repository settings,
  so it always prompts;
- reading and writing git history, pushing a branch, and worktrees;
- reading PRs, issues and runs, and the PR loop's writes: create, edit, comment, ready,
  `gh pr merge <n> --auto …` and `gh pr merge <n> --disable-auto`, issues, re-runs and the
  review gate's re-run;
- `gh api repos/…` reads.

**Denied, whatever the allow list says:**
- force pushes, pushes that delete a branch or update one with `+`;
- `gh pr merge --admin`;
- adding `review-override` with `gh pr edit` or `gh issue edit`. Only the maintainer applies
  it ([R-GOV-002](../RULES.md#R-GOV-002));
- `gh api` with a method or a body in any spelling: `-X`, `--method`, `-f`, `-F`, `--field`,
  `--raw-field` and `--input`, joined to their value or not. What's left is a GET;
- repository settings, variables, secrets, rulesets, label creation and deletion, and
  releases.

`governance-approved` on a maintainer-only path can't be told apart by a command pattern;
the rule in [project-instructions.md](project-instructions.md) covers it.

<a id="lifecycle"></a>
## Lifecycle rules every brief assumes
<!-- @section
-->

- **On every wake, first** check `ops/STOP` (stop if present) and `ops/mode.md`. A builder
  also checks `ops/PAUSE-builders`, and ends the turn without acting while it is present.
- **Triggers, not chains.** Timed work runs from durable platform cron triggers
  (`create_trigger`), never from a `send_later` a session re-arms. The coordinator creates
  them and records their ids in `ops/coordinator.md`:
  - one per role cadence, hourly, each firing a fresh session (`create_new_session_on_fire`)
    whose prompt names the role, its brief and its state file: the planner, each
    `reviewer-<shard>`, ops and the board;
  - one check-in trigger every 2 hours that fires into the coordinator's own session.

  A trigger keeps firing through a usage limit; a fire that fails is simply missed, and the
  next one carries on. The platform's minimum interval, normally an hour, is the slowest
  any role reacts; events cover what can't wait.
- **Taking a role.** A fired session reads its role's state file. When the `session:` line
  names an earlier session that is still mid-turn (`get_session`), the fire ends at once.
  Otherwise it writes its own session id on that line, archives the earlier session, and
  acts. A session that gets a line after it lost the role forwards the line, unchanged, to
  the session the file names, and ends. Lines sent to a role between fires reach the
  session that holds it.
- **Recycling.** A role session lasts one fire, so its context stays short. The coordinator
  is recycled daily, at the digest, or once its context passes about half the window: it
  writes its state files, then carries on from `ops/` and GitHub alone, as on a cold start,
  relying on nothing earlier in its conversation.
- **Whoever opens a PR is its author session**: a builder, intake, the planner or ops, which
  is also the author of Dependabot's PRs. The author subscribes to the PR's events and
  drives it by [builder › The loop](builder.md#the-loop), steps 3 to 6 and 8, until it
  merges. Then it sends `MERGED` to the planner and ops. A PR's events go to its author
  session only; other roles poll.
- **Rows and shards come from GitHub.** A PR's row is the item in its body's `Completes:` or
  `Part-of:` line, or `#<pr>` without one. Its author is the session in its body's
  `Claude-Session:` line. Its shard is its number mod the reviewer count that `roster.md`'s
  `shards:` line gives for that number. A recused PR goes to the next shard,
  `(shard + 1) mod count`. Nobody sends a line to record any of this.
- **Adoption.** When a session dies or stalls, the coordinator interrupts and archives it,
  then starts a new one with "adopt #<pr>". The new session subscribes, reads the PR and the
  old thread, and replaces the PR body's `Claude-Session:` line with its own.
- **No status posts in threads.** Every reply in a thread wakes the coordinator. Reply in a
  thread only when the maintainer wrote there.
- **Builders end after their PR merges.** A fresh row gets a fresh thread, except a stacked
  follow-up, which the coordinator hands to its base PR's builder while that one runs.
- **Reconcile from `main`.** Events can be lost when a session dies. At each fire, the
  planner and ops first run `scripts/repo heads-changed --state
  /mnt/project-files/ops/heads/<role>.txt`, ending the turn on `unchanged`; otherwise they
  read `main` and the merged PRs, and act on every merge as if its `MERGED` line had
  arrived. The coordinator rebuilds `roster.md` from GitHub at each check-in.
