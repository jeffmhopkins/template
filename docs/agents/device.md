<!-- @doc
id = "AGENT-DEVICE"
title = "Device session"
kind = "brief"
status = "active"
review_every = "90d"
-->

# Device session

> Runs what cloud threads can't, on the maintainer's own machine and only while it is on:
> hardware checks, other operating systems, signed-in accounts and bootstrap.

**Runs as:** a Remote Control session on the maintainer's machine. **Count:** 0 or 1. **Started
by:** the coordinator, after the maintainer says yes to a device-run card, or asks for
bootstrap in the chat. It runs only while their machine is on.

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

This session can't read `/mnt/project-files/ops/`, so everything it needs comes from its
start instructions and from GitHub.

1. Take the queue from your start instructions: one line per item, `<item> #<pr> #<issue>`,
   in priority order. Lane holders and milestone items go first.
2. Check out the PR's head, or its merge commit if it has merged.
3. Read the checklist on the tracking issue (`gh issue view <issue>`), follow it, and
   capture the output.
4. Record the result on the item's `hardware` tracking issue: passed, failed with logs, or not
   run.
5. File a bug for anything found, and send the coordinator
   `HW-RESULT <item> #<pr>: pass/fail/not-run <issue>`. Leave the issue open: a pass is
   recorded on `main` by the builder's record PR, which closes it.

<a id="bootstrap"></a>
## Bootstrap
<!-- @section
depends_on = ["RUNBOOK-BOOTSTRAP#run"]
-->

When the coordinator starts this session for bootstrap, follow the
[bootstrap runbook](../runbooks/bootstrap.md#run) under the maintainer's `gh` login:

1. Say what you'll do: change the repo's GitHub settings with `scripts/bootstrap.sh`, in a
   folder of their choice.
2. Check the tools: `git --version`, `gh auth status` and `python3 --version` (3.11 or
   later). If one is missing, give them the install line for their system from
   [Getting started](../GETTING-STARTED.md#before), and wait. If `gh auth status` says they
   aren't logged in, ask them to run `gh auth login` in a terminal on this computer, choosing
   GitHub.com, HTTPS and the web browser login, and wait. It needs their hands, so never run
   it for them. Then check `gh auth status` again.
3. Clone the repo with `gh repo clone OWNER/REPO`, or use their existing clone, and change
   into it. Pull `main` first if the clone already existed.
4. Run `scripts/bootstrap.sh`. Exit 2 means it stopped before changing anything: pass on its
   message as it is, and stop.
5. Run `scripts/bootstrap.sh --check`. It must end with `All settings match.`
6. Send the coordinator `BOOTSTRAP-RESULT ok/stopped/failed: <the last line, or the stop
   message>`, and tell them they can close the folder. Leave the clone in place.

The same procedure re-runs bootstrap after intake's design PR merges. Change nothing beyond
what the script does.

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

> [!WARNING]
> This is the maintainer's own machine. Do only what the checklist names.

- Push from the device, unless the fix is trivial and inside the PR's own row.
- Change system settings beyond what the checklist names.
- Run bootstrap unless the maintainer asked for it, or edit a repo setting by hand.
