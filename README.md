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

> [!IMPORTANT]
> **Agent?** Read [CLAUDE.md](CLAUDE.md) and your brief in [docs/agents/](docs/agents/README.md)
> first.

The full pitch, users, demos and never-rules are in [docs/PRODUCT.md](docs/PRODUCT.md). Every
doc, with what it is for, is listed in [docs/README.md](docs/README.md).

<a id="how"></a>
## How this repo works
<!-- @section
-->

This repo is built by a swarm of AI agents and governed by checks that a PR can't weaken.

- **Builders** take one roadmap item each and deliver it as one PR. Their entry point is
  [CLAUDE.md](CLAUDE.md).
- **A reviewer agent** judges every PR under [REVIEW.md](REVIEW.md). Its verdict is the
  merge gate; GitHub auto-merges once every required check is green.
- **Rules** have permanent IDs in [docs/RULES.md](docs/RULES.md). **Decisions** are ADRs in
  [docs/adr/](docs/adr/README.md), each in an agent or a maintainer tier.
- **Docs are checked like code** ([docs/DOCS.md](docs/DOCS.md)): a change that makes a
  section stale can't merge until an agent re-reads it.
- **One tool**, `scripts/repo`, does the checking, configured by [`repo.toml`](repo.toml).
- **The agent roles** (intake, coordinator, planner, builders, reviewers, integrator,
  triage, board) have one brief each in [docs/agents/](docs/agents/README.md).

The template's design is in [docs/design/template-design.md](docs/design/template-design.md).
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
