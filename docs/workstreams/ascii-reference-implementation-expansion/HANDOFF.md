# ASCII Reference Implementation Expansion — Handoff

Status: Complete
Last updated: 2026-05-30

## Current State

This lane governs how `merman-ascii` should learn from `repo-ref/mermaid-ascii` and
`repo-ref/beautiful-mermaid` while preserving the model-driven boundary. Reference provenance is
tracked. classDiagram ASCII now renders boxes plus an expanded single-relationship subset from
`RenderSemanticModel::Class`, and ER ASCII now renders entity boxes plus a first single-relationship
subset from `RenderSemanticModel::Er`. XYChart ASCII now renders compact bars, lines, mixed plots,
horizontal bars, and Unicode/ASCII chart characters from `RenderSemanticModel::XyChart`. Flowchart
delta triage against `beautiful-mermaid` is recorded, and thick edges now render from the typed
flowchart stroke model. Public integration is also wired: `merman::ascii` re-exports the shipped
typed helpers, `HeadlessAsciiRenderer`/`render_ascii_sync` render class, ER, and XYChart through the
typed `render_model` path, and CLI ASCII smoke coverage exercises those shipped families. This lane
is closed as of ARI-080.

## Closeout Task

- Task ID: ARI-080
- Owner: codex
- Files:
  - `docs/workstreams/ascii-reference-implementation-expansion/*`
- Validation: `cargo nextest run -p merman-ascii`;
  `cargo nextest run -p merman --features ascii`;
  `cargo nextest run -p merman-cli --features ascii`;
  `cargo fmt --all --check`;
  `cargo clippy -p merman-ascii -p merman --features ascii --all-targets -- -D warnings`;
  `cargo clippy -p merman-cli --features ascii --all-targets -- -D warnings`;
  `git diff --check`
- Status: DONE
- Review: closeout review found no blocking findings. Deferred behavior is listed as follow-on
  candidates instead of hidden work in this lane.
- Evidence: `EVIDENCE_AND_GATES.md`

## Decisions Since Last Update

- `beautiful-mermaid` is a reference implementation, not a spec.
- New ASCII diagram renderers must consume `merman-core` typed models.
- Do not port or duplicate `beautiful-mermaid`'s parser or SVG renderer into `merman-ascii`.
- Class, ER, and xychart are separate vertical slices.
- The ARI-020 class slice supports class boxes, members, methods, ASCII/Unicode borders, and one
  solid extension relationship.
- ARI-030 expands the single-relationship layout to extension labels, reverse extension
  orientation, aggregation, composition, dependency dotted arrows, and Unicode composition markers.
- Multiple relationships, relation layouts with unrelated classes, association/no-marker
  relationships, lollipop markers, namespaces, notes, and styling remain follow-on work with
  explicit diagnostics where the current slice encounters them.
- ARI-040 adds ER entity boxes, attributes, relationship labels, identifying/non-identifying line
  style, and common cardinality markers from typed `ErRelSpecRenderModel`.
- Multiple ER relationships, ER relationship layouts with unrelated entities, entity styling/classes,
  and richer ER graph placement remain follow-on work with explicit diagnostics where the current
  slice encounters them.
- ARI-050 adds a terminal-native XYChart scaling contract: fixed five-row vertical plots,
  three-character category bands, rounded bar heights, stair-step line overlays, ten-character
  horizontal value bars, inferred numeric x labels, and per-line trimming for stable snapshots.
- XYChart legends, ANSI/color output, multi-series spacing, and full-size terminal plot layout remain
  follow-on work.
- ARI-060 ports thick flowchart edges because the typed `FlowEdge.stroke` field preserves the
  semantics and the existing route drawing can swap glyphs without a layout redesign.
- ARI-060 rejects the current `beautiful-mermaid` `RL` approximation because treating `RL` as `LR`
  would misrepresent Mermaid direction. True `BT`/`RL`, subgraph direction overrides, multiline
  subgraph labels, color/style roles, state graph rendering, and uncommon shapes are deferred in the
  gap matrix.
- ARI-070 keeps the public boundary model-driven: new top-level API exposure is a re-export of
  existing `merman-ascii` typed helpers, and CLI behavior still routes Mermaid source through
  `merman-core` typed render models before text rendering.
- The shipped terminal-text support matrix is now flowchart/graph, sequenceDiagram, classDiagram,
  erDiagram, and xychart. Other diagram families should continue to fail explicitly with
  unsupported-diagram diagnostics until a typed renderer exists.

## Blockers

- None.

## Follow-Ons

This lane is closed. Split future work into narrower lanes when product priority is clear:

- Class/ER multi-relationship graph layout and placement for unrelated classes/entities.
- True BT/RL graph transforms with arrow/corner remapping.
- Subgraph direction overrides and multiline subgraph labels.
- ANSI/HTML color and class/style role rendering for terminal output.
- State diagram graph text rendering if typed state models preserve enough semantics.
- Additional uncommon flowchart shape approximations.
- Richer XYChart legends, color, multi-series spacing, and full-size terminal plotting.

Reference-source obligations remain unchanged: do not port parsers from `repo-ref/`, keep derived
source/fixtures attributed with tracked MIT notices, and treat Mermaid upstream as the spec.
