<!-- @doc
id = "RUNBOOK-TEMPLATE-UPGRADE"
title = "Template upgrade runbook"
kind = "runbook"
status = "active"
review_every = "90d"
-->

# Template upgrade runbook

> Take a newer template version as one governance PR: template-owned paths come over,
> project-owned content is never overwritten.

How a project made from the template takes a later template version without being reset.
The project records where it came from in [`repo.toml`](../../repo.toml): `[template].source`
is the template's GitHub repo and `[template].version` the version the project is on. In the
template itself, `source` names the template's own repo, and this runbook doesn't apply
([CLAUDE › Which repo](../../CLAUDE.md#which-repo)).

<a id="when"></a>
## When and who
<!-- @section
-->

- An upgrade is one row the planner writes, delivered by one builder as one PR.
- Each template version is a tag `v<version>` in the template's repo, with a heading in its
  `CHANGELOG.md` saying what changed.
- Upgrade one version at a time when a version's CHANGELOG entry asks for steps of its own;
  otherwise go straight to the latest.

<a id="compare"></a>
## Compare the versions
<!-- @section
-->

From the project's root, with `<source>` and `<old>` read from `[template]` in `repo.toml`:

1. List the template's versions and pick `<new>`:
   ```sh
   git ls-remote --tags https://github.com/<source>.git 'v*'
   ```
2. Fetch both versions under their own ref namespace, so they never touch the project's own
   tags:
   ```sh
   git fetch --no-tags https://github.com/<source>.git \
     "refs/tags/v<old>:refs/template/v<old>" "refs/tags/v<new>:refs/template/v<new>"
   ```
3. Read what changed: `git show refs/template/v<new>:CHANGELOG.md` and
   `git diff --stat refs/template/v<old> refs/template/v<new>`.

<a id="what"></a>
## What comes over, and what never does
<!-- @section
depends_on = ["AGENT-INTAKE#outputs"]
-->

**Comes over** (the template owns these; the project's own edits to them are merged, not
lost):
- `tools/repo/`, `.github/`, `stacks/`, `scripts/` and `.claude/`;
- `repo.toml`, `.editorconfig` and `.gitattributes`;
- `CLAUDE.md`, `REVIEW.md`, `docs/DOCS.md`, `docs/SECURITY-AGENTS.md`, `docs/agents/`,
  `docs/runbooks/` and `docs/design/`;
- the template's own groups in `docs/RULES.md`, ADRs 0001 to 0005, `docs/adr/TEMPLATE.md`;
- the template's entries in `docs/facts.toml` and `docs/external.toml`.

**Never overwritten** (the project owns these):
- the product docs: `docs/PRODUCT.md`, `docs/ROADMAP.md`, `docs/ARCHITECTURE.md`, `README.md`
  (apart from the `template-version` fact marker) and `docs/progress.toml`;
- ADRs 0006 and up, and the rule groups intake added to `docs/RULES.md`;
- the values intake set inside template-owned files: `[stack]`, the project's extra
  `[checks].required` names and lanes in `repo.toml`, the licence allow list in the stack
  pack, the project-instructions additions, and the project's repo in the badge entries of
  `docs/external.toml`;
- `lanes.toml`, and the ledger `docs/verified.toml`, which only `docs verify` writes.

`CHANGELOG.md` describes the template; it is read at the new tag and not copied. A template
ADR numbered above 0005 comes over under the project's next free ADR number, and the links to
it in the files that come over are renamed to match.

<a id="apply"></a>
## Apply the change
<!-- @section
-->

1. Branch from `main` and open a draft PR (CLAUDE.md's PR loop), titled
   `chore: upgrade the template to v<new>`.
2. Make the template's diff for the paths that come over, and apply it three-way:
   ```sh
   git diff --binary refs/template/v<old> refs/template/v<new> -- \
     tools/repo .github stacks scripts .claude repo.toml .editorconfig .gitattributes \
     CLAUDE.md REVIEW.md docs/DOCS.md docs/SECURITY-AGENTS.md docs/agents docs/runbooks \
     docs/design docs/RULES.md docs/adr docs/facts.toml docs/external.toml \
     ':(exclude)docs/adr/README.md' > "$TMPDIR/template.patch"
   git apply --3way "$TMPDIR/template.patch"
   ```
   Hunks in files the project never edited apply cleanly. Where both sides changed a line,
   git leaves a conflict.
3. Resolve each conflict by keeping the project's values (what intake set, the project's
   rules and ADRs) inside the template's new structure. Drop any hunk the template's diff
   makes to a project-owned ADR (0006 and up).
4. Set `[template].version` in `repo.toml`, and the `template-version` value in
   `docs/facts.toml`, to `<new>`. `scripts/repo docs check` names every marker that must
   change with them.

<a id="after"></a>
## Check, restamp, land
<!-- @section
depends_on = ["RULES#R-GOV-001", "RULES#R-GOV-007", "RUNBOOK-BOOTSTRAP#check"]
-->

1. Run `scripts/repo config check`, the stack's jobs (`scripts/repo stack run <job>`), the
   tool's tests (`cargo test --manifest-path tools/repo/Cargo.toml --locked`) and
   `node --test '.github/tests/*.test.mjs'`.
2. Run `scripts/repo docs check`. Re-read every section it lists as stale and restamp each one
   with its own note, naming what the template change did to it
   (`.claude/skills/verify-docs/SKILL.md`). Then `scripts/repo docs generate`.
3. The PR is a governance change: `scripts/repo governance --base origin/main` lists the
   protected paths. Write the `## Governance change` section (template version `<old>` to
   `<new>`, and each check loosened or tightened, from the CHANGELOG) and apply
   `governance-approved` ([R-GOV-001](../RULES.md#R-GOV-001)).
4. Any `maintainer-only:` path it prints needs the maintainer's own words on the PR, with
   `needs-human` until they are there ([R-GOV-007](../RULES.md#R-GOV-007)).
5. When `[checks].required` changed, the rulesets must follow it once the PR merges: run
   `scripts/bootstrap.sh --check`, and send its differences to the coordinator for the
   maintainer to apply ([bootstrap runbook](bootstrap.md#check)).
