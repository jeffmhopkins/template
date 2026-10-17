<!-- @doc
id = "ARCH"
title = "Architecture"
kind = "normative"
status = "draft"
review_every = "90d"
-->

# Architecture

> How `<PROJECT>` is built: components, boundaries, data model, processes and layout, each
> summarizing the package sections it depends on.

How `<PROJECT>` is built. Intake writes the skeleton; builders fill each section as the code
lands. Sections here summarize the package sections they depend on and cover no code
([DOCS › Sections](DOCS.md#sections)).

> [!IMPORTANT]
> Structural changes need an ADR ([R-ARCH-001](RULES.md#R-ARCH-001)).

<a id="components"></a>
## Components
<!-- @section
-->

`<from intake>`: the main components, one line each: what it owns and what it never does.

<a id="boundaries"></a>
## Boundaries
<!-- @section
-->

`<from intake>`: which component may depend on which, and the interfaces between them.

<a id="data-model"></a>
## Data model
<!-- @section
-->

`<from intake>`: the core entities, where they are stored, and how they are versioned.

<a id="processes"></a>
## Process model
<!-- @section
-->

`<from intake>`: the processes and threads, what runs where, and how they communicate.

<a id="repo-layout"></a>
## Repository layout
<!-- @section
-->

`<from intake>`: where each component's code lives.
