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

> Scope, licensing, security and privacy, public compatibility and spending wait for the
> maintainer's own decision. Agents decide everything else.

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
- **Decision comments**, posted on the PR. Each carries a `source:`: the message id of the
  maintainer's own message in the project that gives the decision.
  - **Direct:** the maintainer comments `/accept ADR-NNNN` or `/reject ADR-NNNN`, with
    `source: <message id>` on the next line.
  - **Proxy:** an agent relays the maintainer's explicit instruction:
    ```
    /accept ADR-NNNN
    on-behalf-of: @<maintainer>
    instruction: "<the maintainer's words, quoted>"
    source: <the maintainer's message id>
    proxy-session: <the relaying agent's session id>
    ```
  - **Maintainer-only paths:** the same fields, with `/approve <path>` as the first line,
    one per changed path or `[governance].maintainer_only` glob; the instruction names the
    PR or the path.
  The instruction names the specific ADR, or the PR or path. Standing or blanket instructions
  are not decisions. A decision card's answer counts for an ADR only when the card's
  question or context lists that ADR's id. An agent never posts a decision the maintainer
  didn't give.
- **Checking a decision.** Agents post under the maintainer's GitHub login, so the login
  proves nothing. The reviewer itself, never a sub-worker, reads each `source:` with
  `fetch_messages`: the author is a `user` whose `author_id` equals the maintainer's user id
  recorded on day 0 in `ops/maintainer.md`, and the words name that ADR, PR or path. A
  decision whose source fails any of these is invalid.

<a id="consequences"></a>
## Consequences

### Positive
- The maintainer sees scope, licence, security and privacy, compatibility and spending
  decisions before they bind, and nothing else.
- Agents revise architecture and process on their own; the review gate is the check on
  them.

### Negative
- A maintainer-tier ADR blocks its PR until the decision is on it.
