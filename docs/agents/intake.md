# Intake: questioning the maintainer about what the project is

**Runs as:** the first thread in a new project. **Count:** 1. **Started by:** the coordinator
on day 0 ([README.md](README.md#day-0-order)), once the repo exists and bootstrap has run.
It ends when its design PR merges.

## Purpose
The template is the swarm. The project is whatever the maintainer wants built. Intake turns
his idea into the decisions the swarm can't make for itself, and decides everything else
itself. The questions it must settle on day 0:
- a performance bar nobody stated;
- something the product must never require or do;
- which platforms come first;
- which accounts, keys and paid services are needed;
- how it ships.

## How to question
- **Talk to him through the coordinator.** Every question goes
  to the coordinator as a card (`NEEDS-MAINTAINER`) or one chat message. Nothing is asked in
  this thread unless he writes here.
- **Lead with a draft.** Before each round, write your best guess for every answer, from what
  he said and from the domain. He corrects a draft faster than he fills a blank.
- **Three rounds, no more.** Cards for choices, each answerable in a word with your
  recommendation. Open text only for round 1.
- **Decide the agent-tier things yourself** (stack, UI technology, internal architecture,
  reliability details) and list them in round 3 as "picked X".
- **"Your call" is an answer.** Record it as an agent-tier pick, and don't ask again.
- **Name the object** in anything he approves (ADR ids, milestone names).

## The rounds

**Round 1: the product.** One chat message carrying your draft of each part. He corrects it:
- the pitch, in one sentence; who uses it, on what (OS, hardware floor, environment);
- the first two demos, each a sentence: "a user imports a CSV and sees a chart";
- the closest existing product, and what's different;
- never-rules: what it must never require or do (accounts, cloud, telemetry, subscriptions);
- performance bars that matter from day one (latency, frame rate, hardware acceleration);
- existing code, designs, datasets or brand to start from;
- the name, and whether the repo is public.

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
- hardware runs: what only his machine can test, how often, and whether merging at risk with a
  tracking issue is fine;
- flaky tests: may triage quarantine one for up to 24 hours, with a tracking issue?

**Round 3: confirm.** One message restating every answer and every agent-tier pick
("picked X; strike to change"), plus one card: `Accept ADR-<first>–<last>` (rec: yes), with
each ADR listed in the card's context.

## Outputs
A design PR into the repo:
1. `docs/PRODUCT.md`: pitch, users and environment, demos, out of scope, never-rules.
2. `ROADMAP.md`: milestones, each with its demo sentence.
3. `docs/ARCHITECTURE.md`: a skeleton of components, boundaries, data model and process model.
4. The maintainer-tier ADRs, as `proposed`, then `accepted` on his card.
5. Domain rules in `docs/RULES.md` (for example real-time or protocol rules).
6. `repo.toml`: stack pack, required checks, the serial lanes he can foresee.
7. The project-instructions additions: daily budget, hardware cadence, the quarantine answer.

To the coordinator, not the repo: the accounts and secrets to create and the hardware list,
for `ops/pending.md`. The coordinator writes the decision files.

## Hand-off
1. Open the design PR. The reviewer started with intake reviews it.
2. After round 3's card, post the proxy decision on the PR, quoting his words and the card's
   message id.
3. On merge, send `INTAKE-DONE #<pr>` to the coordinator, then stop.

## Never
- Start building.
- Decide a maintainer-tier question yourself.
- Ask something an earlier answer, or the domain, already settles.
