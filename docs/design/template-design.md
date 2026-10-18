<!-- @doc
id = "DESIGN"
title = "Template design"
kind = "design"
status = "active"
review_every = "90d"
-->

# Template design

> How the template works: the repo it gives a project, the agent swarm that builds in it,
> the one-time setup, and the plan for building the template itself.

The agent briefs in [docs/agents/](../agents/README.md) are the detailed spec for the swarm.
Where this doc and a brief differ, the brief wins.

<a id="overview"></a>
## 1. Overview
<!-- @section
depends_on = ["AGENTS#lifecycle", "ADR-0007"]
-->

**The template is the swarm.** A fork gets a governed repo and a set of agent roles. The
project's content comes from an intake agent that questions the maintainer.

**The repo side:**
- a builder entry point (`CLAUDE.md`) that points to the rules;
- rules with permanent IDs;
- decision records (ADRs) in two tiers: maintainer and agent;
- a reviewer agent whose verdict is the merge gate;
- a governance check that a PR can't loosen for itself;
- docs checked like code;
- CI shaped for many parallel agent PRs.

One tool (`tools/repo`) does the checking for every language, configured by one file
(`repo.toml`). Each language plugs in through a small stack pack; Rust comes first.

**The agent side:**
- intake runs first;
- then four standing roles: the coordinator, the planner, the reviewers and ops. All but
  the coordinator wake from scheduled triggers, a fresh session per fire;
- a small board role keeps a status page;
- builders: 6 per reviewer, 12 to start.

Agents hand off with fixed one-line messages and keep their state in files. An approval contract says exactly which actions need the maintainer's own
words.

<a id="repo"></a>
## 2. The repo
<!-- @section
depends_on = ["DOCS#sections", "DOCS#tool", "DOCS#lanes", "DOCS#progress", "STACKS#pack-toml", "ADR-0003", "ADR-0007", "ADR-0008"]
-->

### 2.1 Layout

