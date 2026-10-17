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

AI agents write the code here, and GitHub checks every change before it is accepted. The repo
is made of a few parts, each with one job.

**The instructions.** Each kind of agent has a short brief in
[docs/agents/](docs/agents/README.md) saying what it does and what it must never do. Agents
that write code start at [CLAUDE.md](CLAUDE.md). The agent that reviews code follows
[REVIEW.md](REVIEW.md).

**The docs.** Everything the project knows is written down in [docs/](docs/README.md): what
the product is, the plan, the rules and the decisions made along the way. Each part of a doc
names the code it describes. When that code changes, the doc is flagged as possibly out of
date, and the change can't be accepted until an agent has re-read it and confirmed or fixed
it. That is how the docs stay true without anyone remembering to update them.

**The rules and decisions.** [docs/RULES.md](docs/RULES.md) lists every rule, each with a
permanent ID such as `R-GOV-001`. [docs/adr/](docs/adr/README.md) holds the decisions, each
on its own page with the reasons. Some decisions only you, the maintainer, can make.

**The checking tool.** `scripts/repo` is a small program that does the checking: are the docs
up to date, does a change touch something sensitive, is the change titled properly, what is
finished on the plan. [`repo.toml`](repo.toml) is its settings file.

**The language kit.** [stacks/](stacks/README.md) holds the commands for the project's
programming language: how to check formatting, run the tests and audit dependencies. Rust is
included; other languages can be added.

**The automatic checks.** [`.github/workflows/`](.github/workflows/) tells GitHub what to run
on every proposed change: the tests, the docs check, and the review gate, which only turns
green when the reviewer agent approves. Other jobs open an issue when `main` breaks, keep a
progress issue up to date, and run a weekly sweep.

**The one-time setup.** [`scripts/bootstrap.sh`](scripts/bootstrap.sh) switches on the GitHub
settings that make the checks binding, so nothing can be merged without passing them
([getting started](docs/GETTING-STARTED.md#setup)).

A change goes through four steps:

1. An agent takes one item from the plan and proposes it as a pull request.
2. GitHub runs the automatic checks on it.
3. The reviewer agent reads it and approves it or asks for fixes.
4. Once everything is green, GitHub merges it on its own, and the plan marks the item done.

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
