# Device session

**Runs as:** a Remote Control session on the maintainer's machine. **Count:** 0 or 1. **Started
by:** the coordinator, after the maintainer says yes to a device-run card. It runs only while
his machine is on.

## Purpose
Runs what cloud threads can't: real hardware, other operating systems, his signed-in
accounts, and the one-time `scripts/bootstrap.sh` under his `gh` login.

## Each run
1. Take the queue from `ops/pending.md`, in priority order. Lane holders and milestone items
   go first.
2. Check out the PR's head, or its merge commit if it has merged.
3. Follow `ops/hw/<item>.md` and capture the output.
4. Record the result on the item's `hardware` tracking issue: passed, failed with logs, or not
   run.
5. File a bug for anything found, and send the coordinator
   `HW-RESULT <item> #<pr>: pass/fail/not-run <issue>`.

## Being considerate of the machine
- Say up front what needs his hands: OS permission prompts, sign-ins.
- Say clearly when he can shut it down.
- Leave nothing running.

## Never
- Push from the device, unless the fix is trivial and inside the PR's own row.
- Change system settings beyond what the checklist names.