The top level, with what each part is for. [docs/README.md](../README.md) lists every doc,
and [CI and the merge gate](#ci) every workflow.

```
CLAUDE.md  REVIEW.md          the builder's entry point; how the reviewer judges a PR
README.md  CONTRIBUTING.md  SECURITY.md  CODE_OF_CONDUCT.md  LICENSE-*  CHANGELOG.md
repo.toml                     the one config: docs, checks, governance, labels, review marker,
                              stack pack, template source
lanes.toml                    one line per serial lane (value and holder)
docs/                         the docs: hub, docs system, rules, product, architecture,
                              roadmap, ADRs, runbooks, agent briefs, this design
.github/                      workflows, composite actions, their scripts and tests,
                              PR and issue templates, Dependabot
.claude/                      settings (the agents' allow list), the session hook, skills
tools/repo/                   the checking tool, `scripts/repo`
stacks/                       one pack per language; Rust ships
scripts/                      `repo` (runs the tool) and `bootstrap.sh` (GitHub setup)
.editorconfig  .gitattributes editor defaults; LF endings, generated files marked
```

**ADR numbers.** The template ships ADRs from 0001 up to its highest number, listed in the
[ADR index](../adr/README.md). A project numbers its own ADRs from the next free number. A
template ADR added later reaches a project through a template upgrade, which ports it under
the project's next free number ([template-upgrade runbook](../runbooks/template-upgrade.md#what)).

The cloud harness reads `.claude/skills/steward/SKILL.md` from a PR's head branch before it
acts on CI or review events. It holds the merge, restamp and merge-only round rules.

### 2.2 Rules

`docs/RULES.md` holds the rules, each with a permanent ID, the paths it applies to
(`applies_to`) and its enforcer (`enforced_by`: a CI check, `review` or `human`). A rule
naming an enforcer the tool doesn't implement fails the docs check
([DOCS › Rules format](../DOCS.md#rules)). The template's groups are architecture, git and
PRs, documentation, governance, testing, dependencies and security; RULES.md is the list.
Language rules come with the stack pack. Product rules (real-time, protocol, persistence and
so on) are written at intake.

### 2.3 Decisions

- Every ADR declares a tier.
- **Maintainer tier:**
  - what is in or out of the product, and which platforms are supported;
  - licensing;
  - security and privacy posture;
  - public compatibility promises;
  - spending;
  - the tier rules themselves.

  A maintainer-tier ADR opens as `proposed` with `needs-human`. It is accepted only on their
  decision: their `/accept ADR-NNNN`, or an agent relaying their quoted words naming that ADR.
- **Agent tier:** everything else. It lands as `accepted` after review.
- **The tier follows what the ADR decides.** The reviewer blocks a tier that is too low.

### 2.4 Docs checked like code

Every tracked Markdown file starts with a metadata block (TOML in an HTML comment) giving its
ID, kind, status and review interval. Every `##` section has a permanent anchor.

**What a section declares:**
- `covers`: the code it describes, bounded to one package or `max_cover_files` files;
- `depends_on`: other sections or rules it relies on. Architecture sections depend on package
  sections and cover no code, which `docs check` enforces.

**How staleness works:**
- **The ledger** (`docs/verified.toml`) holds one hash per input of each section: its body,
  each covered file, each dependency. When any input changes, the section is stale.
- **Verifying.** `scripts/repo docs verify` restamps a section after an agent re-reads it,
  with a note saying what was checked. Stock notes are rejected, and the reviewer judges
  every note.
- **Merging.** `scripts/repo docs merge-main` resolves ledger and generated-block conflicts by
  recomputing hashes. Where the merged input matches one side's recorded hash, it keeps that
  entry and its note. Otherwise the section is stale and gets re-read. Any other conflict is
  left for the agent.
- **Time-based review never blocks a PR.** Docs past their review interval, and external URLs
  due for a re-check, are listed in the weekly sweep issue.

**Progress is derived, not recorded:**
- `docs/progress.toml` holds items, milestones and dependencies only.
- A PR's body ends with `Completes: <id>` or `Part-of: <id>`. The squash message is the PR
  body. A wave landing carries the trailers `scripts/repo progress --trailers` prints for
  the wave.
- A revert cancels an item. `hardware` means merged with no `Verified-on-hardware: <id>` yet.
- Progress is published as a pinned issue updated on every push to `main`, and on the board.
  No committed file holds it ([ADR-0008](../adr/0008-docs-graph-v2.md)).

### 2.5 The tool: `tools/repo`

One standalone binary, written in Rust, driven by `repo.toml` and the stack pack. Every
project builds it from source with the Rust toolchain that CI sets up for it, whatever the
project's own language. Its commands are listed once, in [DOCS › The tool](../DOCS.md#tool).

**Rules for the tool:**
- It exits non-zero on any finding.
- It resolves the repo root at run time.
- Its config is schema-checked and fails on unknown keys.
- Per-language details are config from the stack pack: the job commands, the CI setup
  action, the paths that make `deps` run, the sweep items, suppression patterns
  (`#[allow`, `eslint-disable`, `# noqa`), lint-level manifest tables, lockfiles, and the
  comment syntax for code back-references ([stack packs](../../stacks/README.md#pack-toml)).

The template records its source repo and version in `repo.toml` and keeps a CHANGELOG. A
project takes later versions through `docs/runbooks/template-upgrade.md`.

### 2.6 Serial lanes and waves

**A serial lane** is a value only one PR at a time may change: a protocol or schema version,
a migration number. ADRs take the next free number instead ([ADR index](../adr/README.md)).

- Each lane is one line in `lanes.toml`: `protocol = { value = 21, holder = 512 }`.
- Two claims on the same lane conflict in git.
- `lane-guard` passes only when `value` is the base's plus one and `holder` is this PR.
- Ops hands out one slot at a time, to the first ready PR
  ([ops.md](../agents/ops.md#serial-lanes)). A lane PR is reviewed before it holds the slot,
  so its `lane-guard` is red until then.

**Waves** batch PRs into a branch that lands in `main` as one PR.
- PRs target `wave/<name>` and run every check.
- Ops syncs `main` into the wave with merge commits, then asks only the PRs left in conflict
  to merge it.
- The wave lands in `main` as one PR that runs everything; ops fixes what it finds.
- Waves stay off until re-merging costs more than they do: more than one `MERGE-MAIN` for
  every two merges over a day, or runners queue. The wave ruleset is created at bootstrap
  ([ADR-0007](../adr/0007-swarm-operating-model.md)).

<a id="ci"></a>
## 3. CI and the merge gate
<!-- @section
covers = [".github/workflows/ci.yml", ".github/workflows/main-health.yml", ".github/workflows/progress.yml", ".github/workflows/after-review.yml", ".github/actions/*/action.yml", ".github/tests/*", ".claude/hooks/*"]
depends_on = ["DOCS#governance", "DOCS#ttl", "REVIEW#output", "RULES#R-GIT-003", "STACKS#pack-toml", "ADR-0004", "ADR-0007"]
-->

The required checks are `[checks].required` in `repo.toml`; `scripts/repo config check`
compares them with the workflows' job names ([R-GIT-003](../RULES.md#R-GIT-003)). Every
third-party action is pinned to a full commit SHA ([ADR-0004](../adr/0004-ci-drafts-platform-pins.md)).

**`ci.yml`**, on every PR and every push to `main`:
- Draft PRs skip every job.
- `fmt`, `lint`, `test` and `deps` run the stack pack's commands, after the pack's setup
  action, and also check the repo tool itself. `deps` does its work on a PR only when a path
  `.github/scripts/deps-changed.py` lists changes, and passes at once otherwise.
- `docs` runs `config check` and `docs check`. `gate-test` runs the node tests.
- `dependency-review` fails a PR that adds or changes a dependency with a known
  vulnerability of high or critical severity.
- The build cache is written only on a push to `main`; `base-tool` caches the release build
  `governance.yml` restores.
- `pr-title`, `lane-guard` and `governance` run in `governance.yml`, from the base
  ([DOCS › Governance changes](../DOCS.md#governance)). `review-gate` runs in
  `review-gate.yml` ([REVIEW › Post the review](../../REVIEW.md#output)).

**Composite actions** (`.github/actions/`):
- `rust-setup`: the Rust toolchain and a restored build cache, for every job that runs the
  repo tool;
- `repo-tool`: builds the tool from the checked-out commit;
- `cargo-deny`: a prebuilt, checksum-pinned cargo-deny;
- `rust-stack`: the Rust pack's setup action, named by its `[ci].setup`.

**`review-gate.yml`** counts only verdict markers that are for the current head SHA, posted
by `REVIEWER_LOGIN`, name a role in `REVIEWER_ROLES` and haven't been edited. The highest
round decides, and a fail beats a pass. A marker whose `reviewer-session` authored a commit is
ignored, and the gate fails a non-merge commit without a `Claude-Session` trailer.
`review-override` needs the maintainer's label and a justification with a link.

**Tests** (`.github/tests/`), run by `gate-test`: the review gate's decision function, cut
out of the deployed `review-gate.yml`, and a scan of every YAML file under `.github/` for
repeated keys and tab indentation.

**`main-health.yml`:** after each `ci` run on `main`, and each completed check suite from
another CI app on `main`, it keeps one pinned "main is red" issue open while a required
check fails there, and closes it once they pass.

**`progress.yml`:** on every push to `main`, it rewrites the pinned "Progress" issue from
`scripts/repo progress`.

**`after-review.yml`** is dormant. Turned on, it runs expensive jobs (other OSes, end to
end) only after the review gate passes, and posts a commit status under each check name.

**`sweep.yml`** (weekly, never fails) rewrites one pinned issue
([DOCS › Time-based review](../DOCS.md#ttl)).

**The session hook** (`.claude/hooks/session-start.sh`) prepares a cloud session: the Rust
components and a prebuilt repo tool, then a short orientation. It never fails the session.

**Rulesets:** every required check is pinned to GitHub Actions (`integration_id`), set by
bootstrap ([setup](#setup)).

<a id="agents"></a>
## 4. The agents
<!-- @section
depends_on = ["AGENTS#messages", "AGENTS#state", "AGENTS#lifecycle", "ADR-0007"]
-->

The briefs are in [docs/agents/](../agents/README.md).

| Type | Count | Job |
|---|---|---|
| Intake | 1, first | questions the maintainer in three rounds; opens the design PR |
| Coordinator | 1 | the maintainer's only contact; launches rows, relays their words, rebuilds the roster from GitHub, owns the triggers, daily digest |
| Planner | 1 | roadmap rows with owned and forbidden paths; what each merge unblocks |
| Builder | 6 per reviewer | one roadmap item per PR, driven until merged; ends after merge |
| Reviewer | 2, then 3 | the merge gate, as stable roles `reviewer-0` to `reviewer-2`, sharded by PR number, every PR whatever its base |
| Reviewer worker | at most 2 per reviewer | drafts one PR's review round |
| Ops | 1 | main's health (revert first), serial lanes, mergeability, waves, issues, Dependabot |
| Board | 1 small role | the status page |
| Device session | 0 or 1 | hardware runs and bootstrap on the maintainer's machine |

**Phases.**
1. **Setup:** the repo is created from the template, bootstrap runs, and the project
   instructions are pasted.
2. **Intake:** intake plus one reviewer. The design PR lands PRODUCT, ROADMAP, an architecture
   skeleton, the maintainer-tier ADRs and `repo.toml`.
3. **Build:** the other standing roles start, then builders, in batches chosen from rows with
   disjoint paths.
4. **Scale-up:** waves turn on, and a third reviewer joins, as measured load requires.
5. **Endgame:** the planner keeps a "what's left" view.

**How they work together:**
- Scheduled triggers, not self-armed timers: each fire starts a fresh, short session that
  takes its role from its state file, so a usage limit costs one missed fire.
- One-line messages, mostly role to role; the coordinator gets only what the maintainer
  needs, and reads PRs and merges from GitHub.
- State files under `ops/`, one writer each, naming the session that holds the role.
- A roster rebuilt from GitHub, with adoption when a session dies.
- A stop flag that every session checks first, and that turns auto-merge off.
- A daily digest with cost against a budget: nothing new launches at 80%, and builders pause
  at 100% until the budget day resets.

The [agents README](../agents/README.md) has the message table and state rules, and
[project-instructions.md](../agents/project-instructions.md) has the standing permissions and
the approval contract.

<a id="setup"></a>
## 5. One-time setup (`docs/runbooks/bootstrap.md`)
<!-- @section
depends_on = ["RUNBOOK-BOOTSTRAP#settings", "RUNBOOK-BOOTSTRAP#by-hand"]
-->

Agents don't change repo settings. `scripts/bootstrap.sh` is idempotent, uses template
defaults, and runs before intake and again after intake's design PR merges. On a private repo
without a paid plan or GitHub Code Security it stops before changing anything. The maintainer runs it, or a device session runs it on their
machine under their `gh` login. `--check` compares the live settings with the desired ones and
prints the commands to fix any difference; any agent may run it read-only.

1. **Repo:**
   - mark the repo as a template (for this repo only);
   - squash merges and merge commits on, rebase merges off;
   - auto-merge and delete-branch-on-merge on;
   - squash title from the PR title, squash message from the PR body.
2. **Rulesets:**
   - `main`: no deletion, no force push, PR required with 0 approvals; required checks from
     `repo.toml`, each pinned to GitHub Actions; not strict.
   - `wave/*`: the same, with merge commits allowed, checks not enforced on branch
     creation, and no deletion rule, so a landed wave is deleted.
3. **Actions:** default token read-only; SHA-pinned actions required; fork PRs need approval.
4. **Labels:** `governance-approved`, `review-override`, `needs-human`, `hardware`.
5. **Repo variables:** `REVIEWER_ROLES`, `REVIEWER_LOGIN` and `MAINTAINER_LOGIN`.
6. **Security:** Dependabot vulnerability alerts and security fixes; private vulnerability
   reporting; secret scanning and push protection.
7. **By hand:** install the Claude GitHub App, paste the project instructions into the
   Claude project, and on a private repo turn on the dependency graph.

<a id="build-plan"></a>
## 6. Building the template
<!-- @section
depends_on = ["ADR-0007"]
-->

Each step is one PR, reviewed through the template's own gate once that exists:
1. Skeleton: docs, rules, the first ADRs, CLAUDE.md, REVIEW.md, the PR template.
2. `tools/repo` and `repo.toml`, with tests. This takes 3 to 4 PRs; progress derivation is new code.
3. `.github`: CI, the review gate and its test, governance, main-health, the sweep, and
   after-review (dormant).
4. `.claude/` hook and skills; final pass on the briefs.
5. `stacks/rust`, `scripts/bootstrap.sh` and the runbooks.
6. A dry run in a scratch fork: run bootstrap and land one item end to end. It must prove:
   - a hand-posted `review-gate` status doesn't unblock a PR;
   - a session started by a trigger fire keeps its tools and connectors, and a trigger
     keeps firing through a usage limit;
   - the board can read GitHub, the thread list and `ops/`, and write its artifact;
   - each standing permission works across sessions without a prompt;
   - two lane claims conflict, and `lane-guard` fails until the second is renumbered;
   - `Completes:` trailers survive a squash and a wave landing.

<a id="open-questions"></a>
## 7. Decisions
<!-- @section
depends_on = ["ADR-0002", "ADR-0003", "ADR-0006", "ADR-0007"]
-->

Agent tier:
- the tool is in Rust;
- docs use the ledger with bounded `covers`;
- waves stay dormant until needed;
- 12 builders and 2 reviewers to start;
- Rust is the first stack pack.

The maintainer's:
1. The reviewer posts under the maintainer's GitHub login; no second identity for now.
2. New projects default to dual MIT/Apache licensing; intake can change it.
3. Outside contributions are closed until a project's first release.
4. Changes to the gate, the tier rules and the project instructions always need the maintainer.
5. The template repo is public.
6. Each project has a daily token budget with auto-pause.
7. The coordinator posts a daily digest in the project chat.
8. A device session runs bootstrap on the maintainer's machine.
9. Ops may quarantine a flaky test for up to 24 hours, with a tracking issue.

Open follow-ups:
- **A merge queue.** Required checks aren't strict, so two PRs that are each green against
  an older `main` can merge into a red `main`. For now, ops reverts the merge that broke it
  and holds auto-merge on the other PRs while the "main is red" issue is open
  ([ops](../agents/ops.md#main-health)). [ADR-0007](../adr/0007-swarm-operating-model.md)
  keeps the queue off and records its design: GitHub offers it only to organization-owned
  repositories, and its `merge_group` runs need their own review of what code they run.
