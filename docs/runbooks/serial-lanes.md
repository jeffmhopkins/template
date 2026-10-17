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
[ADR-0005](../adr/0005-waves-and-serial-lanes.md); the file format and the check are in
[DOCS › Serial lanes](../DOCS.md#lanes). The integrator owns the queue
([integrator brief](../agents/integrator.md)), in `ops/lanes.md`.

<a id="declare"></a>
## Declaring a lane
<!-- @section
depends_on = ["DOCS#lanes"]
-->

A lane is added to `lanes.toml` by its own PR, at intake or when the planner names one,
with its current value and that PR as holder. The planner lists in each row the lanes it
changes. ADR numbers are not a lane: an ADR takes the next free number
([ADR index](../adr/README.md)).

<a id="builder"></a>
## The builder's side
<!-- @section
depends_on = ["RULES#R-GIT-006"]
-->

1. Build and review the change with the lane's value left as it is on the base. Keep
   auto-merge off.
2. Once review passes, send the integrator `LANE-READY <lane> #<pr>`.
3. On `LANE-HOLD <lane> <value> #<pr>`, set the lane's line to that value with your PR as
   holder, and set every file that names the value. Push once; the reviewer runs one short
   lane-value round.
4. Once that round passes, turn on auto-merge.
5. On `MERGE-MAIN #<pr>`, merge at once. The holder comes before anything else you are
   doing.

<a id="integrator"></a>
## The integrator's side
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
depends_on = ["ADR-0005"]
-->

While waves are on, lane PRs go into the current wave, and the integrator grants slots
against the wave's value. The landing PR advances each lane by the number of claims it
carries, which `lane-guard` allows on a landing PR.
