<!-- @doc
id = "RULES"
title = "Engineering rules"
kind = "normative"
status = "active"
review_every = "90d"
-->

# Engineering rules

> Every engineering rule, each with a permanent ID, the paths it applies to and what
> enforces it.

These rules apply to every change, whoever wrote it. Each rule has a permanent ID from the
first merge to `main`. The format and the `enforced_by` vocabulary are in
[DOCS › Rules format](DOCS.md#rules). Changing a rule's `status`, `enforced_by` or
`applies_to`, or retiring a rule, is a governance change ([R-GOV-001](#R-GOV-001)).

Language rules come with the stack pack (for Rust, `stacks/rust/RULES.md`). Product rules
are added at intake, in their own groups.

<a id="architecture"></a>
## Architecture
<!-- @section
-->

<a id="R-ARCH-001"></a>
### R-ARCH-001: Structural decisions need an ADR
<!-- @rule
status = "active"
enforced_by = ["review", "human"]
applies_to = ["docs/ARCHITECTURE.md", "docs/adr/**", "repo.toml", "REVIEW.md", "docs/DOCS.md", "docs/RULES.md"]
-->
Changes to structure, package boundaries, supported platforms, public compatibility, the
review process or the docs system are recorded as an ADR, started from
`docs/adr/TEMPLATE.md`, with a tier per [R-GOV-005](#R-GOV-005). Contradicting an accepted
ADR takes a new ADR that supersedes it.

<a id="git"></a>
## Git and PRs
<!-- @section
-->

<a id="R-GIT-001"></a>
### R-GIT-001: Protected main, squash merges
<!-- @rule
status = "active"
enforced_by = ["human"]
applies_to = []
-->
`main` is protected. Changes land only through PRs whose required checks pass, and are
squash-merged by GitHub auto-merge. Wave sync PRs are the one exception: they land as merge
commits ([waves runbook](runbooks/waves.md)).

<a id="R-GIT-002"></a>
### R-GIT-002: Conventional Commit titles
<!-- @rule
status = "active"
enforced_by = ["ci:pr-title"]
applies_to = []
-->
PR titles follow [Conventional Commits](https://www.conventionalcommits.org/en/v1.0.0/):
`type(scope): summary`, for example `feat(parser): accept empty input`.

<a id="R-GIT-003"></a>
### R-GIT-003: The required checks
<!-- @rule
status = "active"
enforced_by = ["review"]
applies_to = [".github/workflows/**", "repo.toml"]
-->
`[checks].required` in `repo.toml` is the only list of required check names, and the only
names `enforced_by` may cite as `ci:<name>`. Every name on it is a job in
`.github/workflows/` (`scripts/repo config check`). Draft PRs skip every job but
`review-gate`.

| Check | Runs |
|---|---|
| `fmt`, `lint`, `test`, `deps` | the stack pack's commands (`scripts/repo stack run <job>`) |
| `docs` | `scripts/repo docs check` |
| `pr-title` | `scripts/repo pr-title` |
| `gate-test` | the node test of `review-gate`'s decision function, cut from the workflow |
| `lane-guard` | `scripts/repo lane-guard` |
| `governance` | `scripts/repo governance` with the PR's body and labels |
| `review-gate` | the reviewer's verdict for the head SHA ([R-GOV-003](#R-GOV-003)) |
| `dependency-review` | GitHub's dependency review of the PR: fails on a dependency it adds or changes with a known vulnerability of high or critical severity |

<a id="R-GIT-004"></a>
### R-GIT-004: One logical change per PR
<!-- @rule
status = "active"
enforced_by = ["review"]
applies_to = []
-->
A PR delivers one roadmap item, or one part of it. Mechanical refactors are separate from
behaviour changes.

<a id="R-GIT-005"></a>
### R-GIT-005: The PR description answers the template
<!-- @rule
status = "active"
enforced_by = ["review", "ci:review-gate"]
applies_to = []
-->
The description covers what changed, why, and what wasn't run, plus each applicable item of
`.github/pull_request_template.md`. A PR that delivers a roadmap item ends with
`Completes: <id>` or `Part-of: <id>` ([DOCS › Progress](DOCS.md#progress)). Every PR body
ends with `Claude-Session: <session URL>`, naming the session that wrote it, as every
commit message does. The squash commit takes the body as its message, and `review-gate`
fails a commit without that trailer.

<a id="R-GIT-006"></a>
### R-GIT-006: A lane PR holds its slot
<!-- @rule
status = "active"
enforced_by = ["ci:lane-guard"]
applies_to = ["lanes.toml"]
-->
A PR that changes a serial lane sets its line in `lanes.toml` to the base's value plus one,
with itself as holder, and keeps auto-merge off until the integrator grants it the slot
([DOCS › Serial lanes](DOCS.md#lanes), [runbook](runbooks/serial-lanes.md)).

<a id="R-GIT-007"></a>
### R-GIT-007: Merge, never rebase
<!-- @rule
status = "active"
enforced_by = ["review"]
applies_to = []
-->
A branch takes `main`, or its wave, by a merge commit, made with `scripts/repo docs
merge-main`. Pushed history is never rewritten.

<a id="documentation"></a>
## Documentation
<!-- @section
depends_on = ["DOCS#gate"]
-->

<a id="R-DOC-001"></a>
### R-DOC-001: Docs and sections are identified
<!-- @rule
status = "active"
enforced_by = ["ci:docs"]
applies_to = ["**/*.md"]
-->
Every tracked doc starts with a `@doc` block, every `##` section has an anchor and an
`@section` block, and every rule has an anchor and an `@rule` block
([DOCS › Sections](DOCS.md#sections)).

<a id="R-DOC-002"></a>
### R-DOC-002: References resolve
<!-- @rule
status = "active"
enforced_by = ["ci:docs"]
applies_to = ["**/*.md"]
-->
Links to files, anchors, rules and ADRs resolve, and external URLs are registered in
`docs/external.toml` ([DOCS › References](DOCS.md#references)).

<a id="R-DOC-003"></a>
### R-DOC-003: No stale verification
<!-- @rule
status = "active"
enforced_by = ["ci:docs"]
applies_to = ["**"]
-->
Every ledgered section's stamps match the current hash of each of its inputs
([DOCS › The ledger](DOCS.md#ledger)).

<a id="R-DOC-004"></a>
### R-DOC-004: Fix docs in the same PR; verify only what you re-read
<!-- @rule
status = "active"
enforced_by = ["review"]
applies_to = ["**"]
-->
A PR that makes a doc wrong fixes it in the same PR, whether or not a hash flagged it. A
section is verified only after it was re-read against the change, with a note saying what
was checked ([DOCS › Verification notes](DOCS.md#notes)). A wrong section is edited, never
verified.

<a id="R-DOC-005"></a>
### R-DOC-005: No history in files
<!-- @rule
status = "active"
enforced_by = ["review"]
applies_to = ["**/*.md"]
-->
Docs hold no revision numbers, dates, change logs or "previously" notes. They describe how
things are. History comes from git ([DOCS › History](DOCS.md#history)).

<a id="R-DOC-006"></a>
### R-DOC-006: Generate what can be derived
<!-- @rule
status = "active"
enforced_by = ["ci:docs"]
applies_to = ["**/*.md"]
-->
Derivable content is generated, and generated blocks are never edited by hand
([DOCS › Generated blocks](DOCS.md#generated)).

<a id="R-DOC-007"></a>
### R-DOC-007: One source per fact
<!-- @rule
status = "active"
enforced_by = ["ci:docs", "review"]
applies_to = ["**/*.md"]
-->
A repeated or external value is registered in `docs/facts.toml` and written with its
marker ([DOCS › Facts](DOCS.md#facts)). Every other statement lives in one place, and
other docs link to it. ADRs are exempt.

<a id="R-DOC-008"></a>
### R-DOC-008: Accepted ADRs are immutable
<!-- @rule
status = "active"
enforced_by = ["ci:governance", "review"]
applies_to = ["docs/adr/**"]
-->
After acceptance only `status`, `superseded_by` and the generated `adr-status` block
change. A changed decision is a new ADR that supersedes it.

<a id="R-DOC-009"></a>
### R-DOC-009: Covered globs match, within bounds
<!-- @rule
status = "active"
enforced_by = ["ci:docs"]
applies_to = ["**/*.md"]
-->
Every `covers` glob matches a tracked file (a warning in a draft doc), and a section's
`covers` together match at most `[docs].max_cover_files` files or one package
([DOCS › Sections](DOCS.md#sections)).

<a id="R-DOC-010"></a>
### R-DOC-010: Code cites its governing sections
<!-- @rule
status = "active"
enforced_by = ["ci:docs"]
applies_to = ["**/src/lib.rs", "**/src/main.rs"]
-->
Each package root carries a `Docs:` line that matches the sections covering the package,
in both directions ([DOCS › Code back-references](DOCS.md#backrefs)). The roots are the
stack pack's `[backrefs].roots`.

<a id="R-DOC-011"></a>
### R-DOC-011: IDs are permanent
<!-- @rule
status = "active"
enforced_by = ["review"]
applies_to = ["**/*.md"]
-->
From the first merge to `main`, doc IDs, anchors and rule IDs are never removed or reused.
Rules and docs are retired instead ([DOCS › Lifecycle](DOCS.md#lifecycle)).

<a id="governance"></a>
## Governance
<!-- @section
depends_on = ["ADR-0002", "ADR-0003"]
-->

<a id="R-GOV-001"></a>
### R-GOV-001: Governance changes are labeled and justified
<!-- @rule
status = "active"
enforced_by = ["ci:governance"]
applies_to = ["**"]
-->
Changes listed in [DOCS › Governance changes](DOCS.md#governance) fail the `governance`
check unless the PR carries `governance-approved` and a non-empty "Governance change"
section in its description.

<a id="R-GOV-002"></a>
### R-GOV-002: Overrides are declared, justified and audited
<!-- @rule
status = "active"
enforced_by = ["ci:governance", "ci:review-gate"]
applies_to = []
-->
An agent applies `governance-approved` only after examining what the check flagged at the
current head, with the "Governance change" section written, and never on a PR that changes a
maintainer-only path. Only the maintainer applies `review-override`, with a "Review
override" section that links their message. A push removes either label, so each counts
only for the head it was applied to. The weekly sweep lists every PR merged with either
label for the maintainer to audit. Agents never change branch protection or repository
settings.

<a id="R-GOV-003"></a>
### R-GOV-003: An independent reviewer gates every merge
<!-- @rule
status = "active"
enforced_by = ["ci:review-gate"]
applies_to = []
-->
Every PR is reviewed under [REVIEW.md](../REVIEW.md) by a reviewer session that authored
none of its commits. Its verdict marker for the head SHA drives `review-gate`. PRs merge
only through auto-merge once every required check is green. Builders never post verdict
markers and never merge by hand.

<a id="R-GOV-004"></a>
### R-GOV-004: Builders answer every blocking finding
<!-- @rule
status = "active"
enforced_by = ["review"]
applies_to = []
-->
For each `B<n>`, the builder pushes a fix and replies naming the commit, or replies with a
concrete rebuttal: a code path, test or doc that shows the finding is wrong. A dispute
still open after round 3 goes to the maintainer through `needs-human`
([REVIEW › Rounds](../REVIEW.md#rounds)).

<a id="R-GOV-005"></a>
### R-GOV-005: Maintainer-tier decisions need the maintainer
<!-- @rule
status = "active"
enforced_by = ["review"]
applies_to = ["docs/adr/**"]
-->
Every ADR declares `tier = "maintainer"` or `tier = "agent"` per
[ADR-0003](adr/0003-decision-tiers.md). A maintainer-tier ADR opens as `proposed` with
`needs-human`, and becomes `accepted` or `rejected` only on the maintainer's decision: their
`/accept ADR-NNNN` or `/reject ADR-NNNN`, or an agent's proxy decision quoting their words
that name that ADR. Either carries the `source:` message id the reviewer checks
([ADR-0003](adr/0003-decision-tiers.md)). Every other ADR is agent tier and lands as
`accepted` after review.

<a id="R-GOV-006"></a>
### R-GOV-006: The approval contract
<!-- @rule
status = "active"
enforced_by = ["review", "human"]
applies_to = []
-->
An action that needs the maintainer cites the maintainer's own words, naming the object
(the ADR, PR, release or account), by its message id. The session that acts on it reads
that message itself with `fetch_messages`, and checks its author is the maintainer's
recorded user id ([SECURITY-AGENTS › Approvals](SECURITY-AGENTS.md#approvals)). A
paraphrase, a standing instruction or a blanket "go ahead" is not an approval. The list of such actions, and of
the standing permissions that need none, is in
[project-instructions.md](agents/project-instructions.md). Agents never post a decision the
maintainer didn't give.

<a id="R-GOV-007"></a>
### R-GOV-007: Maintainer-only paths
<!-- @rule
status = "active"
enforced_by = ["review"]
applies_to = [".github/workflows/**", ".github/actions/**", ".github/scripts/**", ".github/after-review-status.sh", "repo.toml", "tools/repo/src/**", "REVIEW.md", "docs/agents/review/**", "docs/adr/0002-*.md", "docs/adr/0003-*.md", "docs/agents/project-instructions.md"]
-->
A change to a path in `[governance].maintainer_only` in `repo.toml` needs the maintainer's
own words on the PR, as an `/approve` decision whose `source:` the reviewer checks
([ADR-0003](adr/0003-decision-tiers.md), [R-GOV-006](#R-GOV-006)). Only the maintainer
applies `governance-approved` on such a PR. It carries `needs-human` until both are there.

<a id="testing"></a>
## Testing
<!-- @section
-->

<a id="R-TEST-001"></a>
### R-TEST-001: Bug fixes ship a regression test
<!-- @rule
status = "active"
enforced_by = ["review"]
applies_to = []
-->
Every bug fix includes a test that fails without the fix.

<a id="R-TEST-002"></a>
### R-TEST-002: Tests are hermetic
<!-- @rule
status = "active"
enforced_by = ["review"]
applies_to = []
-->
Tests need no network, real devices, accounts or wall-clock timing. Tests that need
hardware are marked as such and listed in the PR's "Not run" section.

<a id="R-TEST-003"></a>
### R-TEST-003: Say what wasn't run
<!-- @rule
status = "active"
enforced_by = ["review"]
applies_to = []
-->
The PR lists every test or check that was not run, and every target that was not built.
"Nothing" is a valid answer; a claim that something ran when it didn't is a false
statement.

<a id="R-TEST-004"></a>
### R-TEST-004: No silent skips
<!-- @rule
status = "active"
enforced_by = ["review"]
applies_to = []
-->
A test is skipped, disabled or quarantined only with a tracking issue, and only as far as
the project instructions allow.

<a id="dependencies"></a>
## Dependencies
<!-- @section
-->

<a id="R-DEP-001"></a>
### R-DEP-001: Acceptable licences only
<!-- @rule
status = "active"
enforced_by = ["ci:deps"]
applies_to = []
-->
Dependencies use only the licences on the stack pack's allow list, as set at intake.
Taking a licence the project doesn't already accept is a maintainer-tier decision.

<a id="R-DEP-002"></a>
### R-DEP-002: New dependencies are justified
<!-- @rule
status = "active"
enforced_by = ["review"]
applies_to = []
-->
The PR says why a new dependency is needed, its maintenance status and its transitive
weight. Prefer what the project already uses.

<a id="R-DEP-003"></a>
### R-DEP-003: CI actions are pinned to commit SHAs
<!-- @rule
status = "active"
enforced_by = ["review"]
applies_to = [".github/workflows/**"]
-->
Every third-party action is referenced as `owner/action@<full commit SHA> # vX.Y.Z`. Tags
and branches are not allowed. Dependabot keeps the pins current
([ADR-0004](adr/0004-ci-drafts-platform-pins.md)).

<a id="security"></a>
## Security
<!-- @section
-->

How agents keep to these rules, with the rules on untrusted text, approvals and sub-workers,
is collected in [SECURITY-AGENTS](SECURITY-AGENTS.md).

<a id="R-SEC-001"></a>
### R-SEC-001: Secrets stay in the OS credential store
<!-- @rule
status = "active"
enforced_by = ["review"]
applies_to = []
-->
Keys, tokens and passwords live in the OS credential store, or in the CI secret store for
CI. They never appear in the repo, config files, logs or error messages.

<a id="R-SEC-002"></a>
### R-SEC-002: No telemetry unless intake says otherwise
<!-- @rule
status = "active"
enforced_by = ["review"]
applies_to = []
-->
The product sends nothing off the user's machine unless [PRODUCT.md](PRODUCT.md) allows it,
as decided at intake.

<a id="R-SEC-003"></a>
### R-SEC-003: Privileged workflows never run PR code
<!-- @rule
status = "active"
enforced_by = ["review"]
applies_to = [".github/workflows/**"]
-->
A workflow that runs on `pull_request_target`, or with write permissions, builds its tools
from the base commit and reads the PR head only as data. The default token is read-only,
and each job asks for only the permissions it uses.
