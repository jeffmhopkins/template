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
is the template's GitHub repo, `[template].version` the version the project is on, and
`[template].commit` the full SHA of the template commit its template-owned files match.
Intake records the commit; each upgrade moves it.

> [!NOTE]
> In the template itself, `source` names the template's own repo, and this runbook doesn't
> apply ([CLAUDE › Which repo](../../CLAUDE.md#which-repo)).

<a id="when"></a>
## When and who
<!-- @section
-->

- An upgrade is one row the planner writes, delivered by one builder as one PR.
- Each template version is a tag `v<version>` in the template's repo, with a heading in its
  `CHANGELOG.md` saying what changed ([Releasing the template](#release)).
- Upgrade one version at a time when a version's CHANGELOG entry asks for steps of its own;
  otherwise go straight to the latest.
- The diff always starts from `[template].commit`, never from a tag: a project made before
  the first tag, or between two tags, still has an exact starting point.

<a id="compare"></a>
## Compare the versions
<!-- @section
-->

From the project's root, with `<source>` and `<commit>` read from `[template]` in
`repo.toml`:

1. List the template's versions and pick `<new>`:
   ```sh
   git ls-remote --tags https://github.com/<source>.git 'v*'
   ```
2. Fetch the template's history and the new version under their own ref namespace, so they
   never touch the project's own branches or tags:
   ```sh
   git fetch --no-tags https://github.com/<source>.git \
     "refs/heads/main:refs/template/main" "refs/tags/v<new>:refs/template/v<new>"
   git cat-file -e "<commit>^{commit}"
   ```
3. Read what changed: `git show refs/template/v<new>:CHANGELOG.md` and
   `git diff --stat <commit> refs/template/v<new>`.

**No `[template].commit` yet?** Find the template commit whose tree equals the project's
first commit's tree, since "Use this template" copies a tree exactly:
```sh
tree=$(git rev-parse "$(git rev-list --max-parents=0 HEAD | tail -n 1)^{tree}")
git log --format='%H %T' refs/template/main | awk -v t="$tree" '$2 == t { print $1; exit }'
```
Record it in `repo.toml` in the upgrade PR. If nothing matches, use the commit of the tag
`[template].version` names, and say so in the PR.

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
- the template's own groups in `docs/RULES.md`, the template's ADRs that `<commit>` already
  has, `docs/adr/TEMPLATE.md`;
- the template's entries in `docs/facts.toml` and `docs/external.toml`.

**Never overwritten** (the project owns these):
- the product docs: `docs/PRODUCT.md`, `docs/ROADMAP.md`, `docs/ARCHITECTURE.md`, `README.md`
  (apart from the `template-version` fact marker) and `docs/progress.toml`;
- the project's own ADRs, and the rule groups intake added to `docs/RULES.md`;
- the values intake set inside template-owned files: `[stack]`, the project's extra
  `[checks].required` names and lanes in `repo.toml`, the licence allow list in the stack
  pack, the project-instructions additions, the project's repo in the badge entries of
  `docs/external.toml`, and the security report link in `.github/ISSUE_TEMPLATE/config.yml`;
- `lanes.toml`, and the ledger `docs/verified.toml`, which only `docs verify` writes.

In a project, `CHANGELOG.md` is the project's own: intake replaced the template's. The
template's changelog is read at the new tag and never copied.

**How template ADRs and project ADRs share one sequence.** A project starts with the
template's ADRs, 0001 up to the template's highest number at `<commit>`, and numbers its own
from the next free number. The template's ADRs that `<commit>` already has keep their
numbers, and only their status changes come over ([Apply](#apply), step 3).

**New template ADRs are ported one by one.** An ADR the template added after `<commit>` may
share its number with one of the project's. Each comes over under the project's next free
ADR number ([ADR index](../adr/README.md)), with its `id` and the links to it in the files
that come over renamed to match. Its file says which template ADR it is: the port adds one
line under its title, `> Ported from the template's ADR-<NNNN>.`, which is part of the ADR
from then on. A later upgrade finds the template ADRs already ported by that line, and
applies a template change that supersedes one to the project's number for it.

<a id="apply"></a>
## Apply the change
<!-- @section
-->

1. Branch from `main` and open a draft PR (CLAUDE.md's PR loop), titled
   `chore: upgrade the template to v<new>`.
2. Make the template's diff for the paths that come over, without its ADRs, and apply it
   three-way:
   ```sh
   git diff --binary <commit> refs/template/v<new> -- \
     tools/repo .github stacks scripts .claude repo.toml .editorconfig .gitattributes \
     CLAUDE.md REVIEW.md docs/DOCS.md docs/SECURITY-AGENTS.md docs/agents docs/runbooks \
     docs/design docs/RULES.md docs/adr docs/facts.toml docs/external.toml \
     ':(exclude)docs/adr/README.md' ':(exclude)docs/adr/[0-9]*.md' > "$TMPDIR/template.patch"
   git apply --3way "$TMPDIR/template.patch"
   ```
   Hunks in files the project never edited apply cleanly. Where both sides changed a line,
   git leaves a conflict.
3. Apply the changes to the template's ADRs that `<commit>` already has, by name:
   ```sh
   git diff --binary <commit> refs/template/v<new> -- \
     $(git ls-tree --name-only <commit> -- 'docs/adr/' | grep '/[0-9]') > "$TMPDIR/adrs.patch"
   git apply --3way "$TMPDIR/adrs.patch"
   ```
   An accepted ADR changes only by being superseded ([R-DOC-008](../RULES.md#R-DOC-008)), so
   this patch should carry only `status`, `superseded_by` and banner changes. Any other
   change to an accepted ADR fails `governance` whatever the labels: leave it out and tell
   the maintainer. Then port each ADR the template added since, as [above](#what) says.
4. Resolve each conflict by keeping the project's values (what intake set, the project's
   rules and ADRs) inside the template's new structure.
5. Set `[template].version` in `repo.toml`, and the `template-version` value in
   `docs/facts.toml`, to `<new>`, and `[template].commit` to the full SHA of `v<new>`
   (`git rev-parse refs/template/v<new>^{commit}`). `scripts/repo docs check` names every
   marker that must change with them.

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
   `<new>`, and each check loosened or tightened, from the CHANGELOG). Apply
   `governance-approved` only when it prints no `maintainer-only:` path
   ([R-GOV-001](../RULES.md#R-GOV-001)).
4. Any `maintainer-only:` path it prints needs the maintainer's `/approve` decision on the
   PR, and the maintainer applies the label themselves, with `needs-human` until both are
   there ([R-GOV-007](../RULES.md#R-GOV-007)). An upgrade that changes workflows or the tool
   always has such paths.
5. When `[checks].required` changed, the rulesets follow it in two steps
   ([bootstrap runbook](bootstrap.md#run)). Before the PR merges, once it reports each new
   check, the maintainer adds the new checks: `scripts/bootstrap.sh --also-require
   origin/main` from the PR's branch. Right after it merges, `scripts/bootstrap.sh` from
   `main` removes the checks it dropped. A dropped check never reports on the PR itself, so
   when there is one, that second run comes just before the merge, once the PR is otherwise
   green. The builder sends each step to the coordinator as `NEEDS-MAINTAINER`, offering a
   device session.

<a id="release"></a>
## Releasing the template
<!-- @section
depends_on = ["CLAUDE#which-repo"]
-->

This applies in the template itself. Each version in its `CHANGELOG.md` gets a tag, so
projects can name what they upgrade to.

1. The PR that finishes a version moves its changes under a `## <version>` heading in
   `CHANGELOG.md`, sets `[template].version` in `repo.toml` and the `template-version` fact to
   it, and lists the steps a project needs beyond the diff, if any.
2. Once it merges, the maintainer tags the merge commit on `main` and pushes the tag:
   ```sh
   git tag -a v<version> -m "template v<version>" <merge commit>
   git push origin v<version>
   ```
   Publishing a tag is an outside action, so an agent does it only on the maintainer's own
   words ([project-instructions](../agents/project-instructions.md)).
3. Every `## <version>` heading in `CHANGELOG.md` has its tag. A version without one is
   tagged at the commit that added its heading.
