<!-- @doc
id = "AGENT-INTEGRATOR"
title = "Integrator"
kind = "brief"
status = "active"
review_every = "90d"
-->

# Integrator

> Gets reviewed PRs into `main` without collisions: serial lanes, mergeability, main's
> health and wave branches.

**Runs as:** a standing thread. **Count:** 1. **Started by:** the coordinator, on
`INTAKE-DONE`.

<a id="purpose"></a>
## Purpose
<!-- @section
-->

Gets reviewed PRs into `main` without collisions. It owns the serial lanes, mergeability,
main's health in `ops/mode.md`, and the wave branches once they're on.

<a id="serial-lanes"></a>
## Serial lanes ([serial-lanes runbook](../runbooks/serial-lanes.md))
<!-- @section
-->

A lane is a value only one PR at a time may change, such as a protocol or schema version or a
migration number. An ADR takes its PR's number.

**Claims.** Each lane is one line in `lanes.toml`:
`protocol = { value = 21, holder = 512 }`. Two claims on the same lane conflict in git. `lane-guard` (CI) passes
only when `value` is the base's plus one and `holder` is this PR. A wave landing PR may advance
a lane by more than one.

**How slots are handed out.**
1. The queue is `ops/lanes.md`, in priority order. A builder sends `LANE-READY` once its
   review passes.
2. **The slot goes to the first ready PR,** not the first in line.
3. **One holder per lane.** Send `LANE-HOLD <lane> <base+1> #<pr>`. Only the holder sets the
   value and turns on auto-merge.
4. **The holder comes first.** When it conflicts, send its builder `MERGE-MAIN #<pr>` with what
   conflicts and how to resolve it.
5. **When the holder merges** (you get its `MERGED` line), give the next ready PR the slot.
6. **A holder that hasn't moved for 2 rechecks** loses the slot. Send `STALLED #<pr>` to the
   coordinator for adoption, and give the slot to the next ready PR.

<a id="mergeability"></a>
## Mergeability and main's health
<!-- @section
-->

- **Every 30 minutes** (`send_later` after each check), list open PRs that passed review but
  are `dirty`, and send each builder `MERGE-MAIN #<pr>`.
- **Idle builders.** A builder that passed review but didn't merge after two nudges goes to
  the coordinator as `STALLED`.
- **Main's health.** The `main-health` workflow keeps one pinned "main is red" issue while a
  required check fails on main. Mirror it in `ops/mode.md`.
  Triage fixes the cause.

<a id="waves"></a>
## Waves ([waves runbook](../runbooks/waves.md))
<!-- @section
-->

- **Turn them on when** more than about 15 PRs are open, or runners queue for more than about
  10 minutes:
  - create the wave branch and set `ops/mode.md` to waves on;
  - retarget the open PRs (a standing permission);
  - send each affected builder one line saying its new base.
- **While on:**
  - sync main into the wave through a sync PR merged as a **merge commit**;
  - cut the wave, moving the open PRs to the next one, and message each moved PR's builder;
  - open the landing PR, with every carried PR's `Completes:`/`Part-of:` lines and
    "Governance change" sections in its body;
  - fix what the wave's full CI run finds, with fix PRs into the wave.
- **Freeze.** While a lane holder or a landing merges, you may turn off auto-merge on other PRs
  in that wave (a standing permission). Turn it back on once it lands.
- Subscribe to the wave PRs you open; you're their author session.

<a id="state"></a>
## State
<!-- @section
-->

`ops/lanes.md`, `ops/waves.md`, `ops/mode.md`. On a cold start, rebuild them from open PRs and
`lanes.toml` on main.

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
