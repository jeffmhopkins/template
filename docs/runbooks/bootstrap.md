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

It runs once, before intake, and is safe to run again. This runbook covers who runs it, how,
exactly what it sets, and the few steps it can't do.

> [!IMPORTANT]
> Agents never change repo settings; any agent may run the read-only `--check`.

<a id="before"></a>
## Before you start
<!-- @section
covers = ["scripts/bootstrap.sh"]
-->

- **Who runs it.** The maintainer, from a clone of the new repo, or a device session on
  their machine under their `gh` login ([device brief](../agents/device.md)).
- **A `gh` login with admin rights** on the repo: `gh auth status` shows it. Rulesets and
  Actions settings need admin.
- **`gh` and `python3`** (3.11 or later) on the `PATH`.
- **A plan that has rulesets** for the repo: public repos on any plan, private repos on a
  paid plan. On a private repo, secret scanning needs GitHub Secret Protection, and the
  required `dependency-review` check needs the dependency graph and GitHub Code Security.
- **`repo.toml` as committed.** The script reads the required checks from
  `[checks].required` and the label names from `[labels]`.

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

The script is idempotent. It reads each setting first and writes only those that differ,
printing `ok:` for a match and `set:` for a write. Running it again changes nothing.

<a id="settings"></a>
## What it sets
<!-- @section
covers = ["scripts/bootstrap.sh"]
depends_on = ["DESIGN#setup"]
-->

| Area | Desired state |
|---|---|
| Merging | squash merges and merge commits on, rebase merges off; auto-merge on; delete branch on merge; squash commit title from the PR title and message from the PR body |
| Template | with `--template` only: marked as a template repository |
| Ruleset `main` | on `refs/heads/main`: no deletion, no force push, PR required with 0 approvals, squash only; every check in `[checks].required` required and pinned to GitHub Actions (app id 15368); not strict |
| Ruleset `wave/*` | on `refs/heads/wave/*`: the same, with merge commits allowed as well as squash, and required checks not enforced when the branch is created |
| Actions | workflow token read-only and unable to approve PRs; actions must be pinned to a full commit SHA; fork PRs from every outside collaborator wait for approval |
| Labels | the four names in `[labels]` (governance, override, human, hardware), each with a fixed colour and description |
| Variables | `REVIEWER_SESSIONS` created with the placeholder `none` and `REVIEWER_LOGIN` with the `gh` user's login, each only when it does not exist yet. An existing value is never overwritten |
| Security | Dependabot vulnerability alerts and automated security fixes on; secret scanning and secret scanning push protection enabled |

Rulesets are matched by name among the repo's own rulesets. A missing one is created; one
that differs is replaced as a whole with the desired body. Bypass actors are not part of the
desired state, so a ruleset with any is reported as differing.

The fork-PR approval policy exists only where GitHub offers it; elsewhere the script prints
`skip:` for it. Secret scanning and push protection are read from the repo's
`security_and_analysis`, which GitHub shows only to admins and only where the plan offers
them; when it is absent the script prints `skip:` for each. When GitHub refuses to enable
one, the script prints `warn:` and carries on.

<a id="check"></a>
## Read-only check
<!-- @section
covers = ["scripts/bootstrap.sh"]
-->

`scripts/bootstrap.sh --check` makes no writes. For each setting it prints `ok:`, `skip:`
where GitHub doesn't show the setting to this login or plan, or `differs:` followed by the
exact `gh` command that fixes it (request bodies as a heredoc). It
exits 1 when anything differs and 0 when everything matches. Any agent may run it; an agent
that finds a difference reports it to the coordinator and changes nothing. Exit 2 means the
script could not run: no `gh` or `python3`, no login, no `repo.toml`, a repo it cannot
resolve, or a bad argument.

<a id="by-hand"></a>
## By hand
<!-- @section
covers = ["scripts/bootstrap.sh"]
depends_on = ["AGENTS#day-0-order"]
-->

These need the maintainer's own accounts. The script prints the first two at the end.

1. **Install the Claude GitHub App** on the repo (https://github.com/apps/claude).
2. **Paste the project instructions.** Copy
   [docs/agents/project-instructions.md](../agents/project-instructions.md) into the Claude
   project's instructions, with the project name filled in.
3. **List the reviewer sessions.** Once the reviewers are running, the maintainer sets
   `REVIEWER_SESSIONS` to their session ids, comma-separated, as the `review-gate` workflow
   reads them: `gh variable set REVIEWER_SESSIONS --body "<ids>"`. The same applies when a
   reviewer is added or replaced.

> [!WARNING]
> Until `REVIEWER_SESSIONS` lists the reviewers, the gate counts no marker.
