<!-- @doc
id = "STACKS"
title = "Stack packs"
kind = "guide"
status = "active"
review_every = "180d"
-->

# Stack packs

> How a language plugs into the template, and the steps to add a new one. Each pack is one
> directory under `stacks/`; [rust/](rust/README.md) is the pack the template ships.

The stack pack holds what is specific to a language: the commands behind the `fmt`, `lint`,
`test` and `deps` CI jobs, the setup those jobs need, the paths that make `deps` run, the
weekly sweep's dependency items, the lint and dependency config, the patterns that mark a
lint suppression, the comment that links a package to its docs, and the language's rules.
The workflows read all of it from the pack, so adding a pack edits no workflow. The one
language they name is Rust, for the repo tool itself: every job that runs `scripts/repo`
sets up Rust, and the sweep checks the tool's own dependencies. A project uses exactly one
pack, named in `repo.toml`:

```toml
[stack]
pack = "rust"    # reads stacks/rust/pack.toml
```

<a id="layout"></a>
## What a pack contains
<!-- @section
-->

```
stacks/<name>/
  pack.toml     the commands and patterns the repo tool reads (below)
  README.md     a tracked doc: how this stack starts in a project, and its config
  RULES.md      a tracked doc, id RULES-<NAME>, with rules R-<NAME>-NNN
  <config>      lint, format and dependency config, copied to the repo root by the
                project's first PR in this language
```

| Pack | Status |
|---|---|
| [rust](rust/README.md) | ships with the template |

<a id="pack-toml"></a>
## `pack.toml` reference
<!-- @section
covers = ["tools/repo/src/config.rs", "tools/repo/src/stack.rs"]
-->

| Key | Required | What it does |
|---|---|---|
| `detect` | yes | a file at the repo root that means the stack is in use, such as `Cargo.toml` or `package.json`. While it is absent, `scripts/repo stack run <job>` prints `skip: no <detect>` and passes |
| `[jobs]` | no | job name → shell command. `scripts/repo stack run fmt` runs the `fmt` command from the repo root, and so on for `lint`, `test` and `deps` |
| `[suppressions].patterns` | no | regular expressions; an added line that matches one is a lint suppression, which `scripts/repo governance` reports as a governance change |
| `[suppressions].reason_required` | no | when `true`, a matching line must also contain `reason =`, or it is a finding |
| `[suppressions].files` | no | globs of the files to scan for suppressions; empty means every file that isn't Markdown |
| `[protected_tables]` | no | manifest glob → list of dotted table names, such as `"**/Cargo.toml" = ["lints", "workspace.lints"]`. Any edit to one of those tables, including adding or removing it, is a governance change. A manifest whose new version doesn't parse is one too |
| `[lockfiles]` | no | lockfile glob → the shell command that relocks it, run in the lockfile's directory, such as `"**/Cargo.lock" = "cargo metadata --format-version 1 > /dev/null"`. `scripts/repo docs merge-main` resolves a conflict in a matching file by taking the merged branch's side and running the command |
| `[backrefs].roots` | yes | globs of package root files. A root whose package a doc section covers carries one back-reference line |
| `[backrefs].prefix` | yes | the text that starts that line, such as `//! Docs:` or `# Docs:` |
| `[ci].setup` | no | a composite action directory under `.github/actions/`, such as `.github/actions/rust-stack`. Every stack job in `ci.yml`, and the sweep, runs `scripts/repo stack setup .stack-setup`, which copies the directory there, then `uses: ./.stack-setup` with input `job` set to `fmt`, `lint`, `test`, `deps` or `sweep`. Without the key, or while `detect` is absent, it writes a no-op action there instead |
| `[ci].deps_paths` | no | globs of the manifests, lockfiles and dependency config whose change on a PR makes the `deps` job run. `.github/scripts/deps-changed.py` reads them, and always adds the setup action, the repo tool's manifests, every `pack.toml` and `ci.yml` |
| `[[sweep]]` | no | the weekly sweep's items for the stack, in order, each a `title` and a `run` shell command. `scripts/repo stack sweep` runs each from the repo root and prints its Markdown output under `### <title>`; a command that prints nothing shows as passed, and one that fails shows its exit code and the end of its output. It always exits 0, and skips every item while `detect` is absent |

Unknown keys are an error. `scripts/repo config check` loads the pack and checks its globs
and table names, that `[ci].setup` is a directory under `.github/actions/` holding an
`action.yml`, and that every `[[sweep]]` item has a `title` and a `run`, with no title
twice.

> [!NOTE]
> `reason_required` looks for the literal text `reason =`, which matches Rust's attribute
> syntax. For a language whose suppressions take a reason another way, set it to `false` and
> require the reason with a rule in the pack's `RULES.md`, enforced by `review`.

