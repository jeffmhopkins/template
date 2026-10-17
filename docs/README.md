<!-- @doc
id = "DOCS-INDEX"
title = "Documentation"
kind = "readme"
status = "active"
review_every = "90d"
-->

# Documentation

> Where every doc is and what it is for. Builders start at CLAUDE.md; agents start at their
> brief.

How these docs are checked is in [DOCS.md](DOCS.md). The project's own pitch is in the
[root README](../README.md).

<a id="start"></a>
## Start here
<!-- @section
-->

| Document | Purpose |
|---|---|
| [GETTING-STARTED.md](GETTING-STARTED.md) | For the maintainer: how the template works and how to start a project with it |
| [CLAUDE.md](../CLAUDE.md) | The builder's entry point: finding the rules for a change, the docs duty, the PR loop |
| [DOCS.md](DOCS.md) | How docs, sections, the ledger and the repo tool work |
| [PRODUCT.md](PRODUCT.md) | What the product is, who it is for, and what it must never do |
| [ROADMAP.md](ROADMAP.md) | The milestones, each with its demo |
| [ARCHITECTURE.md](ARCHITECTURE.md) | How the product is built: components, boundaries, data model, processes |

<a id="rules"></a>
## Rules and decisions
<!-- @section
-->

| Document | Purpose |
|---|---|
| [RULES.md](RULES.md) | Every engineering rule, with its permanent ID |
| [REVIEW.md](../REVIEW.md) | How the reviewer judges a PR and posts its verdict |
| [adr/](adr/README.md) | Every decision record, with its tier and status |
| [Rust stack rules](../stacks/rust/RULES.md) | The rules for Rust changes |
| [SECURITY.md](../SECURITY.md) | How to report a vulnerability |
| [SECURITY-AGENTS.md](SECURITY-AGENTS.md) | What agents act on, whose words count as approval, secrets |
| [CONTRIBUTING.md](../CONTRIBUTING.md) | Who may contribute, and how work lands |
| [CODE_OF_CONDUCT.md](../CODE_OF_CONDUCT.md) | The standard everyone taking part follows |

<a id="runbooks"></a>
## Runbooks
<!-- @section
-->

| Document | Purpose |
|---|---|
| [bootstrap.md](runbooks/bootstrap.md) | The one-time GitHub setup, before intake |
| [waves.md](runbooks/waves.md) | Opening, syncing, cutting and landing wave branches |
| [serial-lanes.md](runbooks/serial-lanes.md) | How a PR gets the next value of a serial lane |
| [template-upgrade.md](runbooks/template-upgrade.md) | Taking a newer template version into a project |

<a id="agents"></a>
## Agents
<!-- @section
-->

| Document | Purpose |
|---|---|
| [agents/](agents/README.md) | Every agent type, the day 0 order, the message formats and the shared state |
| [project-instructions.md](agents/project-instructions.md) | The standing permissions and the approval contract |
| [Steward skill](../.claude/skills/steward/SKILL.md) | What to do on each event on your own PR |
| [Verify-docs skill](../.claude/skills/verify-docs/SKILL.md) | How to re-read and restamp stale sections |

<a id="design"></a>
## Design and tooling
<!-- @section
-->

| Document | Purpose |
|---|---|
| [template-design.md](design/template-design.md) | How the template works, and the plan for building it |
| [The repo tool](../tools/repo/README.md) | The command reference for `scripts/repo` |
| [Rust stack pack](../stacks/rust/README.md) | The Rust commands, lint and licence config the tool reads |
