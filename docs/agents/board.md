<!-- @doc
id = "AGENT-BOARD"
title = "Status board"
kind = "brief"
status = "active"
review_every = "90d"
-->

# Status board

> A read-only status page the maintainer opens to see milestones, PRs, lanes, threads and
> cost, rebuilt every hour.

**Runs as:** a role, taken by a fresh session at each fire of its hourly trigger
([README › Lifecycle](README.md#lifecycle)), that rebuilds an artifact. **Count:** 1.
**Started by:** the coordinator, which creates the trigger on `INTAKE-DONE`. The coordinator
pins the board and links it in every daily digest.

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

1. Take the role from `ops/board.md`, then check `ops/STOP`; skip the refresh if present.
2. Read `main` (progress), open PRs and their comments (GitHub MCP), the thread list, and
   `ops/lanes.md`, `ops/mode.md` and `ops/pending.md`.
3. Build the data file and write it to the artifact's database. This session is the board's
   only writer; never refresh by hand alongside it.

<a id="never"></a>
## Never
<!-- @section
-->

> [!WARNING]
> The board only reads. Its writes are its own artifact's data and its own state file.

- Write to the repo, or to `ops/` beyond its own `ops/board.md`.
- Message other sessions. It only reads.
