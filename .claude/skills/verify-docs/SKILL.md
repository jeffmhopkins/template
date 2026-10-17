---
name: verify-docs
description: How to re-read and restamp the doc sections a change or a merge made stale. Use when `scripts/repo docs check` reports stale sections, after `scripts/repo docs merge-main`, or before pushing a change that touches covered code.
---
<!-- @doc
id = "SKILL-VERIFY-DOCS"
title = "Verify-docs skill"
kind = "guide"
status = "active"
review_every = "90d"
-->

# Verify docs

A doc section is stale when its text, a file it covers or a section it depends on changed
since it was last verified. Restamping it is a claim that it was re-read against that change
and still holds.

<a id="find"></a>
## Find what is stale
<!-- @section
depends_on = ["DOCS#ledger"]
-->

Run `scripts/repo docs check`. For each stale section it names the section
(`DOC#anchor`) and the inputs that changed:
- `body`: the section's own text;
- `file:<path>`: a file its `covers` match (added, removed, renamed or edited);
- `dep:<DOC#anchor>`: a section or rule it depends on.

After `scripts/repo docs merge-main`, the list it prints is the same set, with the notes this
branch already wrote on each section.

<a id="reread"></a>
## Re-read each section
<!-- @section
depends_on = ["DOCS#notes"]
-->

For each stale section, one at a time:

1. Open the section and read it whole.
2. Read what changed in each named input: `git diff origin/main -- <path>` for a file, the
   dependency's text for a `dep:`, your own edit for `body`.
3. Check every claim in the section against the change: names, flags, paths, defaults,
   behaviour, counts, examples.
4. **If anything is wrong, edit the section** until it holds. Never verify a section you
   know is wrong.
5. Verify it:
   ```sh
   scripts/repo docs verify <DOC#anchor> --by agent:<name> --note "<what you checked>"
   ```
   `<name>` is your thread's name.

Re-run `scripts/repo docs check` until it is clean. Never edit `docs/verified.toml` by hand,
and never hand-edit an `@generated` block; run `scripts/repo docs generate`.

<a id="notes"></a>
## Writing the note
<!-- @section
depends_on = ["DOCS#notes"]
-->

The note names what changed and what in the section you checked against it. The reviewer
re-reads each verified section and judges each note; one that doesn't engage with the
change is a blocking finding.

Good notes:
- `"Added --json to docs check; the command table and exit codes still match main.rs"`
- `"Renamed the config key to max_cover_files; updated it in step 3, the rest holds"`
- `"Merge brought main's new lint job; earlier note (added --draft) still holds, job list updated"`

Rejected by the tool: fewer than 20 characters, or only "ok", "lgtm", "verified",
"checked" or "re-read".

Weak, and flagged by the reviewer: a note that only restates the section's title, one note
copied across many sections, and a merge note that repeats the earlier note without saying
what the merge changed.

<a id="merges"></a>
## After a merge
<!-- @section
depends_on = ["DOCS#merge"]
-->

Each section `docs merge-main` lists gets its own fresh note with two parts: what the
earlier note said about this PR's change, and what the merge changed in its inputs. A
section the merge didn't touch in substance still gets a note saying which input moved and
that the section's claims hold against it.

<a id="review"></a>
## Time-based review
<!-- @section
depends_on = ["DOCS#ttl"]
-->

A doc past its `review_every` is listed in the weekly sweep issue, and never blocks a PR.
To clear it, re-read the whole doc and run
`scripts/repo docs review <DOC> --by agent:<name> --note "<what you checked>"`, with the same
standard for the note.
