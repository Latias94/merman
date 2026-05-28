# ASCII Renderer Productization

Status: Complete
Last updated: 2026-05-28

## Why This Lane Exists

`merman` needs an ASCII/Unicode output target that library users can depend on in terminals,
documentation pipelines, logs, chat surfaces, and restricted environments where SVG is not useful.
The reference `repo-ref/mermaid-ascii` contains a practical graph and sequence rendering algorithm,
but productizing it in `merman` requires a Rust crate boundary, public API design, tests, provenance,
and explicit unsupported-feature behavior.

## Relevant Authority

- `docs/workstreams/PARITY_BOUNDARY.md`
- `docs/adr/0065-ascii-output-boundary.md`
- `repo-ref/mermaid-ascii` at commit `6fffb8e` from
  `https://github.com/AlexanderGrooff/mermaid-ascii`
- `repo-ref/mermaid-ascii/LICENSE` (MIT)
- `crates/merman-core/src/diagrams/flowchart/model.rs`
- `crates/merman-core/src/diagrams/sequence/render_model.rs`
- Root workspace license: `MIT OR Apache-2.0`

## Problem

There is no tracked, public ASCII rendering surface in `merman`. A direct port of
`mermaid-ascii` would create the wrong architecture because the Go project owns parsing, application
entry points, and rendering together. That would duplicate Mermaid parsing, hide unsupported
features, and leave license/test provenance dependent on the gitignored `repo-ref/` directory.

## Target State

- A new `crates/merman-ascii` crate owns terminal/text rendering.
- `merman-ascii` consumes typed diagram models from `merman-core`.
- The first supported families are flowchart and sequence diagrams.
- The renderer exposes a stable options/errors API suitable for libraries and CLI use.
- Golden fixtures copied or derived from `mermaid-ascii` live in tracked paths with source commit
  and MIT license attribution.
- Unsupported Mermaid features are reported, degraded explicitly, or deferred in a documented
  compatibility table.
- The top-level `merman` crate can expose ASCII rendering behind an opt-in `ascii` feature.

## In Scope

- `docs/adr/0065-ascii-output-boundary.md`
- `docs/workstreams/ascii-renderer-productization/*`
- Future `crates/merman-ascii`
- Future `merman` feature wiring for ASCII APIs
- Future `merman-cli` text output integration after the library API is proven
- Tracked third-party notice/license files for `mermaid-ascii`
- Copied golden fixtures needed for CI and release-source verification

## Out Of Scope

- A second Mermaid parser.
- Browser, SVG, or raster layout reuse for ASCII coordinates.
- Pixel parity with Mermaid CLI.
- Full Mermaid feature coverage in the first implementation slice.
- Product-specific styling, colors, themes, or host application semantics.

## Starting Assumptions

| Assumption | Confidence | Evidence | Consequence if wrong |
| --- | --- | --- | --- |
| ASCII rendering should be a separate crate. | High | It has separate layout rules, public API, tests, and license provenance. | Fold back into `merman` only if API surface proves too small. |
| The Go algorithm is worth porting, but not its parser. | High | `merman-core` already owns typed Mermaid models; `mermaid-ascii` parser is project-local. | Re-evaluate only if current models lack required semantic facts. |
| Flowchart should land before sequence. | High | Flowchart graph routing is the larger reusable primitive and has more golden coverage. | Swap order if sequence proves much cheaper for API validation. |
| Golden outputs are stable public behavior. | High | Downstream users will snapshot terminal output. | Treat output changes as semver-sensitive once released. |
| `unicode-width` is the right width primitive. | Medium | Workspace already depends on `unicode-width`; Go reference uses runewidth behavior. | Add targeted fixtures if CJK/emoji behavior differs. |

## Architecture Direction

Dependency direction:

```text
merman-core
  ^
  |
merman-ascii
  ^
  |
merman feature ascii
  ^
  |
merman-cli --format ascii/unicode
```

Initial internal modules:

```text
crates/merman-ascii/src/
  lib.rs
  error.rs
  options.rs
  text.rs
  canvas.rs
  graph/
    model.rs
    layout.rs
    route.rs
    draw.rs
  sequence/
    layout.rs
    draw.rs
```

`merman-ascii` should adapt `FlowchartV2Model` into an internal `AsciiGraph` and
`SequenceDiagramRenderModel` into an internal `AsciiSequence`. Internal graph/sequence models should
be private unless tests require narrow helper visibility. Public users should depend on rendering
functions and options, not on cell-routing internals.

## Fearless Refactor Brief

Intent: remove the future complexity of mixing parser duplication, SVG layout assumptions, and
ad-hoc terminal rendering into the main `merman` crate.

Scope: new ASCII crate, top-level feature wiring, future CLI format support, tracked fixtures,
tracked third-party notices, and documentation.

Deletion plan: do not port Go parser, Cobra/Gin/web entry points, ad-hoc application config, or any
test path that only reads from gitignored `repo-ref/`.

Boundary plan: keep `merman-core` as semantic authority, keep `merman-render` SVG-specific, and make
`merman-ascii` own character-cell layout and routing.

Testing plan: start with copied upstream golden fixtures, then add merman-native semantic fixtures,
CJK/emoji width tests, unsupported-feature tests, and public API smoke tests.

Risk plan: guard license provenance, output stability, unsupported feature diagnostics, and dense
graph routing limits before shipping a stable public API.

Workflow plan: this is a durable workstream. Implementation should proceed through bounded
`ARP-*` tasks with review and fresh verification before completion claims.

## Closeout Condition

This lane can close when flowchart and sequence ASCII/Unicode rendering are available through the
library API, CLI integration is either shipped or split into a narrower follow-on, tracked license
and fixture provenance are complete, docs describe supported behavior, and all closeout gates have
fresh evidence.

Closeout status: satisfied on 2026-05-28. The lane shipped `merman-ascii`, top-level
`merman --features ascii` APIs, `merman-cli render --format ascii|unicode`, tracked upstream
license/fixture provenance, support matrices, README/CHANGELOG entries, and fresh closeout gate
evidence.
