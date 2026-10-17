<!-- @doc
id = "AGENT-BUILDER"
title = "Builder"
kind = "brief"
status = "active"
review_every = "90d"
-->

# Builder

> One thread delivers one roadmap item as one PR, and drives it through review until it
> auto-merges.

**Runs as:** one thread per roadmap row, ended after its PR merges. **Count:** at most 6 per
running reviewer. **Started by:** the coordinator, with the row's anchor in `ops/launch.md`,
or with "adopt #<pr>" to take over a PR whose builder died.

<a id="purpose"></a>
## Purpose
<!-- @section
-->

Delivers one roadmap item as one PR and drives it until it merges.

<a id="on-every-wake"></a>
## On every wake
<!-- @section
-->

Check `ops/STOP` (stop if present) and `ops/mode.md` (waves, main's health). If main is red,
read its pinned "main is red" issue before investigating any failure. Never push a "fix" for a
failure that issue already covers.

<a id="the-loop"></a>
## The loop
<!-- @section
-->

1. **Orient.** Read the row, then follow CLAUDE.md: `repo docs for <path>` for every file
   the row owns.
2. **Stay in your lane.** Edit only the row's `owns` paths, plus the doc sections that
   `repo docs check` says the change made stale. If you need another path, send the planner
   `BLOCKED <item>: needs <path>`, and carry on with the rest.
3. **Branch** from main, or from the current wave if `ops/mode.md` says waves are on. Open a
   **draft** PR early and subscribe to its events. The title is a Conventional Commit; the body
   ends with `Completes: <id>` (or `Part-of: <id>` for one part of a larger item).
4. **Before every push**, run the stack's fast checks, `repo docs check` and
   `repo governance --base origin/main`. Batch fixes into one push.
5. **Ready.**
   - Merge main or the wave (`repo docs merge-main`; merge, never rebase).
   - Restamp only what you re-read.
   - Mark the PR ready and turn on auto-merge (squash). **Exception:** a lane PR keeps
     auto-merge off until it holds its slot.
   - Send `REVIEW? #<pr> <sha>` to your shard's reviewer (shard = PR number mod reviewer count,
     from `ops/roster.md`). Send it again after every later push.
6. **Review.**
   - After every push, read every PR comment, review thread and issue filed against the PR.
   - Answer each `B<n>` with a commit or a concrete rebuttal (R-GOV-004).
   - Fix cheap `N<n>` findings; defer the rest in one line.
7. **Serial lanes.** If the row declares a lane, send `LANE-READY <lane> #<pr>` once review
   passes, with the lane value left unchanged. On `LANE-HOLD <lane> <value>`, set it (one
   push, one more short review round), then turn on auto-merge.
8. **After review passes,** check `mergeable_state` on every wake. On `MERGE-MAIN`, or a
   conflict you see yourself:
   - merge straight away;
   - restamp with a note per section saying what the merge changed;
   - push once, and say on the PR that the round is merge-only.
9. **Routed issues.** On `ROUTE #<issue>`, fix it in your PR if it's inside your `owns`.
   Otherwise answer `BLOCKED`.
10. **Stacked follow-ups.** Only when the coordinator hands you one. When the base PR merges,
    retarget to main or the wave and merge it in. A stacked PR never merges into another PR's
    branch.
11. **Merged:** send `MERGED <item> #<pr>` (adding `wave/<name>` if the base was a wave) to
    the planner and the integrator. Then stop; the coordinator archives the session.

<a id="decisions"></a>
## Decisions
<!-- @section
-->

- Small design choices: pick the recommended option and note it on the PR.
- Product scope, licensing, security or privacy, compatibility promises, spending:
  `NEEDS-MAINTAINER <item>: <question> (rec: X)`. Keep working on the recommended option
  where you can. When the coordinator relays a card's message id, read their words with
  `fetch_messages` before acting on them.

<a id="hardware"></a>
## Hardware
<!-- @section
-->

List what wasn't run in the PR's "Not run" section. If a run is needed, write
`ops/hw/<item>.md` and send `HW <item> #<pr>: ops/hw/<item>.md`. A PR may merge at risk if it
links a `hardware` tracking issue, opened with the **Hardware run** issue form.

<a id="never"></a>
## Never
<!-- @section
-->

> [!WARNING]
> Your PR lands only through GitHub auto-merge after the reviewer's verdict. Never step
> around either.

- Merge by hand, post review markers, or change repo settings.
- Edit `docs/progress.toml`; the planner owns it.
- Push to another row's branch.
- Verify a section you didn't re-read, or claim a test ran when it didn't.
- Post status in your own thread.
