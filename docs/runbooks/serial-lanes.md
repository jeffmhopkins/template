<!-- @doc
id = "RUNBOOK-LANES"
title = "Serial lanes runbook"
kind = "runbook"
status = "active"
review_every = "90d"
-->

# Serial lanes runbook

> How a PR waits for, holds and gives up the slot that lets it set the next value of a
> serial lane.

How a PR gets the next value of a serial lane. The decision is
[ADR-0007](../adr/0007-swarm-operating-model.md); the file format and the check are in
[DOCS › Serial lanes](../DOCS.md#lanes). Ops owns the queue
([ops brief](../agents/ops.md#serial-lanes)), in `ops/lanes.md`.

<a id="declare"></a>
## Declaring a lane
<!-- @section
depends_on = ["DOCS#lanes"]
-->

A lane is added to `lanes.toml` by its own PR, at intake or when the planner names one,
with its current value and that PR as holder. It also names where the value lives, so a
change there can't slip past `lane-guard` without a claim:

```toml
protocol = { value = 21, holder = 512, files = ["src/proto/version.rs"], pattern = 'PROTOCOL_VERSION' }
migration = { value = 7, holder = 498, files = ["migrations/**"] }
```

- `files`: globs of the files that hold the value. A PR that changes them without changing
  the lane's line fails `lane-guard`.
- `pattern`: optional. A regular expression for the lines that hold the value; only an added
  or removed line that matches it counts. Without it, any change to the files counts, as
  suits a directory of numbered migrations.
- `lane-guard` reads the lanes from the PR's base, so a PR can't narrow `files` to pass its
  own check. Changing `files` or `pattern` is itself a claim on the lane.

The planner lists in each row the lanes it changes. ADR numbers are not a lane: an ADR takes the next free number
([ADR index](../adr/README.md)).

<a id="builder"></a>
## The builder's side
<!-- @section
depends_on = ["RULES#R-GIT-006"]
-->

1. Build and review the change with the lane's value left as it is on the base, in
   `lanes.toml` and in the lane's `files`. Keep auto-merge off. Until you hold the slot,
   `lane-guard` may fail on the lane's files; that is expected, and the reviewer doesn't
   count it as a blocking finding.
2. Once review passes, send ops `LANE-READY <lane> #<pr>`.
3. On `LANE-HOLD <lane> <value> #<pr>`, set the lane's line to that value with your PR as
   holder, and set every file that names the value. If the branch fell behind, merge `main`
   in the same push. Push once; the reviewer runs one short lane-value round, or a
   merge-only round that also checks the value.
4. Once that round passes, turn on auto-merge.
5. On `MERGE-MAIN #<pr>`, merge at once. The holder comes before anything else you are
   doing.

<a id="integrator"></a>
## Ops's side
<!-- @section
depends_on = ["DOCS#lanes"]
-->

1. Keep each lane's queue in `ops/lanes.md`, in priority order.
2. **The slot goes to the first ready PR**, not the first in line: a PR is ready once its
   review passed and everything it waits on has merged.
3. **One holder per lane.** Send `LANE-HOLD <lane> <base+1> #<pr>`.
4. When the holder conflicts, send its builder `MERGE-MAIN #<pr>` with what conflicts and
   how to resolve it.
5. When the holder merges, give the next ready PR the slot.
6. A holder that hasn't moved for 2 rechecks loses the slot. Send the coordinator
   `STALLED #<pr>` and give the slot to the next ready PR.

<a id="waves"></a>
## Lanes and waves
<!-- @section
depends_on = ["ADR-0007"]
-->

While waves are on, lane PRs go into the current wave, and ops grants slots
against the wave's value. The landing PR advances each lane by the number of claims it
carries, which `lane-guard` allows on a landing PR.
