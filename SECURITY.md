<!-- @doc
id = "SECURITY"
title = "Security policy"
kind = "guide"
status = "active"
review_every = "180d"
-->

# Security policy

<a id="reporting"></a>
## Reporting a vulnerability
<!-- @section
-->

Report a vulnerability privately, through the repository's **Security** tab ("Report a
vulnerability"), following GitHub's
[private reporting guide](https://docs.github.com/en/code-security/security-advisories/guidance-on-reporting-and-writing-information-about-vulnerabilities/privately-reporting-a-security-vulnerability).
Never open a public issue for one. Include the affected version or commit, the steps to
reproduce, and the impact you see.

<a id="supported"></a>
## Supported versions
<!-- @section
-->

Until the first release, only the latest commit on `main` is supported. After it, the
supported versions are listed here.

<a id="posture"></a>
## Security posture
<!-- @section
-->

The rules are `R-SEC-*` in [docs/RULES.md](docs/RULES.md#security). The product's security
and privacy posture is in [docs/PRODUCT.md](docs/PRODUCT.md#never), set by the maintainer.
How the agents that build it treat untrusted text, approvals and secrets is in
[docs/SECURITY-AGENTS.md](docs/SECURITY-AGENTS.md).
