<!-- @doc
id = "AGENT-BOARD"
title = "Status board"
kind = "brief"
status = "active"
review_every = "90d"
-->

# Status board

**Runs as:** a small standing thread on a cheap model that rebuilds an artifact hourly
(`send_later` after each run). It runs as a scheduled task instead only if the dry run proves
one triggered session can do all four reads and the write below. **Count:** 1. **Started by:**
the coordinator, on `INTAKE-DONE`. The coordinator pins the board and links it in every daily
digest.

<a id="purpose"></a>
## Purpose
<!-- @section
-->

One page the maintainer can open to see the whole project without asking.

<a id="what-it-shows"></a>
## What it shows
<!-- @section
-->

- **Milestone progress,** derived from `Completes:` trailers on `main`. Wave merges show as
  "in wave", not done.
- **Every open PR:** item, draft or ready, review verdict and round, failing checks,
  `mergeable_state`, lane slot.
- **Lanes** and their holders; **waves** and what each carries; whether main is red.
- **Threads:** working, idle or blocked, and what each needs.
- **Needs the maintainer:** from `ops/pending.md`, linked.
- **Cost:** the same figures as the daily digest.

<a id="refresh"></a>
## Refresh
<!-- @section
-->

1. Check `ops/STOP`; skip the refresh if present.
2. Read `main` (progress), open PRs and their comments (GitHub MCP), the thread list, and
   `ops/lanes.md`, `ops/mode.md` and `ops/pending.md`.
3. Build the data file and write it to the artifact's database. This session is the board's
   only writer; never refresh by hand alongside it.

<a id="never"></a>
## Never
<!-- @section
-->

- Write to the repo or to `ops/`.
- Message other sessions. It only reads.
