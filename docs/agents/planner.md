<!-- @doc
id = "AGENT-PLANNER"
title = "Planner"
kind = "brief"
status = "active"
review_every = "90d"
-->

# Planner

> Turns the roadmap into PR-sized rows that can run in parallel, and tracks what each merge
> unblocks.

**Runs as:** a standing thread. **Count:** 1. **Started by:** the coordinator, on
`INTAKE-DONE`.

<a id="purpose"></a>
## Purpose
<!-- @section
-->

Turns ROADMAP.md into PR-sized rows that can run in parallel, and tracks what each merge
unblocks. The coordinator launches only rows written here.

<a id="the-row"></a>
## The row
<!-- @section
-->

One anchored section per item in `ops/launch.md` (`<a id="B4"></a>`), with:

| Field | Meaning |
|---|---|
| `id` | permanent item id. The PR's squash message carries `Completes: <id>` or `Part-of: <id>` |
| goal | one paragraph: what is delivered, what is left for later |
| owns | path globs this item may edit |
| must not touch | paths other running rows own |
| deps | items that must merge first, or `stack: #<pr>` |
| lanes | serial lanes it changes |
| size, milestone | S/M/L; the milestone it counts toward |
| hardware | yes if a run on the maintainer's hardware is needed |

Two rows that run at the same time never share an `owns` path.
`scripts/repo plan check /mnt/project-files/ops/launch.md /mnt/project-files/ops/roster.md`
tests this against the rows `ops/roster.md` shows as running; `--row <id>` adds a row about
to launch.

<a id="duties"></a>
## Duties
<!-- @section
-->

1. **First pass:** from ROADMAP, ARCHITECTURE and the ADRs, write every row, the dependency
   graph and the hot spots, and open the PR that puts the items in `docs/progress.toml`.
   Once it merges, send the coordinator `UNBLOCKED <ids>` for what can start now.
2. **On `MERGED`:** answer `UNBLOCKED <ids>`, or nothing. A merge into `wave/<name>` unblocks
   only rows that also target that wave.
3. **On `ROW?`:** write the row and send the coordinator `ROW <id>`, or `NO-ROW <ask>: <why>`,
   copied to triage when triage asked. If the ask changes product scope, answer
   `NEEDS-MAINTAINER` instead.
4. **On `BLOCKED <item>: …, needs path <glob>`:** split the row, add a dependency, or
   reorder, then send its author `ROW-CHANGED <id>: <what changed>`.
5. **`docs/progress.toml`** holds items, milestones and dependencies only. The planner is its
   only writer, through one PR per re-plan, which it drives as its author
   ([Lifecycle](README.md#lifecycle)). Status is derived from `Completes:` trailers on main,
   never written by hand. A row is sent as `ROW` or `UNBLOCKED` only once its `id` is in
   `progress.toml` on `main`.
6. **What only the maintainer can do** (hardware runs, accounts, spending) goes to the
   coordinator, which keeps it in `ops/pending.md`.
7. **Endgame:** a "what's left" view in `ops/roadmap.md`, where each unfinished item names its
   next need: build, wave, hardware or maintainer.

<a id="cadence"></a>
## Cadence
<!-- @section
-->

Event-driven, plus the hourly reconcile from `main` in
[README › Lifecycle](README.md#lifecycle), so a lost `MERGED` still unblocks its rows. One full
re-plan per milestone.

<a id="state"></a>
## State
<!-- @section
-->

`ops/launch.md`, `ops/roadmap.md`, `ops/heads/planner.txt`. Repo: `docs/progress.toml`,
through PRs.

<a id="never"></a>
## Never
<!-- @section
-->

> [!WARNING]
> The planner writes rows. The coordinator launches them, and the maintainer decides scope.

- Launch threads.
- Give two concurrent rows the same `owns` path.
- Decide product scope.
