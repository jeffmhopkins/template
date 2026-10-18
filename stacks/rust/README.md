<!-- @doc
id = "STACK-RUST"
title = "Rust stack pack"
kind = "guide"
status = "active"
review_every = "180d"
-->

# Rust stack pack

> Everything Rust-specific: the commands behind the CI jobs, the lint and licence config,
> and the patterns the repo tool reads.

A stack pack holds everything language-specific: the commands behind the stack's CI jobs,
the lint and dependency config, the suppression patterns and the back-reference syntax the
repo tool reads, the stack's CI setup and sweep items, and the stack's rules. The rest of the
repo is the same for every stack, apart from the Rust setup the repo tool needs for itself.

<a id="plugging-in"></a>
## How a pack plugs in
<!-- @section
covers = ["stacks/rust/pack.toml"]
-->

`repo.toml` names the pack: `[stack].pack = "rust"` selects `stacks/rust/pack.toml`.
`scripts/repo config check` validates the pack with the rest of the config.

| `pack.toml` key | Read by |
|---|---|
| `detect` | `scripts/repo stack run`: when this file is absent at the repo root, every job prints `skip: no <detect>` and passes |
| `[jobs]` `fmt`, `lint`, `test`, `deps` | the CI jobs of the same names in `.github/workflows/ci.yml`, through `scripts/repo stack run <job>`, from the repo root |
| `[suppressions].patterns` | `scripts/repo governance`: an added line matching one is a new suppression, so a governance change. The Rust pack matches `#[allow(`, `#[expect(`, their inner forms, `cfg_attr(…allow(` or `expect(`, and `#[ignore` |
| `[suppressions].reason_required` | `scripts/repo governance`: a matching line without `reason = "…"` is a finding |
| `[protected_tables]` | `scripts/repo governance`: an edit to `[lints]` or `[workspace.lints]` in any `Cargo.toml` is a governance change |
| `[lockfiles]` | `scripts/repo docs merge-main`: a conflicted `Cargo.lock` takes the merged branch's side, then `cargo metadata` in its directory adds this branch's own dependencies back |
| `[backrefs]` | `scripts/repo docs check`: a file matching `roots` whose package some section covers has one `prefix` line listing exactly those sections as `DOC#anchor` |
| `[ci].setup` | `scripts/repo stack setup` in every stack job of `ci.yml` and in the sweep: the Rust pack names `.github/actions/rust-stack`, which installs cargo-deny for `deps` and the sweep |
| `[ci].deps_paths` | `.github/scripts/deps-changed.py`: on a PR, the `deps` job runs only when a `Cargo.toml`, `Cargo.lock` or `deny.toml` changes, or one of the paths the script always watches |
| `[[sweep]]` | `scripts/repo stack sweep` in the weekly sweep: `cargo deny check`, and the compatible updates `Cargo.lock` would take (`.github/scripts/cargo-updates.sh`) |

The pack's rules are in [RULES.md](RULES.md), a tracked doc with id `RULES-RUST`, whose
`R-RUST-*` rules sit beside the engineering rules in `docs/RULES.md`.

The Rust toolchain and build cache come from `.github/actions/rust-setup`. The repo tool is
written in Rust, so every CI job that runs it uses that action whatever the stack, and the
Rust pack's own setup action only adds cargo-deny.

<a id="setup"></a>
## Starting the Rust project
<!-- @section
covers = ["stacks/rust/deny.toml", "stacks/rust/clippy.toml", "stacks/rust/rustfmt.toml"]
-->

The first PR that adds the root `Cargo.toml`:
1. copies `deny.toml`, `clippy.toml` and `rustfmt.toml` from this directory to the repo
   root, where cargo-deny, clippy and rustfmt look for them;
2. adds `rust-toolchain.toml` ([R-RUST-001](RULES.md#R-RUST-001));
3. adds the lint table below to the root `Cargo.toml`, and `[lints] workspace = true` to
   every crate;
4. adds a Dependabot `cargo` entry for directory `/` in `.github/dependabot.yml`.

> [!IMPORTANT]
> Every one of these is a governance change. Until that PR lands, the stack jobs skip.

<a id="lints"></a>
## The workspace lint table
<!-- @section
depends_on = ["RULES-RUST#R-RUST-003", "RULES-RUST#R-RUST-004", "RULES-RUST#R-RUST-005", "RULES-RUST#R-RUST-006", "RULES-RUST#R-RUST-009", "RULES-RUST#R-RUST-011"]
-->

```toml
[workspace.lints.rust]
unsafe_code = "deny"                 # R-RUST-005
unsafe_op_in_unsafe_fn = "deny"      # R-RUST-006
missing_docs = "warn"                # R-RUST-011

[workspace.lints.clippy]
allow_attributes_without_reason = "deny"   # R-RUST-004
undocumented_unsafe_blocks = "deny"        # R-RUST-006
unwrap_used = "deny"                       # R-RUST-009
```

> [!WARNING]
> CI runs clippy with `-D warnings`, so a `warn` here fails the `lint` job too.

The `//! Docs:` line ([R-RUST-012](RULES.md#R-RUST-012)) is the crate-level doc comment
`missing_docs` asks for.

<a id="other-stacks"></a>
## Adding another stack
<!-- @section
-->

Adding a pack for another language is covered in [the stacks guide](../README.md#adding).
