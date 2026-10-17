<!-- @doc
id = "ADR-0006"
title = "Separate GitHub identities for agents and the gate"
kind = "adr"
status = "proposed"
tier = "maintainer"
-->

# ADR-0006: Separate GitHub identities for agents and the gate

<!-- @generated begin adr-status -->
> **Status:** proposed
<!-- @generated end adr-status -->

> Agents act through a GitHub App of their own, and `review-gate` is posted by an identity no
> PR controls, so the maintainer's login means a human and the gate can't be forged.

<a id="context"></a>
## Context
Every agent pushes, comments and labels under the maintainer's GitHub login
([ADR-0002](0002-review-gate-and-governance.md)). The checks that ask "did the maintainer do
this" compare a login, so an agent applying `review-override` or `governance-approved`
passes them. The rule that agents never do so, and the `source:` check of decisions
([ADR-0003](0003-decision-tiers.md)), are process controls.

The required checks are pinned to the GitHub Actions app. A PR that changes a workflow runs
it as that app, which can post any status or check name on its own head, `review-gate`
included.

<a id="decision"></a>
## Decision
- **An agent identity.** A GitHub App, installed only on the project's repository, with
  contents, pull requests, issues and statuses access. Every agent session pushes, comments
  and labels as the App. The maintainer's login is used only by the maintainer.
- **Label checks compare actors.** `review-gate` and `governance` count `review-override`, and
  `governance-approved` on maintainer-only paths, only when applied by `MAINTAINER_LOGIN`,
  which then means the maintainer.
- **A gate identity.** `review-gate` is posted as a check run by a second App, whose key is
  held only by the gate workflow on the default branch, through an environment that PR
  workflows can't use. The ruleset pins `review-gate` and `governance` to that App's
  `integration_id`, so a status from GitHub Actions no longer satisfies them.
- **Bootstrap** creates both Apps from a manifest, stores their keys in the repository's
  secret store, and sets the rulesets.

<a id="consequences"></a>
## Consequences

### Positive
- A label or a comment from the maintainer's login is the maintainer's.
- A PR's own workflow can't pass the gate or the governance check.
- Direct decisions on GitHub need no `source:` check.

### Negative
- Bootstrap creates two Apps and handles their private keys; the maintainer installs them
  and rotates the keys.
- Agent sessions need the App's token in place of the maintainer's sign-in, which every
  brief and the device setup must say how to get.
- Rulesets differ between the template and projects until a project adopts this.
