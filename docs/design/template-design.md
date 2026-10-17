<!-- @doc
id = "DESIGN"
title = "Template design"
kind = "design"
status = "active"
review_every = "90d"
-->

# Template design

How this template works: the repo it gives a project, the agent swarm that builds in it,
the one-time setup, and the plan for building the template itself. The agent briefs in
[docs/agents/](../agents/README.md) are the detailed spec for the swarm. Where this doc and a
brief differ, the brief wins.

<a id="overview"></a>
## 1. Overview
<!-- @section
-->

**The template is the swarm.** A fork gets a governed repo and a set of agent roles. The
project's content comes from an intake agent that questions the maintainer.

**The repo side:**
- a builder entry point (`CLAUDE.md`) that points to the rules;
- rules with permanent IDs;
- decision records (ADRs) in two tiers: maintainer and agent;
- a reviewer agent whose verdict is the merge gate;
- a governance check that a PR can't weaken;
- docs checked like code;
- CI shaped for many parallel agent PRs.

One language-neutral tool (`tools/repo`) does the checking, configured by one file
(`repo.toml`). Each language plugs in through a small stack pack; Rust comes first.

**The agent side:**
- intake runs first;
- then five standing roles: coordinator, planner, reviewers, integrator, and triage;
- a small board thread keeps a status page;
- builders: 6 per reviewer, 12 to start.

Agents hand off with fixed one-line messages and keep their state in files. An approval contract says exactly which actions need the maintainer's own
words.

<a id="repo"></a>
## 2. The repo
<!-- @section
-->

### 2.1 Layout

```
CLAUDE.md                     entry point for builders: where the rules are, not what they are
REVIEW.md                     how the reviewer judges a PR
README.md                     project pitch, link to progress
CONTRIBUTING.md  SECURITY.md  CODE_OF_CONDUCT.md  LICENSE-*
repo.toml                     the one config: tracked and protected paths, required checks,
                              serial lanes, label names, review marker prefix, stack pack
lanes.toml                    one line per serial lane (value and holder)
docs/
  DOCS.md                     the docs system
  RULES.md                    the rules
  PRODUCT.md                  written by intake: pitch, users, demos, never-rules
  ARCHITECTURE.md             skeleton: components, boundaries, data model, processes
  ROADMAP.md                  milestones, written by intake
  progress.toml               items, milestones, dependencies (status is derived)
  verified.toml               the docs verification ledger (empty)
  facts.toml  external.toml   repeated values; registered external URLs
  adr/  TEMPLATE.md README.md (generated index)
        0001 docs as a checked graph                     (agent tier)
        0002 review gate and governance                  (maintainer tier)
        0003 decision tiers                              (maintainer tier)
        0004 CI: drafts skip, platform after review, pins (agent tier)
        0005 wave branches and serial lanes              (agent tier)
  SECURITY-AGENTS.md          the agents' rules on untrusted text, approvals and secrets
  runbooks/  waves.md  serial-lanes.md  bootstrap.md  template-upgrade.md
  agents/                     one brief per agent type, plus project-instructions.md
  design/                     this document
.github/
  pull_request_template.md  ISSUE_TEMPLATE/  dependabot.yml
  workflows/ ci.yml review-gate.yml governance.yml after-review.yml sweep.yml main-health.yml
  tests/review-gate-decide.test.mjs
.claude/
  settings.json  hooks/session-start.sh
  skills/steward/SKILL.md     PR conventions the harness reads before CI and review events
  skills/verify-docs/SKILL.md how to re-read and stamp doc sections
tools/repo/                   the checking tool
stacks/rust/                  Rust stack pack: CI steps, language rules, lint and licence
                              defaults, dependency lint, suppression patterns
scripts/bootstrap.sh          one-time GitHub setup, run by the maintainer
.editorconfig  .gitattributes editor defaults; LF endings, generated files marked
```

The template's ADRs take 0001 to 0005, and each project numbers on from 0006.

