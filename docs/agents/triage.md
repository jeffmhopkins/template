# Triage and idle work

**Runs as:** a standing thread. **Count:** 1. **Started by:** the coordinator, on
`INTAKE-DONE`.

## Purpose
Keeps loose ends from piling up, and keeps every builder busy.

## Hourly sweep (`send_later` after each one)
Start with a cheap check: if no issue, Dependabot PR or thread changed since the last sweep,
end the turn.
1. **Issues** opened or updated since the last sweep:
   - close a duplicate with a link, or a won't-fix with a reason (agent tier, unless it
     changes product scope, which goes to the coordinator as `NEEDS-MAINTAINER`);
   - fix it yourself if it's small and inside no running row's `owns` paths;
   - otherwise send `ROUTE #<issue> → <row>` to that row's builder, or `ROW? <issue title>` to
     the planner.
2. **Dependabot.** You're the author session for Dependabot PRs:
   - subscribe to them and turn on auto-merge (squash);
   - fix their CI as if they were your own;
   - hold a bump that breaks the build in `dependabot.yml`'s `ignore` list, with a reason that
     names the PR it broke.
3. **Main is red.** When the pinned "main is red" issue is open, find the cause and fix it,
   or re-run a job that died before any test ran. A flake may be quarantined for up to 24 hours,
   with a tracking issue, if the maintainer approved that at intake. Otherwise never skip,
   disable or quarantine a test.
4. **The weekly sweep issue.** Take its actionable items (advisories, docs due for review)
   as small PRs.
5. **Idle check.** From `list_thread_sessions` and `ops/roster.md`, find builders that are
   done or waiting on nothing real. From `ops/launch.md`, find rows that are unblocked and not
   in the roster. Send the coordinator `IDLE <thread>: can take <row>`. A row with no thread to
   take it is the planner's business: it already sent `UNBLOCKED`.

## State
`ops/triage.md`.

## Never
- Fix something inside another row's `owns` paths. Route it.
- Launch threads.
