<!-- @doc
id = "TOOL"
title = "The repo tool"
kind = "guide"
status = "active"
review_every = "180d"
-->

# The repo tool

> The command reference for `scripts/repo`, the one binary that checks docs, governance,
> lanes and progress.

`tools/repo` is the repository's checking tool: one standalone Rust binary, configured by
[`repo.toml`](../../repo.toml) and the stack pack it names. Everyone runs it as
`scripts/repo <command>`, which builds it on first use. What the checks mean is specified in
[DOCS › The tool](../../docs/DOCS.md#tool); this page is the command reference.

<a id="basics"></a>
## Basics
<!-- @section
-->

- **Root.** The tool finds the repository root at run time: the git top level when it holds
  `repo.toml`, else the nearest ancestor of the current directory that does. With
  `--root <dir>` before the command, it starts from `<dir>` instead, so a build from one
  tree checks another. Paths in output are relative to that root; file arguments such as
  `--pr-body` stay relative to the current directory.
- **Exit codes.** `0`: clean. `1`: findings, printed one per line as `path:line: message`
  (or `<command>: message`). `2`: a usage or runtime error, printed to stderr.
- **Config.** `repo.toml`, `stacks/<pack>/pack.toml`, `lanes.toml` and
  `docs/progress.toml` are parsed strictly: an unknown key is an error.
- **Globs.** `*` and `?` stay inside one path segment, `**` crosses segments, and `{a,b}`
  and `[…]` work as usual. `*.md` matches only root-level files.
- **History.** Commands that need history read it with git; nothing is cached in files.
  `governance` and `lane-guard` read both revisions as git objects only.

<a id="commands"></a>
## Commands
<!-- @section
-->

<details>
<summary><b>All commands</b> (16)</summary>

| Command | Does | Exit |
|---|---|---|
| `config check` | schema-checks `repo.toml`, the stack pack and `lanes.toml`; every `[checks].required` name must be a check name in `.github/workflows/*.yml`: a job's `name:`, else its id (a job whose `name:` is an expression has none); every copy of a `[labels]` or `[review].marker` value under `.github/` carries the annotation `[labels].<key> in repo.toml` (on its line or the comment line above) and equals the value; R-GOV-007's `applies_to` and the maintainer-only list in `docs/agents/project-instructions.md` equal `[governance].maintainer_only` | 1 on any problem |
| `docs check` | every docs finding: metadata, anchors, references, rules, facts, external URLs, `covers` bounds, `covers` in an architecture section, ledger form and freshness, back-references, ADR banner and index drift, `progress.toml` | 1 on any error |
| `docs verify <DOC#anchor> --by <who> --note "<text>"` | writes the section's current input hashes into the ledger; entries whose input changed or is new take this `by` and `note` | 2 on a stock note, a bad `--by`, or untracked covered files |
| `docs for <path>` | the sections, rules and ADRs that govern a file; the file need not exist | 0 |
| `docs generate` | rewrites every `@generated` block: `adr-status`, `adr-index`, `progress` | 1 when a block can't be generated |
| `docs log <DOC[#anchor]> [--rev <rev>]` | revisions, verifications and reviews from git, with the review clock | 0 |
| `docs review <DOC> --by <who> --note "<text>"` | writes `reviewed = { by, note }` into the `@doc` block | 2 for ADRs and docs without `review_every` |
| `docs merge-main [--from <branch>]` | merges `origin/<branch>` (default `main`), resolves ledger, generated-block and lockfile conflicts, lists stale sections with this branch's previous notes | 1 when conflicts remain or sections are stale |
| `docs due` | docs past `review_every` and external URLs past `recheck_every` | always 0 |
| `governance --base <rev> [--head <rev>] [--pr-body <file>] [--labels a,b] [--author <login>] [--into <branch>] [--main <rev>]` | lists governance changes between the merge base and the head, and a `maintainer-only: <path>` line per changed maintainer-only path and a `maintainer-only: <path>#<rule id>` line per changed `R-GOV-*` rule; with `--author dependabot[bot]`, a diff of action re-pins only is exempt and lists nothing; with `--into wave/<name>`, skips each file whose head version is the one on `--main` (default `origin/main`) and lists every other protected or maintainer-only path as a problem | listing: 0; with `--pr-body` or `--labels`: 1 when changes exist and the label or the `## Governance change` section is missing, a PR into a wave changes a protected path, or an accepted ADR changes beyond being superseded, whatever the labels |
| `pr-title "<title>"` | Conventional Commit header check, or `Revert "<title>"` | 1 when it doesn't parse |
| `lane-guard --base <rev> --head <rev> --pr <n> [--wave --head-ref <branch> --base-ref <branch>]` | checks the lanes this PR changed, and changes to a lane's `files` without a claim | 1 on a bad or missing claim; 2 when `--wave` is given for anything but a `wave/*` branch on `origin` into `main` |
| `progress [--format markdown\|json]` | status of each `progress.toml` item from commit trailers | 0; a trailer naming an unknown item is a warning |
| `progress --trailers <range>` | the `Completes:`, `Part-of:` and `Verified-on-hardware:` lines of the commits in a git range, oldest first, each once, leaving out commits a later revert in the range cancels | 0 |
| `plan check <launch.md> <roster.md> [--row <id>]` | no two running rows own overlapping paths | 1 on an overlap, or a running row without `owns` |
| `heads-changed --state <file> [--refs <glob,…>]` | prints `changed` or `unchanged` for the refs `git ls-remote origin` lists that match the globs (default `refs/heads/main` and `refs/heads/wave/**`), and stores them, writing a temporary file and renaming it over the state file | always 0 |
| `stack run <job>` | runs the pack's command for a CI job from the root; prints `skip: no <detect>` when the detect file is absent | the command's status |
| `stack setup <dir>` | copies the pack's `[ci].setup` action directory to `<dir>`, or writes a no-op action there when the pack names none or the detect file is absent; refuses a path outside the repo or one holding tracked files | 0; 2 on a bad path |
| `stack sweep` | runs each `[[sweep]]` item and prints its Markdown under `### <title>`, or its exit code and the end of its output when it fails; skips every item when the detect file is absent | always 0 |

</details>

`--by` is `agent:<name>` or `human:<login>`. A note shorter than 20 characters, or one that is
only a stock phrase such as "ok", "lgtm", "verified", "checked" or "re-read", is refused.

<a id="merge"></a>
## Merging main
<!-- @section
-->

`docs merge-main` refuses a dirty tree or a merge already in progress, fetches
`origin/<branch>` and merges it. For each conflicted file:

- **The ledger:** each entry the two sides disagree on is resolved against the merged tree.
  The side whose hash equals the input's hash now is kept, with its note. When neither does,
  the entry is dropped and its section is stale.
- **A tracked doc whose conflicts all lie inside `@generated` blocks:** the merged branch's
  side is taken, then every block is regenerated from the merged sources.
- **A lockfile matching a `[lockfiles]` glob in the stack pack:** once nothing else is left,
  the merged branch's side is taken and the pack's relock command runs in the file's
  directory; it prints `relocked: <path>`. While anything else is left, the lockfile is
  left too, with the command to run.
- **Anything else** is left as git wrote it; the merge stays open.

When nothing is left, it commits the merge and prints `stale: <DOC#anchor>` for each stale
section, followed by `previous note:` lines: the notes this branch wrote on that section
since the merge base.

<a id="progress"></a>
## Progress trailers
<!-- @section
-->

`progress` reads every commit reachable from `HEAD`. Lines `Completes: <id>`,
`Part-of: <id>` and `Verified-on-hardware: <id>` count in a trailer paragraph: one whose
every line is `Key: value`, after HTML comments are removed and outside code fences. Several
ids may be comma-separated; an id with a space in it is ignored. A commit whose message has
`This reverts commit <sha>`, or whose subject is `Revert "<subject>"` or
`revert: <subject>` naming an earlier commit, cancels that commit's trailers. Reverts are
settled newest first, so a revert that is itself reverted cancels nothing. A
Verified-on-hardware counts only after the item's latest Completes. Status: `done`
(Completes), `hardware` (done, `hardware = true`, no Verified-on-hardware since),
`in-progress` (Part-of), else `planned`.

<a id="lanes"></a>
## Lane claims
<!-- @section
-->

A lane counts as changed by the PR when its line at the head differs from the merge base.
Each changed lane needs `value` equal to the base tip's value plus one and `holder` equal to
`--pr`. A new lane passes when its holder is the PR. Removing a lane fails.

A lane the base declares with `files` also fails when the PR changes one of those files
without changing the lane's line. With a `pattern`, only an added or removed line that
matches it counts. The lanes come from the base tip, so a PR can't drop its own files.

`--wave`, for a wave's landing PR, needs `--head-ref` naming a `wave/*` branch that exists
as `origin/<branch>` and `--base-ref main`. A changed lane then only has to exceed the base's
value.

<a id="plans"></a>
## Plan files
<!-- @section
-->

`launch.md`: a row starts at an `<a id="<ID>"></a>` line and runs to the next one. Its
`owns` field is a table row whose first cell is `owns`, or a line that starts with
`owns:` (list markers and bold allowed); its globs are the backticked values in it, or its
comma-separated words when none are backticked.

`roster.md`: any table with a `row` column (or a column whose header contains `row`) and a
`state` (or `status`) column. Lines whose id has no `launch.md` section, such as roles, are
skipped. A row is running unless its state is `done`, `merged`, `ended`, `closed`, `archived`,
`abandoned`, `cancelled`, `canceled`, `dropped`, `retired` or `planned`. The format agents
write is in [AGENTS › State](../../docs/agents/README.md#state).

Two globs overlap when one matches the other's text, a tracked file matches both, or one is
a `dir/**` tree containing where the other starts.

<a id="formats"></a>
## File formats
<!-- @section
-->

**Stack pack** (`stacks/<pack>/pack.toml`):

```toml
detect = "Cargo.toml"            # file at the root that means the stack is in use
[jobs]                           # CI job → shell command, run from the root
fmt = "cargo fmt --all --check"
[suppressions]
patterns = ['#\[allow\(']        # regular expressions; a matching line is a suppression
reason_required = true           # the line must carry `reason = "..."`
files = ["**/*.rs"]              # optional; default: every file that is not Markdown
[protected_tables]               # optional; manifest glob → dotted tables
"**/Cargo.toml" = ["lints", "workspace.lints"]
[backrefs]
roots = ["**/src/lib.rs", "**/src/main.rs"]
prefix = "//! Docs:"
```

A package is a directory holding the pack's `detect` file.

**External URLs** (`[docs].external`): `[[external]]` entries with `id`, `url` and
`recheck_every` (such as `"365d"`); optional `relied_on`, `title`, `note`. An entry's
re-check clock restarts at the last commit that changed it.

**Facts** (`[docs].facts`): `[[fact]]` entries with `id` and `value`; optional `note`,
`check = "file" | "manual"`, and `source = { file, key }` (a TOML file and a dotted key that
must hold the value; required with `check = "file"`).

**Lanes** (`[lanes].file`): top-level `name = { value = <int>, holder = <PR> }`, with
optional `files = ["<glob>", …]` and `pattern = '<regex>'`.

**Template** (`repo.toml` `[template]`): `source` (`<owner>/<repo>`), `version`
(`MAJOR.MINOR.PATCH`) and, in a project, `commit`: the full SHA of the template commit its
files match.

**Progress** (`[docs].progress`): `[[milestone]]` with `id`, `title`, `demo`; `[[item]]`
with `id`, `milestone`, `title`, optional `deps` and `hardware`. Ids are unique, `deps` name
items and form no cycle.

<a id="develop"></a>
## Developing the tool
<!-- @section
-->

```sh
cargo test --manifest-path tools/repo/Cargo.toml
cargo clippy --manifest-path tools/repo/Cargo.toml --all-targets -- -D warnings
cargo fmt --manifest-path tools/repo/Cargo.toml --check
```

Unit tests sit beside each module; `tests/` builds throwaway git repositories and runs the
binary against them. The crate has its own `Cargo.lock` and an empty `[workspace]` table,
so it never joins a project's workspace.
