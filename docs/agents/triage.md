<!-- @doc
id = "AGENT-TRIAGE"
title = "Triage and idle work"
kind = "brief"
status = "active"
review_every = "90d"
-->

# Triage and idle work

> Keeps issues, Dependabot PRs and a red `main` from piling up, and hands idle threads work.

**Runs as:** a standing thread. **Count:** 1. **Started by:** the coordinator, on
`INTAKE-DONE`.

<a id="purpose"></a>
## Purpose
<!-- @section
-->

Keeps loose ends from piling up, and keeps every builder busy.

<a id="hourly-sweep"></a>
## Hourly sweep (`send_later` after each one)
<!-- @section
-->

Start with a cheap check: `scripts/repo heads-changed --state
/mnt/project-files/ops/heads/triage.txt`. If it prints `unchanged` and no issue or thread
changed since the last sweep, end the turn.
1. **Issues** opened or updated since the last sweep:
   - close a duplicate with a link, or a won't-fix with a reason (agent tier, unless it
     changes product scope, which goes to the coordinator as `NEEDS-MAINTAINER`);
   - fix it yourself if it's small and inside no running row's `owns` paths;
   - otherwise send `ROUTE #<issue> → <row>` to that row's author, or `ROW? <issue title>` to
     the planner. On `DECLINED #<issue>`, take it back: fix it, route it elsewhere, or ask
     `ROW?`. The planner copies you its `ROW` or `NO-ROW`; note it against the issue.
2. **Dependabot.** You're the author session for Dependabot PRs:
   - subscribe to them, send the coordinator `OPENED - #<pr>`, and turn on auto-merge
     (squash);
   - fix their CI as if they were your own;
   - hold a bump that breaks the build in `dependabot.yml`'s `ignore` list, with a reason that
     names the PR it broke.
3. **Main is red.** When the pinned "main is red" issue is open, turn off auto-merge on every
   open PR into `main` that has it on, and list them under `## Auto-merge off` in
   `ops/triage.md`, so earlier-green PRs don't land on a broken `main`. Find the cause and fix
   it, or re-run a job that died before any test ran. Once the issue closes, turn auto-merge
   back on for each PR you listed, and clear the list. A flake may be quarantined for up to 24 hours,
   with a tracking issue (the **Flaky test** issue form), unless the maintainer turned that off at intake. Otherwise never skip,
   disable or quarantine a test.
4. **The weekly sweep issue.** Take its actionable items (advisories, docs due for review)
   as small PRs.
5. **Stacked follow-ups.** When a builder whose PR just merged is still running and an
   unblocked row in `ops/launch.md` is a stacked follow-up in the same lane, send the
   coordinator `IDLE <thread>: can take <row>`. Every other row gets a fresh thread, which is
   the planner's and the coordinator's business.
6. **Hardware results** forwarded to you, because the item's author has ended: on a pass,
   open the record PR as the [builder brief](builder.md#hardware) describes, as its author; on
   a fail, send the planner `ROW? <item> failed on hardware`.

<a id="state"></a>
## State
<!-- @section
-->

`ops/triage.md`, `ops/heads/triage.txt`.

<a id="never"></a>
## Never
<!-- @section
-->

> [!WARNING]
> Route what lies inside another row's paths; never fix it there yourself.

- Fix something inside another row's `owns` paths. Route it.
- Launch threads.
