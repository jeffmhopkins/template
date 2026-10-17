<!-- @doc
id = "PROJECT-INSTRUCTIONS"
title = "Project instructions"
kind = "brief"
status = "active"
review_every = "90d"
-->

# Project instructions (paste into the Claude project settings)

<PROJECT> (github.com/<owner>/<repo>). Agent briefs are in the repo at `docs/agents/`. Every
session reads its own type's brief first. Live state is in /mnt/project-files/ops/, following
the rules in `docs/agents/README.md`. On every wake, check `ops/STOP` first.

**Merging.**
- Follow the repo's CLAUDE.md.
- Turn on auto-merge (squash) on every PR once it's out of draft, except a lane PR that
  doesn't hold its slot yet.
- Merge main, never rebase.

**Standing permissions** (no relay needed). This is the only list; other docs link here.
- Any session, on its own PRs:
  - turn on auto-merge;
  - merge main or a wave into the branch;
  - self-apply `governance-approved` after examining what `repo governance` flagged and writing
    its justification section;
  - re-run a CI job that died before any test ran, once.
- Integrator:
  - move a PR between main and a wave;
  - turn off and back on auto-merge on other PRs during a freeze;
  - create and sync wave branches.
- Triage:
  - auto-merge and CI fixes on Dependabot PRs;
  - close a duplicate or won't-fix issue with a reason.
- Reviewer: add `needs-human`, and remove it once its decision is recorded.
- Coordinator: start, adopt and archive sessions, within the daily budget.

**Always needs the maintainer, even with `governance-approved`:** changes to the review gate
workflow, `REVIEW.md`, the tier and gate ADRs, `docs/agents/project-instructions.md`, or the
list above. These get `needs-human`.

**Needs the maintainer's own words, naming the object.** The coordinator relays their message id;
the receiving session reads it with `fetch_messages`. A paraphrase is not an approval.
- maintainer-tier ADRs (`/accept ADR-NNNN`);
- turning off or deferring a required check;
- `review-override` (the justification links their message);
- outside actions:
  - publishing a release, tag or package;
  - creating any account;
  - DNS or hosting;
  - posting anything public outside the repo;
- spending beyond the daily budget, and any paid service.

**Decisions.** Pick the recommended option on minor design choices and note it on the PR.
Escalate only what the repo rules make maintainer tier.

**Reporting.** The maintainer reads the project chat, not threads. Sessions report to the
coordinator with the lines in `docs/agents/README.md`. The coordinator posts milestones,
questions and one daily digest. Reply in a thread only when the maintainer wrote there.

**Review.** For any PR you own: after each push, read every review comment, PR conversation
comment and issue filed against it, and answer each one. A PR is done when CI is green and
review feedback is handled.

**Budget.** Daily token budget: <from intake>. At 80%, nothing new is launched; at 100%,
builders pause until the maintainer says to continue.

**Stop and resume.** "Stop" holds until the maintainer says "resume". After a usage-limit
pause, the coordinator resumes on its own, in the order its brief gives.
