<!-- @doc
id = "ADR-0007"
title = "Swarm operating model at scale"
kind = "adr"
status = "accepted"
tier = "agent"
-->

# ADR-0007: Swarm operating model at scale

<!-- @generated begin adr-status -->
> **Status:** accepted
<!-- @generated end adr-status -->

> Roles wake from durable cron triggers into fresh sessions, rows and shards come from
> GitHub, review starts only on green CI and goes incremental, a red `main` is reverted
> first, and waves turn on by measured conflict. It supersedes ADR-0005.

<a id="context"></a>
## Context
The swarm is meant to run 12 builders and 2 or 3 reviewers. At that size several parts of
the first design stall or cost too much:

- Each role re-arms its own `send_later` chain. A wake that fires during a usage limit
  fails, the chain ends, and nothing wakes until the maintainer types. Standing threads
  also poll on a context that grows all day.
- A red `main` turns off auto-merge on every PR, including the one that fixes it.
- A push removes `governance-approved`, and nobody asks the maintainer again.
- Every head gets a full re-review, reviews start before CI is green, and review demand
  outgrows supply on the first day.
- The third reviewer and dead-reviewer rules need counts the coordinator never sees, and
  every reviewer start waits on the maintainer to edit `REVIEWER_SESSIONS`.
- `OPENED` and `MERGED` bring the coordinator about 100 wakes a day in its most expensive
  context, and it never checks the roster against GitHub.
- A wave sync makes every wave PR re-merge, waves turn on by open-PR count, and wave PRs
  skip `test`.
- `STOP` leaves auto-merge on, and pausing at the budget isn't possible.
- A private repo spends its Actions minutes fast: `deps` compiles cargo-deny on every run,
  and three governance jobs each build the tool.

A [merge queue](https://docs.github.com/en/repositories/configuring-branches-and-merges-in-your-repository/configuring-pull-request-merges/managing-a-merge-queue)
would test each PR on top of the ones ahead of it. GitHub offers it only on repositories
owned by an organization, and runs `merge_group` workflows from the queue's own commit.

<a id="decision"></a>
## Decision
**Wakes and sessions.**
- Every role's timed work runs from a durable platform cron trigger (`create_trigger` with
  a fresh session per fire), one trigger per role cadence. No role re-arms a `send_later`
  chain. A trigger keeps firing through a usage limit; a fire that fails is simply missed.
- A fired session reads its role's `ops/` state file. When the file names an earlier
  session that is still mid-turn, the fire ends at once. Otherwise the fire takes the role:
  it writes its own session id into the file, archives the earlier session, and acts.
  Messages to a role go to the session its state file names. A session that gets a line
  after it lost the role forwards the line, unchanged, to the session that holds it.
- The coordinator's check-in is a trigger that fires into the coordinator's own session.
  The coordinator is recycled daily, or when its context passes a threshold: it writes its
  state and carries on from `ops/` and GitHub alone, as on a cold start.
- The coordinator creates every trigger on day 0 and records their ids. `STOP` disables
  them, and resume enables them.

**Roles and messages.**
- One `ops` role replaces the integrator and triage: lanes, mergeability, waves, main's
  health, issues and Dependabot.
- `OPENED`, `REVIEWERS` and `IDLE` are dropped. A PR's row is the item in its body's
  `Completes:` or `Part-of:` line, or `#<pr>` without one; its author session is the
  `Claude-Session:` line in its body; its shard is its number mod the reviewer count in
  force at that number. The coordinator rebuilds the roster from GitHub at each check-in.
  `MERGED` goes to the planner and ops only, and `RECUSED` goes straight to the next shard's
  reviewer.
- Any session that sees the pinned "main is red" issue before `ops/mode.md` does sends ops
  `MAIN-RED #<issue>`, so ops reacts at once instead of at its next fire.

**Reviewers.**
- Reviewers have stable role names, `reviewer-0` to `reviewer-2`. The repo variable
  `REVIEWER_ROLES` lists them and bootstrap sets it once. A verdict marker carries
  `reviewer=<role>` and `reviewer-session=<session id>`. `review-gate` counts a marker only
  from a listed role, and ignores one whose session id is in a commit's `Claude-Session:`
  trailer. This replaces [ADR-0002](0002-review-gate-and-governance.md)'s clause on
  `REVIEWER_SESSIONS`; the rest of ADR-0002 stands. Role names buy no more security than
  session ids did: both are public ([ADR-0002](0002-review-gate-and-governance.md) lists the
  forgery and its audit).
- A builder sends `REVIEW?` only when every required check but `review-gate` is green on
  the head. Sweeps skip heads with a red check, and never review a draft unless asked.
