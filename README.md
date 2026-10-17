# template

A repo to fork for large build projects run by a swarm of AI agents. The template is the
swarm: its rules, review gate, tooling and agent roles. Each new project's content comes
from an intake agent questioning the maintainer.

**Status: design.** This branch holds the plan for review. Nothing in it is built yet.

- [docs/design/template-design.md](docs/design/template-design.md): what the template is,
  how it works, and the open questions for the maintainer (§6).
- [docs/agents/](docs/agents/README.md): one brief per agent type, the message formats, the
  state-file rules and the project instructions to paste into Claude.

Once the design is approved, it gets built in the order given in the design's §5.
