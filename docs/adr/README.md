<!-- @doc
id = "ADR-INDEX"
title = "Architecture decision records"
kind = "readme"
status = "active"
review_every = "180d"
-->

# Architecture decision records

Every decision recorded in `docs/adr/`. The table is generated from each ADR's `@doc` block
by `scripts/repo docs generate` ([DOCS › Generated blocks](../DOCS.md#generated)). New ADRs
start from [`TEMPLATE.md`](TEMPLATE.md) and take the next free number; the template's own
ADRs are 0001 to 0005. The tier rules are in [ADR-0003](0003-decision-tiers.md).

<!-- @generated begin adr-index -->
| ADR | Title | Tier | Status |
|---|---|---|---|
| [ADR-0001](0001-docs-as-checked-graph.md) | Documentation as a checked graph | agent | accepted |
| [ADR-0002](0002-review-gate-and-governance.md) | Review gate and governance | maintainer | accepted |
| [ADR-0003](0003-decision-tiers.md) | Decision tiers | maintainer | accepted |
| [ADR-0004](0004-ci-drafts-platform-pins.md) | CI: drafts skip, platform jobs after review, SHA pins | agent | accepted |
| [ADR-0005](0005-waves-and-serial-lanes.md) | Wave branches and serial lanes | agent | accepted |
<!-- @generated end adr-index -->