- A round is incremental when `git range-diff` shows the earlier reviewed commits unchanged
  and only new commits on top: it reviews the new commits and every open finding. A full
  round runs on request, on a new PR, and after 3 incremental rounds in a row.
  [REVIEW.md](../../REVIEW.md#rounds) defines the kinds of round.
- The rule that round 5 needs the maintainer counts `fail` rounds only.
- Review latency is read from GitHub: a ready PR whose other required checks are green and
  whose `review-gate` has been pending for a time. It decides both when a third reviewer
  starts and when a reviewer is dead. A dead reviewer's session is interrupted and archived
  before a new one takes its role, so a state file never has two writers. State files carry
  the owning session id.

**Merging.**
- **Red `main`:** ops first reverts the merge that broke it, whatever paths it touched. The
  PR that fixes `main`, a revert or a fix, keeps auto-merge on; ops turns it off only on the
  other PRs.
- **Maintainer labels:** the maintainer is asked to apply a label only when the PR is
  ready to land at its final head, on a "ready to land" card: for `governance-approved`,
  once review has passed and CI is green; for `review-override`, once their override
  decision is recorded and every other required check is green. When a push removes the label, the author asks again for the new
  head. The coordinator keeps one card per PR and head SHA.
- **STOP** turns off auto-merge on every open PR and records which. Resume turns it back on
  for those. `STOP` is sent with priority `now`.
- **Budget:** builders check an `ops/PAUSE-builders` file at every wake. The session-hour
  proxy counts turn time, so a short poll costs seconds. The budget day has a fixed boundary
  and time zone, and a budget pause lifts at the boundary unless the maintainer said stop.

**Lanes and waves.** This replaces [ADR-0005](0005-waves-and-serial-lanes.md).
- A serial lane is one top-level line in `lanes.toml`:
  `<lane> = { value = <n>, holder = <pr> }`. A PR that changes it sets `value` to the base's
  plus one and `holder` to its own number. `lane-guard`, a required check, passes only on
  that shape; a wave's landing PR may advance a lane by more than one. Ops grants one slot
  per lane at a time, to the first ready PR, and only the holder turns on auto-merge.
- A lane PR is reviewed before it holds a slot, so `lane-guard` red before `LANE-HOLD` is
  expected and never a blocking finding. A hand-off may merge `main` and set the value in
  one push.
- `ops/launch.md` declares shared-append paths, such as lockfiles and module registration
  files, that any row may add to. `docs merge-main` resolves a conflict in a lockfile the
  stack pack names by taking the merged branch's side and running the pack's relock
  command.
- Work may land in batches through `wave/<name>` branches cut from `main`. PRs into a wave
  are reviewed as usual and run every check. Ops brings `main` in through sync PRs merged
  as merge commits, never squashed. After a sync, only the PRs left in conflict get
  `MERGE-MAIN`; a sync never sends `BASE`.
- The wave lands in `main` as one PR whose body carries the `Completes:`/`Part-of:` lines
  and "Governance change" sections of the PRs it carries. Ops fixes what the landing run
  finds; builders end when their PR merges into the wave.
- Waves turn on by measured cost, not by PR count: when, over a day, ops sends more than one
  `MERGE-MAIN` for every two merges, or runners queue for more than about 10 minutes. The
  `wave/*` ruleset comes from bootstrap.

**CI minutes.**
- `deps` installs a prebuilt, checksum-pinned cargo-deny, and runs only when a manifest,
  lockfile or the deny config changes. Otherwise it reports a passing check.
- `governance.yml` builds the base's tool once per run and shares it with its jobs.

**Merge queue: not now.** It stays off, and waves remain the answer to re-conflicts. The
design for later: `ci.yml` also runs on `merge_group`; `review-gate`, `governance`,
`pr-title` and `lane-guard` each gain a `merge_group` path that reads the PR numbers from the
queue branch name, and posts success on the queue commit only when the same check passed on
each PR's head; bootstrap adds a `merge_queue` rule to the `main` ruleset as an option. It
waits because personal-account repositories can't have a queue, and because `merge_group`
runs workflows from the queue commit, which holds the PR's own changes: the base-only
guarantee of `pull_request_target` doesn't carry over and needs its own review.

<a id="consequences"></a>
## Consequences

### Positive
- A usage limit costs one missed fire, not the whole swarm. Each fire starts from a short
  context.
- The coordinator wakes for launches and the maintainer, not for every PR.
- Reviewers spend rounds on green heads and on what changed. Starting or replacing a
  reviewer needs nothing from the maintainer.
- A red `main` clears with one revert, and a STOP stops merges.
- The maintainer is asked for a label once per final head, never for a head that will
  change.

### Negative
- Role state lives only in files and on GitHub. A fire that doesn't write its state before
  it ends loses its work.
- The minimum trigger interval, normally an hour, sets the slowest reaction; events and
  `MAIN-RED` cover what can't wait.
- A revert removes good work along with the break; its author redoes it in a new PR.
- An incremental round can miss an interaction between new and old commits; the full round
  every few rounds and on request bounds that.
- `main` can still go red when two green PRs merge together, until a merge queue is
  possible.
