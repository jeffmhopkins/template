<!-- @doc
id = "ADR-INDEX"
title = "Architecture decision records"
kind = "readme"
status = "active"
review_every = "180d"
-->

# Architecture decision records

> The index of every decision record, with its tier and status, generated from the ADRs
> themselves.

Every decision recorded in `docs/adr/`. The table is generated from each ADR's `@doc` block
by `scripts/repo docs generate` ([DOCS › Generated blocks](../DOCS.md#generated)). New ADRs
start from [`TEMPLATE.md`](TEMPLATE.md) and take the next free number on the PR's base.
That is the only numbering rule. Two open PRs can pick the same number: the PR that merges
the base after the other landed sees `scripts/repo docs check` fail on the duplicate id, and
takes the next free number again. A project starts with the template's ADRs, 0001 up to the
template's highest number, and numbers its own from the next free one. A template ADR added
later comes over in a template upgrade under the project's next free number, so in a project
the numbers alone don't say which ADRs came from the template
([template-upgrade runbook](../runbooks/template-upgrade.md#what)). The tier rules are in
[ADR-0003](0003-decision-tiers.md).

<!-- @generated begin adr-index -->
| ADR | Title | Tier | Status |
|---|---|---|---|
| [ADR-0001](0001-docs-as-checked-graph.md) | Documentation as a checked graph | agent | superseded by ADR-0008 |
| [ADR-0002](0002-review-gate-and-governance.md) | Review gate and governance | maintainer | accepted |
| [ADR-0003](0003-decision-tiers.md) | Decision tiers | maintainer | accepted |
| [ADR-0004](0004-ci-drafts-platform-pins.md) | CI: drafts skip, platform jobs after review, SHA pins | agent | accepted |
| [ADR-0005](0005-waves-and-serial-lanes.md) | Wave branches and serial lanes | agent | superseded by ADR-0007 |
| [ADR-0006](0006-separate-agent-identities.md) | Separate GitHub identities for agents and the gate | maintainer | proposed |
| [ADR-0007](0007-swarm-operating-model.md) | Swarm operating model at scale | agent | accepted |
| [ADR-0008](0008-docs-graph-v2.md) | Documentation as a checked graph, v2 | agent | accepted |
<!-- @generated end adr-index -->
