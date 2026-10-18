<!-- @doc
id = "ADR-0008"
title = "Documentation as a checked graph, v2"
kind = "adr"
status = "accepted"
tier = "agent"
-->

# ADR-0008: Documentation as a checked graph, v2

<!-- @generated begin adr-status -->
> **Status:** accepted
<!-- @generated end adr-status -->

> ADR-0001's decision, restated with two corrections: the ledger is not a protected path,
> and progress lives in the pinned progress issue, not in a generated README block. It
> supersedes ADR-0001.

<a id="context"></a>
## Context
[ADR-0001](0001-docs-as-checked-graph.md) made the docs a graph of anchored sections bound
by hashes to what they describe. Two of its statements don't match the repo, and shouldn't:

- It says the ledger `docs/verified.toml` is a protected path. It isn't. Almost every PR
  restamps a section, so protecting the ledger would make almost every PR a governance
  change, and the label would stop meaning anything. The ledger's safeguards are elsewhere:
  only `docs verify` and `docs merge-main` write it, `docs check` fails on any other form,
  stock notes are refused, and the reviewer judges every note.
- It says derivable content is generated and drift-checked. The README's `progress` block
  was generated and never drift-checked: it changes with every merge to `main`, so a check
  would fail PRs for merges they had nothing to do with, and an unchecked copy goes stale.
  The pinned progress issue, rewritten on every push to `main`, already shows the same table.

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
4. The ledger is not a protected path. Only `docs verify` and `docs merge-main` write it,
   and `docs check` fails on a ledger in any other form.
5. `docs merge-main` resolves ledger and generated-block conflicts by recomputing hashes.
6. No history in files: revisions, dates and authors come from git.
7. Content derivable from the tree is generated and drift-checked. Content derived from
   history, such as roadmap status, is never committed: it is printed by the tool and shown
   in a pinned issue that a workflow rewrites.
8. Facts are registered once. External URLs are registered with what the docs rely on from
   them.
9. Time-based review never blocks a PR. The weekly sweep lists what is due.
10. Rules have permanent IDs and declare their enforcement from a fixed vocabulary that the
    tool checks.
11. Roadmap progress is derived from commit trailers on `main`, never recorded.

Decisions are recorded as ADRs, following the [ADR practice](https://adr.github.io/).

<a id="consequences"></a>
## Consequences

### Positive
- A change that makes a section wrong can't merge until someone re-reads that section.
- Restamping stays an ordinary part of a PR, not a governance change.
- No committed table shows a status that `main` has moved past.

### Negative
- Every PR that touches covered code carries restamps, and the reviewer reads each note.
- Broad `covers` cause re-verification on most PRs; they are narrowed, as a declared
  governance change.
- The tool and `DOCS.md` are protected paths. A bad restamp is caught by the reviewer, not
  by the governance check.
- Progress is read on GitHub or with `scripts/repo progress`, not in the README.