The cloud harness reads `.claude/skills/steward/SKILL.md` from a PR's head branch before it
acts on CI or review events. It holds the merge, restamp and merge-only round rules.

### 2.2 Rules

`docs/RULES.md` holds rules with permanent IDs. Each rule declares:
- `applies_to`: path globs;
- `enforced_by`: a CI check, `review` or `human`. A rule naming an enforcer the tool doesn't
  implement fails the docs check.

The template's own rule groups:
- **Architecture:** structural decisions need an ADR.
- **Git and PRs:**
  - protected `main`, squash merges, Conventional Commit titles;
  - one logical change per PR;
  - the PR description answers the template;
  - a PR that changes a serial lane holds its slot.
- **Docs:**
  - docs and sections have permanent IDs, and references resolve;
  - no stale verification, and a section is verified only after it's re-read;
  - no history in files;
  - derivable content is generated;
  - one source per fact;
  - accepted ADRs are immutable;
  - covered globs must match.
- **Governance:**
  - changes that loosen a check carry a label and a justification;
  - overrides are declared and audited;
  - an independent reviewer gates every merge;
  - builders answer every blocking finding;
  - maintainer-tier decisions need the maintainer;
  - actions that need the maintainer cite their own words, naming the object.
- **Testing:** bug fixes ship a regression test; tests are hermetic; PRs say what wasn't run.
- **Dependencies:**
  - acceptable licences only;
  - new dependencies are justified;
  - CI actions are pinned to commit SHAs.
- **Security:** secrets stay in the OS credential store; no telemetry unless intake says otherwise.

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
  sections and cover no code.

**How staleness works:**
- **The ledger** (`docs/verified.toml`) holds one hash per input of each section: its body,
  each covered file, each dependency. When any input changes, the section is stale.
- **Verifying.** `repo docs verify` restamps a section after an agent re-reads it, with a note
  saying what was checked. Stock notes are rejected, and the reviewer judges every note.
- **Merging.** `repo docs merge-main` resolves ledger and generated-block conflicts by
  recomputing hashes. Where the merged input matches one side's recorded hash, it keeps that
  entry and its note. Otherwise the section is stale and gets re-read. Any other conflict is
  left for the agent.
- **Time-based review never blocks a PR.** Docs past their review interval, and external URLs
  due for a re-check, are listed in the weekly sweep issue.

**Progress is derived, not recorded:**
- `docs/progress.toml` holds items, milestones and dependencies only.
- A PR's body ends with `Completes: <id>` or `Part-of: <id>`. The squash message is the PR
  body. A wave landing lists the trailers it carries.
- A revert cancels an item. `hardware` means merged with no `Verified-on-hardware: <id>` yet.
- Progress is published as a pinned issue updated on every push to `main`, and on the board.

### 2.5 The tool: `tools/repo`

One standalone binary, written in Rust, driven by `repo.toml`. A non-Rust project needs
only a toolchain in CI, or a prebuilt release.

| Command | What it does |
|---|---|
| `repo docs check` | metadata, references, ledger freshness, generated drift, rule enforcers |
| `repo docs verify` | restamp a section after a re-read |
| `repo docs for <path>` | the rules, sections and ADRs that govern a file |
| `repo docs generate` | rewrite generated blocks (ADR index, status banners) |
| `repo docs log` | a doc's or section's history, from git |
| `repo docs review` | stamp a doc's time-based review |
| `repo docs merge-main [--from wave/<name>]` | merge, resolve ledger and generated conflicts, list sections to re-read |
| `repo governance --base <rev>` | list governance changes |
| `repo progress` | derive progress from trailers on `main` |
| `repo lane-guard` | check a PR's serial-lane claim |
| `repo plan check` | no `owns` overlap between running rows |
| `repo heads-changed` | cheap "did anything move" check for sweeps |

