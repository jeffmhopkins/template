<!-- @doc
id = "RUNBOOK-BOOTSTRAP"
title = "Bootstrap runbook"
kind = "runbook"
status = "active"
review_every = "180d"
-->

# Bootstrap runbook

> The one-time GitHub setup the maintainer runs before intake: what it needs, what it sets,
> and what stays by hand.

**Bootstrap is the script that configures GitHub for a repo made from this template.** A new
repo starts with GitHub's defaults, which require no review and no checks. `scripts/bootstrap.sh`
changes every setting the template depends on: how PRs merge, the protection rules on `main`
and `wave/*` with their required checks, what GitHub Actions may do, the labels, the repo
variables the review gate reads, and the security features. Nobody sets these by hand.

It runs once, before intake, and again after intake's design PR merges, since intake may
change the required checks. It is safe to run at any time. This runbook covers who runs it,
how, exactly what it sets, and the few steps it can't do.

> [!IMPORTANT]
> Agents never change repo settings on their own. A device session runs bootstrap only when
> the maintainer asks for it ([device brief](../agents/device.md#bootstrap)). Any agent may
> run the read-only `--check`.

<a id="before"></a>
## Before you start
<!-- @section
covers = ["scripts/bootstrap.sh"]
-->

- **Who runs it.** The maintainer, from a clone of the new repo, or a device session on
  their machine under their `gh` login ([device › Bootstrap](../agents/device.md#bootstrap)).
- **A `gh` login with admin rights** on the repo: `gh auth status` shows it. Rulesets and
  Actions settings need admin. That login becomes `MAINTAINER_LOGIN` ([What it sets](#settings)).
- **`git`, `gh` and `python3` (3.11 or later)** on the `PATH`. Any bash works, including the
  bash 3.2 that macOS ships. Install notes for macOS and Windows (WSL) are in
  [Getting started](../GETTING-STARTED.md#before).
- **A plan that has rulesets** for the repo: public repos on any plan, private repos on a
  paid plan. On a private repo, the required `dependency-review` check also needs GitHub
  Code Security and the dependency graph, and secret scanning needs GitHub Secret
  Protection.
- **`repo.toml` as committed.** The script reads the required checks from
  `[checks].required` and the label names from `[labels]`. With `--also-require <rev>` it
  also requires the checks in `<rev>`'s `repo.toml` ([Running it](#run)).

The script checks the tools, the Python version and the `gh` login first, and then the
access and plan: admin rights, and, for a private repo, the owner's plan and GitHub Code
Security. When a private repo lacks a paid plan or Code Security, setup stops with exit 2
and changes nothing, because without them either nothing protects `main` or no PR can pass
its required checks. When it can't see the owner's plan (another user's account, or an
organization whose plan only its owners see), it says so with `skip:` and carries on.

<a id="run"></a>
## Running it
<!-- @section
covers = ["scripts/bootstrap.sh"]
-->

From the repo root:

1. See what differs, with no writes:
   ```sh
   scripts/bootstrap.sh --check
   ```
2. Apply the settings:
   ```sh
   scripts/bootstrap.sh
   ```
   The repo is the one `gh repo view` resolves in the clone; `--repo <owner>/<repo>` names
   another. `--template` also marks the repo as a template repository, and is used only on
   the template itself.
3. Run `scripts/bootstrap.sh --check` again. It ends with `All settings match.` and exits 0.
4. Do the [by-hand steps](#by-hand).

**When a PR changes `[checks].required`**, the rulesets change in two steps, so no PR waits
on a check nothing reports:
1. Before it merges, once the PR reports each new check: from a clone of the PR's branch,
   `scripts/bootstrap.sh --also-require origin/main`. The rulesets then require both lists.
2. Right after it merges: `scripts/bootstrap.sh` from `main`, which drops the checks the PR
   removed. A check the PR removes never reports on the PR itself, so when it removes one,
   this step comes just before the merge instead, once the PR is otherwise green.

The script is idempotent. It reads each setting first and writes only those that differ,
printing `ok:` for a match, `set:` for a write, and `skip:` for what this login or plan
doesn't offer. Running it again changes nothing.

<a id="settings"></a>
## What it sets
<!-- @section
covers = ["scripts/bootstrap.sh"]
depends_on = ["DESIGN#setup"]
-->

| Area | Desired state |
|---|---|
| Merging | squash merges and merge commits on, rebase merges off; auto-merge on; delete branch on merge; squash commit title from the PR title and message from the PR body, so the body's trailers (`Completes:`, `Claude-Session:`) reach `main` |
| Template | with `--template` only: marked as a template repository |
| Ruleset `main` | on `refs/heads/main`: no deletion, no force push, PR required with 0 approvals, squash only; every check in `[checks].required` required and pinned to GitHub Actions (app id 15368); not strict |
| Ruleset `wave/*` | on `refs/heads/wave/*`: the same, with merge commits allowed as well as squash, required checks not enforced when the branch is created, and no deletion rule, so GitHub deletes a wave once its landing PR merges |
| Actions | workflow token read-only and unable to approve PRs; actions must be pinned to a full commit SHA; fork PRs from every outside collaborator wait for approval |
| Labels | the four names in `[labels]` (governance, override, human, hardware), each with a fixed colour and description |
| Variables | `REVIEWER_SESSIONS` created with the placeholder `none` and `REVIEWER_LOGIN` with the `gh` user's login, each only when it does not exist yet; an existing value is never overwritten. `MAINTAINER_LOGIN` always set to the `gh` user's login: the person whose labels the `governance` and `review-gate` checks trust. Without it they fall back to the repo's owner, which for an organization's repo is the organization |
| Security | Dependabot vulnerability alerts and automated security fixes on; private vulnerability reporting on (public repos); secret scanning and secret scanning push protection enabled |

The required checks are exactly `[checks].required`. Jobs outside it, such as `governance`'s
`head-labels` and `ci`'s `base-tool`, run but are never required.

**The squash message.** A PR's body becomes its squash commit's message. That is how
`Completes:` and `Part-of:` reach `main` for [progress](../DOCS.md#progress), and how the
`Claude-Session:` trailer the `review-gate` check reads reaches a wave's commits. Changing
the default message to anything else breaks both.

Rulesets are matched by name among the repo's own rulesets. A missing one is created; one
that differs is replaced as a whole with the desired body. Bypass actors are not part of the
desired state, so a ruleset with any is reported as differing.

The fork-PR approval policy exists only where GitHub offers it; elsewhere the script prints
`skip:` for it. Private vulnerability reporting is offered on public repos only, so a private
repo gets `skip:` for it. Secret scanning and push protection are read from the repo's
`security_and_analysis`, which GitHub shows only to admins and only where the plan offers
them; when it is absent the script prints `skip:` for each. When GitHub refuses to enable
one, the script prints `warn:` and carries on.

<a id="check"></a>
## Read-only check
<!-- @section
covers = ["scripts/bootstrap.sh"]
-->

`scripts/bootstrap.sh --check` makes no writes. For each setting it prints `ok:`, `skip:`
where GitHub doesn't show the setting to this login or the plan doesn't offer it, or
`differs:` followed by the exact `gh` command that fixes it (request bodies as a heredoc).
A feature the plan lacks, such as rulesets on a free private repo, is a `skip:`, never a
difference. It exits 1 when anything differs and 0 when everything matches. Any agent may
run it; an agent that finds a difference reports it to the coordinator and changes nothing.

Exit 2 means the script could not run, or setup stopped before changing anything: no `git`,
`gh` or `python3`, Python older than 3.11, no login, no `repo.toml`, a repo it cannot
resolve, a bad argument (including an `--also-require` revision with no `repo.toml`), no
admin rights (setup only), or a private repo whose plan lacks
rulesets or GitHub Code Security (setup only).

<a id="by-hand"></a>
## By hand
<!-- @section
covers = ["scripts/bootstrap.sh"]
depends_on = ["AGENTS#day-0-order"]
-->

These need the maintainer's own accounts. The script prints this list at the end, and
[Getting started](../GETTING-STARTED.md#start) puts them in order with bootstrap.

1. **Install the [Claude GitHub App](https://github.com/apps/claude)** on the repo, before
   adding the repo to the Claude project.
2. **Paste the project instructions,** before bootstrap: the fenced block in
   [docs/agents/project-instructions.md](../agents/project-instructions.md), with
   `PROJECT_NAME` and `OWNER/REPO` replaced, into the Claude project's instructions.
3. **List the reviewer sessions.** Each time the coordinator posts a card with reviewer
   session ids (the first reviewers, and each one added or replaced), the maintainer sets
   `REVIEWER_SESSIONS` to them, comma-separated, as the `review-gate` workflow reads them:
   `gh variable set REVIEWER_SESSIONS --body "<ids>"`, or in the repo's **Settings › Secrets
   and variables › Actions**. Then they tell the coordinator, which re-runs `review-gate` on
   the open PRs: changing a variable doesn't re-run it.
4. **Private repo only: turn on the dependency graph** in **Settings › Code security**, or
   `dependency-review` fails on every PR.
5. **After intake's design PR merges,** paste the project instructions again and run
   bootstrap again. Intake adds its answers to the instructions and may change
   `[checks].required`. Until bootstrap runs, the rulesets still require the old list, and a
   PR can wait on a check that no workflow runs any more.

> [!WARNING]
> Until `REVIEWER_SESSIONS` lists the reviewers, the gate counts no marker.