<a id="adding"></a>
## Adding a new stack
<!-- @section
depends_on = ["STACKS#pack-toml"]
-->

Adding a pack is a template change: it lands in the template repo as one PR, and every
project gets it through the [template-upgrade runbook](../docs/runbooks/template-upgrade.md).

> [!IMPORTANT]
> Everything under `stacks/` and `.github/` is a protected path, so the PR is a governance
> change with a "Governance change" section. The pack's `pack.toml`, its setup action and
> the scripts its sweep items run are maintainer-only paths, so the maintainer approves the
> PR and applies the `governance-approved` label themselves.

1. **Make the directory** `stacks/<name>/`, lower case, one word (`node`, `python`, `go`).
2. **Write `pack.toml`.** Pick the `detect` file, then the four job commands. Each command
   must fail on a problem and pass on a clean tree:
   - `fmt` checks formatting without rewriting files;
   - `lint` treats warnings as errors;
   - `test` runs every test;
   - `deps` audits dependencies for advisories and licences.

   Add the suppression patterns, the manifest tables that set lint levels
   (`[protected_tables]`), the lockfiles and the command that relocks each (`[lockfiles]`),
   and the back-reference prefix and roots (examples below).
3. **Add the config files** the four commands read: formatter, linter, licence allow list.
   Licences follow [R-DEP-001](../docs/RULES.md#R-DEP-001).
4. **Write `RULES.md`**, a tracked doc with id `RULES-<NAME>` and one `@rule` block per rule.
   Cover at least the toolchain pin, formatting, lint suppressions with reasons, unsafe or
   dynamic code, error handling and the back-reference line. Each rule names its enforcer:
   `ci:fmt`, `ci:lint`, `ci:test`, `ci:deps`, `review` or `human`.
5. **Write `README.md`**, a tracked doc like [rust/README.md](rust/README.md): what the
   project's first PR in this language does (copying the config files to the root, pinning
   the toolchain) and the lint settings.
6. **Set up CI from the pack.** Write a composite action
   `.github/actions/<name>-stack/action.yml` that installs the toolchain and the tools the
   four commands run, with every third-party action pinned to a full commit SHA and every
   download checked against a pinned checksum. It takes one input, `job`, so it can skip
   what a job doesn't need. Name it in `[ci].setup`. Its steps run in the job's workspace,
   after `rust-setup`, which stays in every job because the repo tool is written in Rust.
   List the stack's manifests, lockfiles and dependency config in `[ci].deps_paths`.
7. **Add sweep items.** One `[[sweep]]` item per weekly check, such as the full dependency
   audit and the updates the lockfile would take. A command longer than one line goes in a
   script under `.github/scripts/`, since the sweep runs it with a token that can write
   issues. The setup action gets `job: sweep` before the items run.
8. **Add Dependabot** for the stack's package manager in `.github/dependabot.yml`, for
   directory `/`. Its `github-actions` entry already watches `.github/actions/*`.
9. **Add the pack to the table** in [What a pack contains](#layout).
10. **Check it:**
   ```sh
   scripts/repo config check                         # the pack parses, globs are valid
   scripts/repo stack run fmt                        # prints skip: no <detect> in the template
   scripts/repo docs check                           # the new READMEs and rules are valid
   scripts/repo governance --base origin/main        # lists the governance changes
   ```
   In a scratch project with the `detect` file present, run each job once on clean code and
   once on code that breaks it, and confirm the job fails.

A project switches to the new pack by setting `[stack].pack` in its `repo.toml`, which intake
does when it picks the stack.

<a id="examples"></a>
## Example `pack.toml` files
<!-- @section
depends_on = ["STACKS#pack-toml"]
-->

Starting points to adapt. Each tool in them must be pinned and installed by the stack's
setup action.

**Node and TypeScript**
```toml
detect = "package.json"

[jobs]
fmt  = "npx --no-install prettier --check ."
lint = "npx --no-install eslint --max-warnings 0 ."
test = "npm test"
deps = "npm audit --audit-level=high"

[suppressions]
patterns = ['eslint-disable', '@ts-ignore', '@ts-expect-error']
reason_required = false

[backrefs]
roots = ["**/src/index.ts"]
prefix = "// Docs:"
```

**Python**
```toml
detect = "pyproject.toml"

[jobs]
fmt  = "ruff format --check ."
lint = "ruff check ."
test = "pytest"
deps = "pip-audit"

[suppressions]
patterns = ['# noqa', '# type: ignore', '# pragma: no cover']
reason_required = false

[backrefs]
roots = ["**/__init__.py"]
prefix = "# Docs:"
```

> [!TIP]
> Copy the Rust pack's `RULES.md` as the outline for a new one, and replace each rule with
> the new language's equivalent. Keep the rule IDs in the new `R-<NAME>-NNN` series.