**Rules for the tool:**
- It exits non-zero on any finding.
- It resolves the repo root at run time.
- Its config is schema-checked and fails on unknown keys.
- Per-language details are config from the stack pack: suppression patterns
  (`#[allow`, `eslint-disable`, `# noqa`), the comment syntax for code back-references, and
  dependency lint.

The template records its source repo and version in `repo.toml` and keeps a CHANGELOG. A
project takes later versions through `docs/runbooks/template-upgrade.md`.

### 2.6 CI and the merge gate

**`ci.yml`:**
- Draft PRs skip every job.
- `fmt`, `lint`, `test` and `docs` come from the stack pack.
- `pr-title`, `gate-test` and `lane-guard` are generic.
- `dependency-review` fails a PR that adds or changes a dependency with a known
  vulnerability of high or critical severity.

**`review-gate.yml`:**
- The reviewer posts one comment per round ending in
  `<!-- review v1 verdict=… sha=… round=… reviewer-session=… -->`.
- The gate counts only markers that:
  - are for the current head SHA;
  - are posted by the configured login;
  - come from a session listed in the `REVIEWER_SESSIONS` repo variable;
  - haven't been edited.
- The highest round decides, and a fail beats a pass.
- A reviewer whose session authored a commit is ignored (recusal by the `Claude-Session`
  trailer), and the gate fails if a non-merge commit lacks that trailer (Dependabot aside).
- `review-override` needs a justification that links the maintainer's message.
- A node test runs the decision function cut out of the deployed workflow.

**Running checks from base:** `governance`, `lane-guard` and `pr-title` run on
`pull_request_target`. They build the checker from the base commit and read the head only as
git objects. The checker's build cache is
written only on pushes to `main`.

**Rulesets:** every required check is pinned to GitHub Actions (`integration_id`).

**Governance changes** need the `governance-approved` label and a "Governance change" section
in the PR description. They are:
- edits to `covers`, `applies_to`, `enforced_by` or `status`;
- any edit to an accepted ADR;
- retiring a rule or a doc;
- edits to protected paths: lint and licence config, `.github/`, the tool, `DOCS.md`,
  `REVIEW.md`;
- new lint suppressions.

Agents may self-label after examining what was flagged. Changes to the gate, the tier rules
or the project instructions always need the maintainer.

**`after-review.yml`** runs expensive jobs (other OSes, end-to-end) only after the review gate
passes, posting commit statuses under each check name. It stays dormant until a project needs it.

**`main-health.yml`:** while a required check fails on `main`, it keeps one pinned "main is
red" issue.

**`sweep.yml`** (weekly, never fails) rewrites one pinned issue with:
- dependency advisories;
- docs due for review;
- available updates;
- every PR merged with `governance-approved` or `review-override`;
- sections re-verified far more often than they are edited;
- lane holders older than a day.

The required-check list lives in `repo.toml`, and the tool compares it with the workflow job
names.

### 2.7 Serial lanes and waves

**A serial lane** is a value only one PR at a time may change: a protocol or schema version,
a migration number. An ADR takes its PR's number.

- Each lane is one line in `lanes.toml`: `protocol = { value = 21, holder = 512 }`.
- Two claims on the same lane conflict in git.
- `lane-guard` passes only when `value` is the base's plus one and `holder` is this PR.
- The integrator hands out one slot at a time, to the first ready PR
  ([integrator.md](../agents/integrator.md)).

**Waves** batch PRs into a branch that lands in `main` as one PR.
- PRs target `wave/<name>` and skip the heavy CI jobs.
- The integrator syncs `main` into the wave with merge commits.
- The wave lands in `main` as one PR that runs everything.
- Waves stay off until about 15 PRs are open, or runners queue. The wave ruleset is created at
  bootstrap.

<a id="agents"></a>
## 3. The agents
<!-- @section
-->

The briefs are in [docs/agents/](../agents/README.md).

