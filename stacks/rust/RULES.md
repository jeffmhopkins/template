<!-- @doc
id = "RULES-RUST"
title = "Rust stack rules"
kind = "normative"
status = "active"
review_every = "90d"
-->

# Rust stack rules

> The rules every Rust change follows on top of the engineering rules, once the project has
> a root `Cargo.toml`.

These rules apply to every Rust change, on top of [the engineering rules](../../docs/RULES.md).
They take effect once the project has a root `Cargo.toml`. The lint table they rely on is in
[the pack's README](README.md#lints). Changing a rule's `status`, `enforced_by` or
`applies_to`, or retiring a rule, is a governance change.

<a id="toolchain"></a>
## Toolchain, format and lints
<!-- @section
-->

<a id="R-RUST-001"></a>
### R-RUST-001: One pinned toolchain
<!-- @rule
status = "active"
enforced_by = ["review"]
applies_to = ["rust-toolchain.toml", "**/Cargo.toml"]
-->
`rust-toolchain.toml` pins the toolchain, with the `rustfmt` and `clippy` components. The
workspace's `rust-version` is that toolchain's version. A toolchain bump is its own PR.

<a id="R-RUST-002"></a>
### R-RUST-002: Formatted
<!-- @rule
status = "active"
enforced_by = ["ci:fmt"]
applies_to = ["**/*.rs"]
-->
`cargo fmt --all --check` passes, with the repo's `rustfmt.toml`.

<a id="R-RUST-003"></a>
### R-RUST-003: Lint clean, with the workspace lint table
<!-- @rule
status = "active"
enforced_by = ["ci:lint", "review"]
applies_to = ["**/*.rs", "**/Cargo.toml"]
-->
`cargo clippy --workspace --all-targets -- -D warnings` passes. The root `Cargo.toml` carries
the pack's `[workspace.lints]` table, and every crate's `Cargo.toml` has
`[lints] workspace = true`. Turning a lint in that table down is a governance change.

<a id="suppressions"></a>
## Suppressions
<!-- @section
-->

<a id="R-RUST-004"></a>
### R-RUST-004: Every suppression carries a reason
<!-- @rule
status = "active"
enforced_by = ["ci:lint", "ci:governance"]
applies_to = ["**/*.rs"]
-->
Every `#[allow(…)]`, `#![allow(…)]`, `#[expect(…)]` and `#![expect(…)]` has
`reason = "…"` saying why the lint is wrong here. Prefer `expect` to `allow`: it fails
once the suppression is no longer needed. `clippy::allow_attributes_without_reason` enforces
the reason. A new suppression is a governance change; `scripts/repo governance` lists it
from the pack's `[suppressions].patterns`.

<a id="unsafe"></a>
## Unsafe code
<!-- @section
-->

<a id="R-RUST-005"></a>
### R-RUST-005: No unsafe code unless a crate declares it
<!-- @rule
status = "active"
enforced_by = ["ci:lint", "ci:governance"]
applies_to = ["**/*.rs"]
-->
The workspace denies `unsafe_code`. A crate that needs `unsafe` declares it once, at its
root: `#![expect(unsafe_code, reason = "<what it wraps>")]`. That line is a suppression, so
adding it is a governance change. Unsafe code lives in the fewest crates possible, ideally
one per foreign API.

<a id="R-RUST-006"></a>
### R-RUST-006: Every unsafe block states why it is sound
<!-- @rule
status = "active"
enforced_by = ["ci:lint", "review"]
applies_to = ["**/*.rs"]
-->
Every `unsafe` block has a `// SAFETY:` comment naming the invariants it relies on and why
they hold (`clippy::undocumented_unsafe_blocks`). Every `unsafe fn` documents its contract
under `# Safety`, and its body still uses `unsafe` blocks (`unsafe_op_in_unsafe_fn`).

<a id="R-RUST-007"></a>
### R-RUST-007: Safe wrappers and owned resources
<!-- @rule
status = "active"
enforced_by = ["review"]
applies_to = ["**/*.rs"]
-->
Unsafe code is wrapped in a safe API next to it. Raw pointers and foreign handles never
cross a crate's public API. A foreign resource is owned by a type that releases it on
`Drop`. Official bindings come before hand-written ones.

<a id="code"></a>
## Code and dependencies
<!-- @section
-->

<a id="R-RUST-008"></a>
### R-RUST-008: Dependencies pass cargo-deny
<!-- @rule
status = "active"
enforced_by = ["ci:deps"]
applies_to = ["**/Cargo.toml", "Cargo.lock", "deny.toml"]
-->
`cargo deny check` passes: no known advisory, only allowed licences, only crates.io and
listed sources, no wildcard versions. Every exception in `deny.toml` carries a reason, and
adding one is a governance change.

<a id="R-RUST-009"></a>
### R-RUST-009: No unwrap outside tests
<!-- @rule
status = "active"
enforced_by = ["ci:lint", "review"]
applies_to = ["**/*.rs"]
-->
Non-test code never calls `unwrap()` (`clippy::unwrap_used`). `expect` is for invariants,
and its message states the invariant: `expect("invariant: …")`.

<a id="R-RUST-010"></a>
### R-RUST-010: Typed errors in libraries
<!-- @rule
status = "active"
enforced_by = ["review"]
applies_to = ["**/*.rs"]
-->
Library crates return error enums (`thiserror`) that carry context. Opaque errors
(`anyhow`) are for binaries and tools only. An error crossing a crate boundary is never
just a string.

<a id="R-RUST-011"></a>
### R-RUST-011: Public items are documented
<!-- @rule
status = "active"
enforced_by = ["ci:lint"]
applies_to = ["**/src/**/*.rs"]
-->
Every public item of a library crate has a doc comment (`missing_docs`).

<a id="R-RUST-012"></a>
### R-RUST-012: Every crate root names its doc
<!-- @rule
status = "active"
enforced_by = ["ci:docs"]
applies_to = ["**/src/lib.rs", "**/src/main.rs"]
-->
A crate root whose package a doc section covers has one `//! Docs:` line in its leading
doc comment, listing exactly the sections that cover the package
([DOCS › Back-references](../../docs/DOCS.md#backrefs)).
