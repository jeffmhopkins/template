# Template changelog

This is the template's own changelog: changes to the template, by template version. The
version is `[template].version` in `repo.toml`. In a project made from the template, intake
replaces this file with the project's own changelog, and template upgrades read this one at
the template's tag.

## 0.1.0

- The repo skeleton: CLAUDE.md, REVIEW.md, the docs system, the rules, ADRs 0001 to 0005,
  the runbooks and the agent briefs.
- Template mode and project mode in CLAUDE.md, from `[template].source` in `repo.toml`.
- The template-upgrade runbook.
- The `dependency-review` required check; bootstrap enables secret scanning and push
  protection.
- `.editorconfig` and `.gitattributes`.
- `docs/SECURITY-AGENTS.md`.