| Type | Count | Job |
|---|---|---|
| Intake | 1, first | questions the maintainer in three rounds; opens the design PR |
| Coordinator | 1 | the maintainer's only contact; launches rows, relays their words, owns the roster, daily digest |
| Planner | 1 | roadmap rows with owned and forbidden paths; what each merge unblocks |
| Builder | 6 per reviewer | one roadmap item per PR, driven until merged; ends after merge |
| Reviewer | 2, then 3 | the merge gate, sharded by PR number, every PR whatever its base |
| Reviewer worker | at most 2 per reviewer | drafts one PR's review round |
| Integrator | 1 | serial lanes, mergeability, main's health, waves |
| Triage | 1 | issues, Dependabot, main is red, idle threads |
| Board | 1 small thread | the status page |
| Device session | 0 or 1 | hardware runs and bootstrap on the maintainer's machine |

**Phases.**
1. **Setup:** the repo is created from the template, bootstrap runs, and the project
   instructions are pasted.
2. **Intake:** intake plus one reviewer. The design PR lands PRODUCT, ROADMAP, an architecture
   skeleton, the maintainer-tier ADRs and `repo.toml`.
3. **Build:** the other standing roles start, then builders, in batches chosen from rows with
   disjoint paths.
4. **Scale-up:** waves turn on, and a third reviewer joins, as load requires.
5. **Endgame:** the planner keeps a "what's left" view.

**How they work together:**
- One-line messages, mostly role to role; the coordinator gets only what the maintainer needs.
- State files under `ops/`, one writer each.
- A roster mapping roles to live sessions, with adoption when a session dies.
- A stop flag that every routine checks first.
- Automatic, tiered resume after a usage limit.
- A daily digest with cost against a budget: nothing new launches at 80%, and builders pause
  at 100%.

The [agents README](../agents/README.md) has the message table and state rules, and
[project-instructions.md](../agents/project-instructions.md) has the standing permissions and
the approval contract.

<a id="setup"></a>
## 4. One-time setup (`docs/runbooks/bootstrap.md`)
<!-- @section
-->

Agents don't change repo settings. `scripts/bootstrap.sh` is idempotent, uses template
defaults, and runs before intake. The maintainer runs it, or a device session runs it on their
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
   - `wave/*`: the same, with merge commits allowed and checks not enforced on branch
     creation.
3. **Actions:** default token read-only; SHA-pinned actions required; fork PRs need approval.
4. **Labels:** `governance-approved`, `review-override`, `needs-human`, `hardware`.
5. **The repo variable `REVIEWER_SESSIONS`.**
6. **Security:** Dependabot vulnerability alerts and security fixes; secret scanning and push
   protection.
7. **By hand:** install the Claude GitHub App, and paste the project instructions into the
   Claude project.

<a id="build-plan"></a>
## 5. Building the template
<!-- @section
-->

Each step is one PR, reviewed through the template's own gate once that exists:
1. Skeleton: docs, rules, ADRs 0001 to 0005, CLAUDE.md, REVIEW.md, the PR template.
2. `tools/repo` and `repo.toml`, with tests. This takes 3 to 4 PRs; progress derivation is new code.
3. `.github`: CI, the review gate and its test, governance, main-health, the sweep, and
   after-review (dormant).
4. `.claude/` hook and skills; final pass on the briefs.
5. `stacks/rust`, `scripts/bootstrap.sh` and the runbooks.
6. A dry run in a scratch fork: run bootstrap and land one item end to end. It must prove:
   - a hand-posted `review-gate` status doesn't unblock a PR;
   - a session woken by `send_later` keeps its tools;
   - the board can read GitHub, the thread list and `ops/`, and write its artifact;
   - each standing permission works across sessions without a prompt;
   - two lane claims conflict, and `lane-guard` fails until the second is renumbered;
   - `Completes:` trailers survive a squash and a wave landing.

<a id="open-questions"></a>
## 6. Decisions
<!-- @section
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
9. Triage may quarantine a flaky test for up to 24 hours, with a tracking issue.
