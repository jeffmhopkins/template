<!-- @doc
id = "ADR-0003"
title = "Decision tiers"
kind = "adr"
status = "accepted"
tier = "maintainer"
-->

# ADR-0003: Decision tiers

<!-- @generated begin adr-status -->
> **Status:** accepted
<!-- @generated end adr-status -->

<a id="context"></a>
## Context
Agents make most decisions themselves. A few bind the maintainer: what the product is,
what it costs, what it promises and what it may do with users' data. Those need the
maintainer's own decision, and nothing else should wait on them.

<a id="decision"></a>
## Decision
Every ADR declares a `tier` in its `@doc` block.

- **`tier = "maintainer"`**: a decision about
  - **product scope**: which features are in or out, and which platforms are supported;
  - **licensing**: the project's licence, and accepting a dependency or asset licence the
    project doesn't already accept;
  - **security and privacy posture**: what data leaves the user's machine, and what the
    product, a client or a plugin may access;
  - **public compatibility promises** made to users and third parties;
  - **spending**: paid services, certificates, hardware, and budget beyond the daily limit;
  - **this tier list and the decision-comment rules below.**

  It opens as `status = "proposed"` and the PR gets `needs-human`. The maintainer decides
  with a decision comment, and the builder sets `status` to match.
- **`tier = "agent"`**: everything else, including architecture, internal design, the review
  process, the docs system, CI and rules, and amending or superseding any ADR whose subject
  is not on the list above. It lands as `accepted` after review. Changes that weaken a
  check still need `governance-approved` and its justification.
- **Classification.** The tier follows what the ADR decides, not what it touches. The
  builder picks it and the reviewer checks it. Too low a tier is blocking; too high is
  non-blocking. An agent-tier ADR that contains one maintainer-level question splits that
  question into its own ADR.
- **Decision comments**, posted on the PR:
  - **Direct:** the maintainer comments `/accept ADR-NNNN` or `/reject ADR-NNNN`.
  - **Proxy:** an agent relays the maintainer's explicit instruction:
    ```
    /accept ADR-NNNN
    on-behalf-of: @<maintainer>
    instruction: "<the maintainer's words, quoted>"
    source: <link to the maintainer's message, or its message id>
    proxy-session: <the relaying agent's session id>
    ```
  The instruction names the specific ADR. Standing or blanket instructions are not
  decisions. An agent never posts a decision the maintainer didn't give.

<a id="consequences"></a>
## Consequences
- The maintainer sees scope, licence, security and privacy, compatibility and spending
  decisions before they bind, and nothing else.
- Agents revise architecture and process on their own; the review gate is the check on
  them.
- A maintainer-tier ADR blocks its PR until the decision is on it.
