<!-- @doc
id = "HARDWARE"
title = "Hardware run records"
kind = "guide"
status = "active"
review_every = "180d"
-->

# Hardware run records

> One file per roadmap item that passed its run on the maintainer's hardware. Each file is
> added by the item's record PR, which carries `Verified-on-hardware: <item>` onto `main`.

<a id="flow"></a>
## How a run is recorded
<!-- @section
depends_on = ["DOCS#progress", "AGENT-BUILDER#hardware"]
-->

1. A PR with `hardware = true` merges at risk, linking a `hardware` tracking issue.
2. The device session runs `ops/hw/<item>.md` and records the result on that issue.
3. On a pass, the item's builder (or triage, if the builder has ended) opens the record PR:
   it adds `docs/hardware/<item>.md` and ends its body with
   `Verified-on-hardware: <item>` and `Closes #<issue>`.
4. Once it merges, `scripts/repo progress` shows the item as `done`. A record counts only
   after the item's latest `Completes:`: if the item is reverted and completed again, it
   needs a new run.

> [!NOTE]
> A merged record is never rewritten. A later run on the same item appends its own section
> (`run-2`, `run-3`) through its own record PR.

<a id="format"></a>
## Record format
<!-- @section
-->

````markdown
<!-- @doc
id = "HW-<item>"
title = "Hardware run: <item>"
kind = "guide"
status = "active"
-->

# Hardware run: <item>

> Passed on the maintainer's hardware for #<pr>, tracked in #<issue>.

<a id="run"></a>
## Run
<!-- @section
-->

- **Commit:** the full SHA the device session checked out
- **Machine:** OS, version, and the hardware the checklist needs
- **Result:** passed

| Step | What a pass looks like | Seen |
|---|---|---|
| … | … | … |

<details>
<summary>Output</summary>

```text
captured output, with tokens, keys and other secrets removed
```
</details>
````
