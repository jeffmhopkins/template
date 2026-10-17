<!-- @doc
id = "ADR-0001"
title = "Documentation as a checked graph"
kind = "adr"
status = "accepted"
tier = "agent"
-->

# ADR-0001: Documentation as a checked graph

<!-- @generated begin adr-status -->
> **Status:** accepted
<!-- @generated end adr-status -->

<a id="context"></a>
## Context
Many agents change the code in parallel, and each reads the docs to decide what to do. A doc
that no longer matches the code misleads every agent that reads it. Nothing in git alone
says which docs a change made wrong.

<a id="decision"></a>
## Decision
Documentation is a graph of anchored sections, checked by `scripts/repo docs`, as specified
in [DOCS](../DOCS.md):

1. Every tracked Markdown file starts with a `@doc` metadata block (TOML in an HTML
   comment). Every `##` section has a permanent anchor and an `@section` block.
2. A section may declare `covers` (the code it describes, bounded to one package or
   `max_cover_files` files) and `depends_on` (sections, rules and ADRs it relies on).
3. The ledger `docs/verified.toml` holds one hash per input of each such section. A changed
   input makes the section stale, and the `docs` check fails until an agent re-reads it and
   restamps it with a note. The reviewer judges every note.
4. `docs merge-main` resolves ledger and generated-block conflicts by recomputing hashes.
5. No history in files: revisions, dates and authors come from git.
6. Derivable content is generated and drift-checked. Facts are registered once.
   External URLs are registered with what the docs rely on from them.
7. Time-based review never blocks a PR. The weekly sweep lists what is due.
8. Rules have permanent IDs and declare their enforcement from a fixed vocabulary that the
   tool checks.
9. Roadmap progress is derived from commit trailers on `main`, never recorded.

Decisions are recorded as ADRs, following the [ADR practice](https://adr.github.io/).

<a id="consequences"></a>
## Consequences
- A change that makes a section wrong can't merge until someone re-reads that section.
- Every PR that touches covered code carries restamps, and the reviewer reads each note.
- Broad `covers` cause re-verification on most PRs; they are narrowed, as a declared
  governance change.
- The tool, `DOCS.md` and the ledger are protected paths.
