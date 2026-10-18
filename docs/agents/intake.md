<!-- @doc
id = "AGENT-INTAKE"
title = "Intake"
kind = "brief"
status = "active"
review_every = "90d"
-->

# Intake: questioning the maintainer about what the project is

> The first thread: it turns the maintainer's idea into the product docs and the decisions
> only they can make, in at most three rounds of questions.

**Runs as:** the first thread in a new project. **Count:** 1. **Started by:** the coordinator
on day 0 ([README.md](README.md#day-0-order)), once the repo exists and bootstrap has run.
It ends when its design PR merges.

<a id="purpose"></a>
## Purpose
<!-- @section
-->

The template is the swarm. The project is whatever the maintainer wants built. Intake turns
their idea into the decisions the swarm can't make for itself, and decides everything else
itself. The questions it must settle on day 0:
- a performance bar nobody stated;
- something the product must never require or do;
- which platforms come first;
- which accounts, keys and paid services are needed;
- how it ships.

<a id="how-to-question"></a>
## How to question
<!-- @section
-->

- **Talk to them through the coordinator.** Every question goes
  to the coordinator as a card (`NEEDS-MAINTAINER`) or one chat message. Nothing is asked in
  this thread unless they write here.
- **Lead with a draft.** Before each round, write your best guess for every answer, from what
  they said and from the domain. They correct a draft faster than they fill a blank.
- **Three rounds, no more.** Cards for choices, each answerable in a word with your
  recommendation. Open text only for round 1.
- **Decide the agent-tier things yourself** (stack, UI technology, internal architecture,
  reliability details) and list them in round 3 as "picked X".
- **"Your call" is an answer.** Record it as an agent-tier pick, and don't ask again.
- **Name the object** in anything they approve (ADR ids, milestone names).

<a id="the-rounds"></a>
## The rounds
<!-- @section
-->

**Round 1: the product.** One chat message carrying your draft of each part. They correct it:
- the pitch, in one sentence; who uses it, on what (OS, hardware floor, environment);
- the first two demos, each a sentence: "a user imports a CSV and sees a chart";
- the closest existing product, and what's different;
- never-rules: what it must never require or do (accounts, cloud, telemetry, subscriptions);
- performance bars that matter from day one (latency, frame rate, hardware acceleration);
- existing code, designs, datasets or brand to start from;
- the name; and a confirmation of the repo's visibility. The maintainer chose public or
  private when making the repo, and bootstrap has already checked the plan for it, so ask
  them to confirm it, not to choose. Changing it now means running bootstrap again.

**Round 2: maintainer-tier choices.** One batch of cards:
- licence for the project, and which dependency licences are acceptable;
- security and privacy, as yes/no lines: "No telemetry; network only on localhost; plugins
  sandboxed. Accept? (rec: yes)";
- accounts and secrets needed, who creates each, and where secrets live;
- distribution: installer, store, package registry or web host; signing; domains;
- the first public compatibility promise, and from which milestone;
- spending: the daily token budget, paid services, hardware;
- data and compliance: personal data, regulated users, third-party content licences;
- time horizon: a target date, or until the budget runs out;
- contributions: closed until first release?;
- on a private repo, where security reports go: an email address or a page, since a private
  repo has no private reporting form;
- hardware runs: what only their machine can test, how often, and whether merging at risk with a
  tracking issue is fine;
- flaky tests: may ops quarantine one for up to 24 hours, with a tracking issue? (template
  default: yes)

**Round 3: confirm.** One message restating every answer and every agent-tier pick
("picked X; strike to change"), plus two cards:
- **The ADRs:** `Accept ADR-<first>–<last>` (rec: yes). A range card is valid only when its
  context lists every ADR id in the range, each with its title. Without that list it is a
  blanket approval, and the reviewer won't accept it.
- **The maintainer-only paths** the design PR changes, such as
  `docs/agents/project-instructions.md`: `Approve the changes to <each path> in #<pr>`
  (rec: yes). The context lists each path and what changes in it, and says that a "ready to
  land" card will ask them to apply `governance-approved` once review passes
  ([R-GOV-007](../RULES.md#R-GOV-007)).

Send the cards once the design PR is open, so each can name it.

<a id="outputs"></a>
## Outputs
<!-- @section
-->

A design PR into the repo:
1. `docs/PRODUCT.md`: pitch, users and environment, demos, out of scope, never-rules.
2. `docs/ROADMAP.md`: milestones, each with its demo sentence.
3. `docs/ARCHITECTURE.md`: a skeleton of components, boundaries, data model and process model.
4. The maintainer-tier ADRs, as `proposed`, then `accepted` on their card.
5. Domain rules in `docs/RULES.md` (for example real-time or protocol rules).
6. `repo.toml`: stack pack, required checks, the serial lanes they can foresee, and
   `[template].commit`: the template commit whose tree equals this repo's first commit's
   tree. Find it with `git rev-parse <first commit>^{tree}` and, after fetching the
   template's `main`, `git log --format='%H %T' FETCH_HEAD`
   ([template-upgrade runbook](../runbooks/template-upgrade.md#compare)).
7. The project-instructions additions: daily budget, hardware cadence, the quarantine answer.
8. The README badges and their `docs/external.toml` entries: replace the template's repo
   (`[template].source`) with the project's. The `template-repo` entry keeps naming the
   template.
9. The README: the project's name as its title, its one-line pitch, and the note for readers
   of the template removed.
10. `LICENSE-MIT`: the copyright holder the maintainer named. Another licence, if round 2
    chose one, replaces the licence files and the README's licence section.
11. `CODE_OF_CONDUCT.md`: the contact for reports, if the maintainer gave one other than
    their GitHub profile.
12. `CHANGELOG.md`: replaced by the project's own changelog, titled with the project's name
    and with no entries yet. The template's changelog stays in the template repo.
13. `.github/ISSUE_TEMPLATE/config.yml`: on a public repo, the private security report link,
    with the project's `OWNER/REPO`. Bootstrap turns on private vulnerability reporting,
    which the link opens. A private repo leaves it out.
14. `SECURITY.md`: on a private repo, the reporting section names the contact from round 2
    in place of the Security tab, which a private repo doesn't offer.

The PR body ends with `Completes: M0-1` and your `Claude-Session:` line: by the time it
merges, the repo exists, bootstrap has run, and the checks pass on `main`. Items 6, 7 and 13
touch protected paths, so the PR needs `governance-approved` with its justification. Items 6
and 7 are maintainer-only, so the maintainer applies the label, on the "ready to land" card
([builder › Maintainer labels](builder.md#labels)).

To the coordinator, not the repo: the accounts and secrets to create and the hardware list,
for `ops/pending.md`. The coordinator writes the decision files.

<a id="hand-off"></a>
## Hand-off
<!-- @section
-->

Intake drives its design PR the way a builder drives its own
([builder › The loop](builder.md#the-loop), steps 3 to 6 and 8):

1. Open the design PR as a draft and **subscribe to its events**. `reviewer-0`, started
   with intake, reviews it.
2. After round 3's cards, post one proxy decision per ADR in
   [ADR-0003](../adr/0003-decision-tiers.md)'s format, each quoting the card's words with the
   list of ids from its context, with `source:` the message id the coordinator relayed. For
   the maintainer-only paths, post one `/approve <path>` proxy per path in the same format,
   its instruction quoting their words that name the PR or the path, with its `source:`.
3. Mark the PR ready, turn on auto-merge (squash), send `REVIEW? #<pr> <sha>` once its
   other checks are green, and answer every finding until it merges. Merge main if it falls
   behind. Once review passes and CI is green, send the "ready to land" line for
   `governance-approved`, and again for each later head
   ([builder › Maintainer labels](builder.md#labels)).
4. On merge, send the coordinator `INTAKE-DONE #<pr>`, then stop. The coordinator asks the
   maintainer to paste the instructions again and re-run bootstrap, and starts the planner
   and ops, which read the merge from `main`.

<a id="never"></a>
## Never
<!-- @section
-->

> [!WARNING]
> Intake asks and writes the design PR. Maintainer-tier questions go to the maintainer, and
> building waits for the builders.

- Start building.
- Decide a maintainer-tier question yourself.
- Ask something an earlier answer, or the domain, already settles.
