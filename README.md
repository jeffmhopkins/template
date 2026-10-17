<!-- @doc
id = "README"
title = "Project README"
kind = "readme"
status = "active"
review_every = "90d"
-->

# `<PROJECT>`

[![CI](https://github.com/jeffmhopkins/template/actions/workflows/ci.yml/badge.svg)](.github/workflows/ci.yml)
[![Licence: MIT OR Apache-2.0](https://img.shields.io/badge/licence-MIT%20OR%20Apache--2.0-blue)](LICENSE-MIT)

> `<from intake>`: one sentence on what the product is and who it is for.

> [!TIP]
> **Human?** Read [docs/GETTING-STARTED.md](docs/GETTING-STARTED.md) first. It explains how
> this works and how to start a project, with no git experience needed.

> [!IMPORTANT]
> **Agent?** Read [CLAUDE.md](CLAUDE.md) and your brief in [docs/agents/](docs/agents/README.md)
> first.

The full pitch, users, demos and never-rules are in [docs/PRODUCT.md](docs/PRODUCT.md). Every
doc, with what it is for, is listed in [docs/README.md](docs/README.md).

<a id="how"></a>
## How this repo works
<!-- @section
-->

| Part | Where | What it is |
|---|---|---|
| Config | [`repo.toml`](repo.toml) | tracked and protected paths, required checks, serial lanes, labels, the review marker and the stack pack |
| Docs | [docs/](docs/README.md) | Markdown with `<!-- @doc -->` and `<!-- @section -->` metadata. Each section lists the files it describes (`covers`) and the sections it relies on (`depends_on`) |
| Ledger | [`docs/verified.toml`](docs/verified.toml) | a BLAKE3 stamp per section over its body and inputs. A changed input makes the section stale until an agent re-reads and restamps it ([DOCS.md](docs/DOCS.md)) |
| Rules and decisions | [RULES.md](docs/RULES.md), [adr/](docs/adr/README.md) | rules with permanent IDs, each naming its enforcer; ADRs in an agent or a maintainer tier |
| Repo tool | [`tools/repo`](tools/repo/README.md), run as `scripts/repo` | a Rust CLI: docs check, verify and generate, governance diff, PR titles, serial-lane guard, derived progress |
| Stack pack | [`stacks/`](stacks/README.md) | the language's `fmt`, `lint`, `test` and `deps` commands and config; Rust ships |
| CI | [`.github/workflows/`](.github/workflows/) | `ci.yml` runs the stack jobs, docs check and dependency review; `governance.yml` runs the governance, PR-title and lane checks from the base commit; `review-gate.yml` turns the reviewer's verdict marker into the `review-gate` check |
| Main upkeep | `main-health.yml`, `progress.yml`, `sweep.yml` | a pinned "main is red" issue, the pinned progress issue, and a weekly sweep of advisories and docs due for review |
| Agents | [CLAUDE.md](CLAUDE.md), [REVIEW.md](REVIEW.md), [docs/agents/](docs/agents/README.md) | one brief per role. Builders start at CLAUDE.md; reviewers judge under REVIEW.md |
| GitHub setup | [`scripts/bootstrap.sh`](scripts/bootstrap.sh) | the rulesets, required checks, labels and security settings, applied once |

A change moves through it like this:

1. A builder takes one roadmap item and opens a PR from its own branch.
2. CI runs the required checks. A change to a protected path is a governance change and needs
   the `governance-approved` label and a written justification.
3. A reviewer agent posts a round under REVIEW.md, ending in a verdict marker for the head SHA.
   `review-gate` goes green only on a pass from a listed reviewer session.
4. GitHub auto-merges (squash). The PR body's `Completes:` or `Part-of:` trailer lands on
   `main`, and progress is derived from it.

The full design is in [docs/design/template-design.md](docs/design/template-design.md).
This repo uses template version 0.1.0<!--fact:template-version-->.

<a id="progress"></a>
## Progress
<!-- @section
-->

Milestones are in [docs/ROADMAP.md](docs/ROADMAP.md). Status is derived from `main`
([DOCS › Progress](docs/DOCS.md#progress)).

<!-- @generated begin progress -->
| Milestone | Done | In progress | Hardware run pending | Planned |
|---|---|---|---|---|
| M0: Template in place | 0 of 1 | 0 | 0 | 1 |

<details>
<summary>M0: Template in place</summary>

Demo: A PR that changes one doc section is reviewed, restamped and auto-merged.

| Item | Title | Status | Depends on |
|---|---|---|---|
| M0-1 | Repo created from the template, bootstrap run, checks green on main | planned |  |

</details>
<!-- @generated end progress -->

<a id="status"></a>
## Status
<!-- @section
-->

`<from intake>`: the current milestone and what it shows. Outside contributions are closed
until the first release ([CONTRIBUTING.md](CONTRIBUTING.md)).

<a id="licence"></a>
## Licence
<!-- @section
-->

Licensed under either of [Apache License, Version 2.0](LICENSE-APACHE) or
[MIT license](LICENSE-MIT), at your option, unless intake sets another licence.
