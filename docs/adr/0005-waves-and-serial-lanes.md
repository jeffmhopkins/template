<!-- @doc
id = "ADR-0005"
title = "Wave branches and serial lanes"
kind = "adr"
status = "superseded"
tier = "agent"
superseded_by = "ADR-0007"
-->

# ADR-0005: Wave branches and serial lanes

<!-- @generated begin adr-status -->
> **Status:** superseded by [ADR-0007](0007-swarm-operating-model.md)
<!-- @generated end adr-status -->

> Serial lanes let one PR at a time change a sequenced value, and wave branches batch PRs so
> they stop re-conflicting on `main`.

<a id="context"></a>
## Context
With many PRs open, every merge into `main` re-conflicts the others, and each re-merge
costs a CI run and a review round. Some values, such as a protocol version or a migration
number, may be changed by only one PR at a time, and two PRs that set the same new value
merge without a git conflict.

<a id="decision"></a>
## Decision
**Serial lanes.**
- Each lane is one top-level line in `lanes.toml`: `<lane> = { value = <n>, holder = <pr> }`.
- A PR that changes a lane sets `value` to the base's plus one and `holder` to its own
  number. Two claims on the same lane conflict in git.
- `lane-guard` (a required check) passes only when every changed lane has that shape. A
  wave landing PR may advance a lane by more than one.
- The integrator grants one slot per lane at a time, to the first ready PR. Only the holder
  turns on auto-merge ([serial-lanes runbook](../runbooks/serial-lanes.md)).

**Waves.**
- Work may land in batches through `wave/<name>` branches, cut from `main`.
- PRs into a wave are reviewed as usual and skip the heavy CI jobs; every other check runs.
  They squash-merge into the wave once `review-gate` passes.
- The integrator brings `main` into the wave through sync PRs that land as merge commits,
  never squashed.
- The wave lands in `main` as one PR that runs every check. Its body carries the
  `Completes:`/`Part-of:` lines and "Governance change" sections of the PRs it carries.
- Waves are off until about 15 PRs are open, or runners queue. The `wave/*` ruleset is
  created at bootstrap.

<a id="consequences"></a>
## Consequences

### Positive
- Lane values reach `main` in the order they were granted.
- Inside a wave, PRs conflict only with each other. Heavy jobs run once per wave.
- PRs straight into `main` work the same whether waves are on or off.

### Negative
- A lane PR waits for its slot.
- A failure the full run finds surfaces at the landing PR and blocks the wave until it is
  fixed there.
