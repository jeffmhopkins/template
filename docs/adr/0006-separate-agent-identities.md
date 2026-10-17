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

> Builders and reviewers each act through a GitHub App of their own, and `review-gate` is
> posted by an identity no PR controls, so the maintainer's login means a human and neither
> a verdict nor the gate can be forged.

<a id="context"></a>
## Context
Every agent pushes, comments and labels under the maintainer's GitHub login
([ADR-0002](0002-review-gate-and-governance.md)). The checks that ask "did the maintainer do
this" compare a login, so an agent applying `review-override` or `governance-approved`
passes them. The rule that agents never do so, and the `source:` check of decisions
([ADR-0003](0003-decision-tiers.md)), are process controls.

The gate counts verdict markers from `REVIEWER_LOGIN`. Every agent posts as that login, so a
builder can post a marker the gate counts. One App shared by all agents would not change
that: the reviewers need an identity the builders can't use.

The required checks are pinned to the GitHub Actions app. A PR that changes a workflow runs
it as that app, which can post any status or check name on its own head, `review-gate`
included.

<a id="decision"></a>
## Decision
- **A builder identity.** A GitHub App, installed only on the project's repository, with
  contents, pull requests, issues and statuses access. Every session other than a reviewer
  pushes, comments and labels as this App. The maintainer's login is used only by the
  maintainer.
- **A reviewer identity.** A second App, with pull requests and issues access only, whose
  key only reviewer sessions hold. `REVIEWER_LOGIN` is its bot login, so the gate counts
  only markers a reviewer posted. Session ids are then needed only for recusal.
- **Label checks compare actors.** `review-gate` and `governance` count `review-override`, and
  `governance-approved` on maintainer-only paths, only when applied by `MAINTAINER_LOGIN`,
  which then means the maintainer.
- **A gate identity.** `review-gate` is posted as a check run by a third App, whose key is
  held only by the gate workflow on the default branch, through an environment that PR
  workflows can't use. The ruleset pins `review-gate` and `governance` to that App's
  `integration_id`, so a status from GitHub Actions no longer satisfies them.
- **Bootstrap** creates the three Apps from manifests, stores the gate App's key in the
  repository's secret store, gives the builder and reviewer keys to the maintainer for the
  sessions that use them, and sets the rulesets.

<a id="consequences"></a>
## Consequences

### Positive
- A label or a comment from the maintainer's login is the maintainer's.
- A PR's own workflow can't pass the gate or the governance check.
- A builder can't post a verdict marker the gate counts.
- Direct decisions on GitHub need no `source:` check.

### Negative
- Bootstrap creates three Apps and handles their private keys; the maintainer installs them
  and rotates the keys.
- Agent sessions need their App's token in place of the maintainer's sign-in, which every
  brief and the device setup must say how to get. A reviewer's key must stay out of every
  builder's reach.
- Rulesets differ between the template and projects until a project adopts this.
