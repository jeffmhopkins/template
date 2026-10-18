<!-- @doc
id = "AGENT-OPS"
title = "Ops"
kind = "brief"
status = "active"
review_every = "90d"
-->

# Ops

> Keeps `main` green and merges flowing: main's health, serial lanes, mergeability, wave
> branches, issues and Dependabot.

**Runs as:** a role, taken by a fresh session at each fire of its hourly trigger
([README › Lifecycle](README.md#lifecycle)). **Count:** 1. **Started by:** the coordinator,
which creates the trigger on `INTAKE-DONE`.

<a id="purpose"></a>
## Purpose
<!-- @section
-->

Gets reviewed PRs into `main` without collisions, and keeps loose ends from piling up. It
owns main's health in `ops/mode.md`, the serial lanes, mergeability, the wave branches,
issues and Dependabot's PRs. It reverts a merge that broke `main` before anything else.

<a id="each-fire"></a>
## Each fire
<!-- @section
depends_on = ["AGENTS#lifecycle"]
-->

1. Take the role ([README › Lifecycle](README.md#lifecycle)), then check `ops/STOP` and
   `ops/mode.md`.
2. **Cheap first step:** `scripts/repo heads-changed --state
   /mnt/project-files/ops/heads/ops.txt`. If it prints `unchanged`, no issue changed since
   `last fire:` in `ops/ops.md`, and the "main is red" issue is as `ops/mode.md` says, write
   `last fire:` and end the turn.
3. Otherwise, in this order: [main's health](#main-health), [lanes](#serial-lanes),
   [mergeability](#mergeability), [waves](#waves), then [issues](#issues).
4. Between fires, act on lines as they arrive: `MAIN-RED`, `MERGED`, `LANE-READY`,
   `DECLINED`, `BLOCKED … needs #<pr>` and `HW-RESULT`.

<a id="main-health"></a>
## Main is red
<!-- @section
-->

The `main-health` workflow keeps one pinned "main is red" issue open while a required check
fails on `main`. Act on it at a fire, or at once on `MAIN-RED #<issue>` from any session
that saw it first.

1. Set `ops/mode.md` to `main: red #<issue>`.
2. **A flake or a runner failure:** re-run the job once. A flake may be quarantined for up
   to 24 hours, with a tracking issue (the **Flaky test** issue form), unless the maintainer
   turned that off at intake. Otherwise never skip, disable or quarantine a test.
3. **Revert first.** Find the merge that broke `main`: the oldest commit on `main` whose
   required checks fail. Open a revert PR as its author: `git revert <sha>` on a branch from
   `main`, titled `revert: <the original title>`, its body naming the issue and ending with
   your `Claude-Session:` line. Mark it ready and turn on auto-merge. You may revert any
   paths; a revert that touches a maintainer-only path still needs the maintainer's label,
   asked for once it is ready to land ([builder › Maintainer labels](builder.md#labels)).
   Fix forward instead only when the fix is smaller than the revert and already understood.
4. **Hold the rest.** Turn off auto-merge on every other open PR into `main` that has it on,
   and list each under `## Auto-merge off` in `ops/ops.md`. The revert or fix PR keeps
   auto-merge on, listed under `## Fixing main`.
5. Send the planner `ROW? <item>: reverted in #<pr>, <why>` for the item the revert took
   out, so it is rebuilt in a new PR.
6. **Once the issue closes:** turn auto-merge back on for each PR you listed, clear both
   lists, and set `ops/mode.md` to `main: green`.

<a id="serial-lanes"></a>
## Serial lanes ([serial-lanes runbook](../runbooks/serial-lanes.md))
<!-- @section
-->

A lane is a value only one PR at a time may change, such as a protocol or schema version or a
migration number.

**Claims.** Each lane is one line in `lanes.toml`:
`protocol = { value = 21, holder = 512, files = ["src/proto.rs"], pattern = 'PROTOCOL' }`.
Two claims on the same lane conflict in git. `lane-guard` (CI) passes only when `value` is
the base's plus one and `holder` is this PR, and fails a change to a lane's `files` with no
claim. A wave landing PR may advance a lane by more than one.

**How slots are handed out.**
1. The queue is `ops/lanes.md`, in priority order. A builder sends `LANE-READY` once its
   review passes. Until it holds the slot, its `lane-guard` may be red; that is expected.
2. **The slot goes to the first ready PR,** not the first in line.
3. **One holder per lane.** Send `LANE-HOLD <lane> <base+1> #<pr>`. Only the holder sets the
   value and turns on auto-merge. It may merge `main` and set the value in one push.
4. **The holder comes first.** When it conflicts, send its author `MERGE-MAIN #<pr>` with
   what conflicts and how to resolve it.
5. **When the holder merges** (its `MERGED` line, or the fire finds it), give the next ready
   PR the slot.
6. **A holder whose author hasn't moved for 2 fires** loses the slot. Send `STALLED #<pr>`
   to the coordinator for adoption, and give the slot to the next ready PR.

<a id="mergeability"></a>
## Mergeability
<!-- @section
-->

- **At each fire,** list open PRs whose review passed but which are `dirty`, and send each
  author `MERGE-MAIN #<pr>`. The author is the session in the PR body's `Claude-Session:`
  line. Count each one in `merge-mains today:` in `ops/ops.md`; the count decides
  [waves](#waves).
- **Reconcile from `main`** ([README › Lifecycle](README.md#lifecycle)): free the slot of
  every lane holder that merged, and update `ops/waves.md`, even when no `MERGED` came.
- **Silent authors.** An author whose PR passed review but didn't merge after two nudges goes
  to the coordinator as `STALLED`.

<a id="waves"></a>
## Waves ([waves runbook](../runbooks/waves.md))
<!-- @section
depends_on = ["RUNBOOK-WAVES#landing", "RUNBOOK-WAVES#when"]
-->

- **Turn them on** when, over the past day, you sent more than one `MERGE-MAIN` for every
  two merges into `main`, or runners queued for more than about 10 minutes:
  - create the wave branch from `main` and set `ops/mode.md` to waves on;
  - retarget the open PRs (a standing permission), except those that change protected
    paths;
  - send each moved PR's author `BASE #<pr> wave/<name>`.
- **While on:**
  - sync main into the wave through a sync PR merged as a **merge commit**. Once it merges,
    send `MERGE-MAIN #<pr>` only to the open PRs into the wave that are now `dirty`. A sync
    never sends `BASE`;
  - cut the wave: create the next wave from the cut wave's tip, move its open PRs onto it,
    and send each `BASE #<pr> wave/<next>`;
  - open the landing PR, with the trailers `scripts/repo progress --trailers
    origin/main..origin/wave/<name>` prints and every carried PR's "Governance change"
    section in its body;
  - fix what the landing PR's checks find yourself, with fix PRs into the cut wave, each
    listed under `fixes:` in `ops/waves.md` and in the landing PR's body. Builders end when
    their PR merges into the wave. Nothing else merges into a cut wave.
  - once it lands, GitHub deletes the wave branch; sync `main` into the next wave.
- **Turn them off** when the rate stays under the threshold for a day: land the last wave
  and set `ops/mode.md` to waves off.
- **Freeze.** While a lane holder or a landing merges, you may turn off auto-merge on other PRs
  in that wave (a standing permission), listing each under `## Auto-merge off`. Turn it back
  on once it lands.
- You're the author of the wave PRs you open: sync, landing and fix PRs
  ([README › Lifecycle](README.md#lifecycle)).

<a id="issues"></a>
## Issues and Dependabot
<!-- @section
-->

1. **Issues** opened or updated since `last fire:`:
   - close a duplicate with a link, or a won't-fix with a reason (agent tier, unless it
     changes product scope, which goes to the coordinator as `NEEDS-MAINTAINER`);
   - fix it yourself if it's small and inside no running row's `owns` paths;
   - otherwise send `ROUTE #<issue> → <row>` to that row's author, or `ROW? <issue title>` to
     the planner. On `DECLINED #<issue>`, take it back: fix it, route it elsewhere, or ask
     `ROW?`. The planner copies you its `ROW` or `NO-ROW`; note it against the issue.
2. **Dependabot.** You're the author session for Dependabot PRs:
   - subscribe to them and turn on auto-merge (squash);
   - fix their CI as if they were your own;
   - hold a bump that breaks the build in `dependabot.yml`'s `ignore` list, with a reason that
     names the PR it broke.
3. **The weekly sweep issue.** Take its actionable items (advisories, docs due for review)
   as small PRs.
4. **Hardware results** forwarded to you, because the item's author has ended: on a pass,
   open the record PR as the [builder brief](builder.md#hardware) describes, as its author; on
   a fail, send the planner `ROW? <item> failed on hardware`.

<a id="state"></a>
## State
<!-- @section
-->

`ops/ops.md`, `ops/mode.md`, `ops/lanes.md`, `ops/waves.md`, `ops/heads/ops.txt`. On a cold
start, rebuild them from open PRs, the "main is red" issue and `lanes.toml` on `main`.

<a id="never"></a>
## Never
<!-- @section
-->

> [!WARNING]
> Sync PRs land as merge commits. Never squash one, and never rewrite another session's
> branch.

- Change repo settings. The wave rulesets come from bootstrap.
- Rewrite history on someone else's branch.
- Squash a sync PR.
- Fix something inside another running row's `owns` paths. Route it.
- Launch threads.
