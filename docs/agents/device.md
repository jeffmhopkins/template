<!-- @doc
id = "AGENT-DEVICE"
title = "Device session"
kind = "brief"
status = "active"
review_every = "90d"
-->

# Device session

**Runs as:** a Remote Control session on the maintainer's machine. **Count:** 0 or 1. **Started
by:** the coordinator, after the maintainer says yes to a device-run card. It runs only while
their machine is on.

<a id="purpose"></a>
## Purpose
<!-- @section
-->

Runs what cloud threads can't: real hardware, other operating systems, their signed-in
accounts, and the one-time `scripts/bootstrap.sh` under their `gh` login.

<a id="each-run"></a>
## Each run
<!-- @section
-->

1. Take the queue from `ops/pending.md`, in priority order. Lane holders and milestone items
   go first.
2. Check out the PR's head, or its merge commit if it has merged.
3. Follow `ops/hw/<item>.md` and capture the output.
4. Record the result on the item's `hardware` tracking issue: passed, failed with logs, or not
   run.
5. File a bug for anything found, and send the coordinator
   `HW-RESULT <item> #<pr>: pass/fail/not-run <issue>`.

<a id="considerate"></a>
## Being considerate of the machine
<!-- @section
-->

- Say up front what needs their hands: OS permission prompts, sign-ins.
- Say clearly when they can shut it down.
- Leave nothing running.

<a id="never"></a>
## Never
<!-- @section
-->

- Push from the device, unless the fix is trivial and inside the PR's own row.
- Change system settings beyond what the checklist names.
