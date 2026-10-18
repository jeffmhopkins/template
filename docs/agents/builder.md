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

Check `ops/STOP` (stop if present), `ops/PAUSE-builders` (while it is present, end the turn
without acting) and `ops/mode.md` (waves, main's health).

If main is red, read its pinned "main is red" issue before investigating any failure. Never
push a "fix" for a failure that issue already covers. If `ops/mode.md` still says
`main: green`, send ops `MAIN-RED #<issue>` once. While the issue is open, ops may turn off
auto-merge on your PR, and you don't turn it on: ops does that once `main` is green.

<a id="the-loop"></a>
## The loop
<!-- @section
-->

1. **Orient.** Read the row, then follow CLAUDE.md: `scripts/repo docs for <path>` for every
   file the row owns.
2. **Stay in your lane.** Edit only the row's `owns` paths, plus the doc sections that
   `scripts/repo docs check` says the change made stale. Any row may also append to the
   shared-append paths in `ops/launch.md#shared`, such as a lockfile or a module list: add
   lines, never change or remove another row's. If you need another path, send the
   planner `BLOCKED <item>: <what>, needs path <glob>`, and carry on with the rest. On
   `ROW-CHANGED`, re-read the row.
3. **Branch** from main, or from the current wave if `ops/mode.md` says waves are on, unless
   the change touches a protected path ([waves runbook](../runbooks/waves.md#which-wave)).
   Open a **draft** PR early and subscribe to its events. The title is a Conventional
   Commit; the body ends with one trailer block: `Completes: <id>` (or `Part-of: <id>` for
   one part of a larger item), then `Claude-Session: <your session URL>`. Those two lines
   are how every other role finds the PR's row and its author; nobody needs a message.
4. **Before every push**, run the stack's fast checks, `scripts/repo docs check` and
   `scripts/repo governance --base origin/main` (for a PR into a wave,
   `--base origin/wave/<name> --into wave/<name>`). Batch fixes into one push.
5. **Ready.**
   - Merge main or the wave (`scripts/repo docs merge-main`, with `--from wave/<name>` for a
     wave; merge, never rebase).
   - Restamp only what you re-read.
   - Mark the PR ready and turn on auto-merge (squash). **Exception:** a lane PR keeps
     auto-merge off until it holds its slot.
   - Once every required check but `review-gate` is green at the head, send
     `REVIEW? #<pr> <sha>` to your shard's reviewer. Your shard is your row's `shard` in
     `ops/roster.md`; until the roster lists it, the PR number mod the reviewer count that
     the roster's `shards:` line gives for it. The reviewer is the `session:` line of
     `ops/review/state-<shard>.md`. Send it again after every later push, once that head is
     green too. Two checks may be red and still count as green here: a lane PR's
     `lane-guard` until it holds the slot, and, on a PR that changes a maintainer-only path,
     `governance` while its log says `governance: only the maintainer's label is missing`
     ([Maintainer labels](#labels)).
6. **Review.**
   - After every push, read every PR comment, review thread and issue filed against the PR.
   - Answer each `B<n>` with a commit or a concrete rebuttal (R-GOV-004), in one
     `Answers to round <n>` comment per push
     ([steward › Review findings](../../.claude/skills/steward/SKILL.md#findings)). A
     rebuttal shows evidence the reviewer can check at the head: a code path with
     `file:line`, a test CI ran, or a rule or doc's text. When you rebut without pushing,
     send `REVIEW? #<pr> <sha>` for the same head, so the reviewer judges the reply.
   - Fix cheap `N<n>` findings; defer the rest in their line of that comment.
7. **Serial lanes.** If the row declares a lane, send `LANE-READY <lane> #<pr>` once review
   passes, with the lane value left unchanged. On `LANE-HOLD <lane> <value>`, set it, merging
   `main` in the same push if it fell behind (one push, one more short review round), then
   turn on auto-merge.
8. **After review passes,** ask for a maintainer label now if the PR needs one
   ([Maintainer labels](#labels)), and check `mergeable_state` on every wake. On
   `MERGE-MAIN`, `BASE`, or a conflict you see yourself:
   - merge straight away;
   - restamp with a note per section saying what the merge changed;
   - push once, and say on the PR that the round is merge-only.
9. **Routed issues.** On `ROUTE #<issue>`, fix it in your PR if it's inside your `owns`.
   Otherwise answer ops `DECLINED #<issue>: <why>`.
10. **Stacked follow-ups.** Only when the coordinator hands you one. When the base PR merges,
    retarget to main or the wave and merge it in. A stacked PR never merges into another PR's
    branch. Until it targets `main` or a wave, its `governance` and `review-gate` checks fail
    closed, since they would run from the unreviewed base branch
    ([REVIEW › Ground rules](../../REVIEW.md#ground-rules)). Keep it a draft until then, and
    send `REVIEW?` once it is retargeted and green.
11. **Merged:** send `MERGED <item> #<pr>` (adding `wave/<name>` if the base was a wave) to
    the planner and ops. Then stop; the coordinator archives the session at its next
    check-in.

<a id="decisions"></a>
## Decisions
<!-- @section
-->

- Small design choices: pick the recommended option and note it on the PR.
- Product scope, licensing, security or privacy, compatibility promises, spending:
  `NEEDS-MAINTAINER <item>: <question> (rec: X)`. Keep working on the recommended option
  where you can. When the coordinator relays a card's message id, read their words with
  `fetch_messages` before acting on them.

<a id="labels"></a>
## Maintainer labels
<!-- @section
-->

A PR that changes a maintainer-only path needs `governance-approved` from the maintainer, and
a PR the maintainer chose to override needs `review-override` from them. Ask only when the PR
is ready to land at its head: for `governance-approved`, review passed and every other
required check is green but `governance`, whose log says only that label is missing; for
`review-override`, their override decision is recorded and every required check but
`review-gate` is green. Until then that red `governance` is expected: it doesn't hold back
`REVIEW?`, and the reviewer doesn't count the missing label as a finding. Then send `NEEDS-MAINTAINER #<pr> <sha>: ready to land, apply <label> (rec: apply)`, with the full
head SHA. A push removes the label, so after any later push, once that head has passed review
and is green, send the line again with the new SHA. The coordinator keeps one card per PR and
head.

<a id="hardware"></a>
## Hardware
<!-- @section
-->

List what wasn't run in the PR's "Not run" section. If a run is needed, open a `hardware`
tracking issue with the **Hardware run** issue form, with the checklist in it: each step and
what a pass looks like. The device session reads it there, since it can't read `ops/`. Link
the issue from the PR and send `HW <item> #<pr>: #<issue>`. A PR may merge at risk once it
links that issue.

On `HW-RESULT <item> #<pr>: pass <issue>`, open the **record PR**: one small PR that adds
`docs/hardware/<item>.md` in the [record format](../hardware/README.md#format) (the
checklist, the result and the commit it ran on, copied from the tracking issue) and ends its
body with `Verified-on-hardware: <item>` and `Closes #<issue>`.
Usually the PR has merged and you have ended, so ops gets the result instead. On `fail`,
fix it in a new PR and send `HW` again. On `not-run`, leave the issue open; the coordinator
queues it again.

<a id="never"></a>
## Never
<!-- @section
-->

> [!WARNING]
> Your PR lands only through GitHub auto-merge after the reviewer's verdict. Never step
> around either.

- Merge by hand, post review markers, or change repo settings.
- Apply `review-override`, or `governance-approved` on a PR that changes a maintainer-only
  path. Only the maintainer applies those ([CLAUDE › Limits](../../CLAUDE.md#limits)).
- Commit without your `Claude-Session:` trailer, or open a PR whose body doesn't end with it.
- Edit `docs/progress.toml`; the planner owns it.
- Push to another row's branch.
- Verify a section you didn't re-read, or claim a test ran when it didn't.
- Post status in your own thread.
