# ASCII Class ER Graph Layout

Status: Active
Last updated: 2026-05-30

## Why This Lane Exists

`merman-ascii` now renders useful first slices for `classDiagram` and `erDiagram`, but both
renderers still encode a single vertical relationship path. They intentionally reject multiple
relationships and unrelated endpoint layouts because silently dropping edges would misrepresent the
diagram.

This lane turns that explicit limitation into a small graph-layout refactor for relationship-heavy
class and ER diagrams while preserving the ASCII boundary: Mermaid parsing stays in `merman-core`,
and terminal layout stays independent from SVG layout.

## Relevant Authority

- ADRs:
  - `docs/adr/0014-upstream-parity-policy.md`
  - `docs/adr/0065-ascii-output-boundary.md`
- Parent lane:
  - `docs/workstreams/ascii-reference-implementation-expansion/`
- Runtime/docs:
  - `crates/merman-ascii/README.md`
  - `crates/merman-ascii/src/class/render.rs`
  - `crates/merman-ascii/src/er/render.rs`
  - `crates/merman-ascii/tests/class_model.rs`
  - `crates/merman-ascii/tests/er_model.rs`

## Problem

The current class and ER renderers are readable for zero or one relationship, but their layout
model is too narrow for common diagrams:

- class diagrams reject multiple relations and relationship layouts with unrelated classes,
- ER diagrams reject multiple relationships and unrelated entity layouts,
- relationship routing is embedded directly in the diagram renderer,
- future class/ER graph improvements would duplicate placement, spacing, and edge-label logic.

The right long-term shape is not a second flowchart renderer. Class and ER need a smaller
relationship-graph seam that knows about terminal boxes and typed relationship metadata, not Mermaid
syntax or SVG geometry.

## Target State

- Class and ER renderers share a bounded internal relationship graph layout seam for box placement,
  component ordering, vertical/horizontal spacing, and simple edge lanes.
- Existing single-relationship outputs remain stable unless a documented snapshot update improves
  the broader graph contract.
- Class diagrams render useful multi-relationship subsets: chains, stars, and connected components
  with extension, dependency, aggregation, and composition markers.
- ER diagrams render useful multi-relationship subsets with cardinality markers, labels, and
  identifying/non-identifying line style.
- Unsupported graph shapes remain explicit diagnostics when the text output would be misleading.
- Public entry points remain unchanged: `render_class`, `render_er`, `render_model`,
  `merman::ascii::render_ascii_sync`, and CLI `--format ascii|unicode`.

## In Scope

- Internal layout module(s) under `crates/merman-ascii/src/` for relationship graph placement.
- Refactoring class/ER renderers to consume that seam instead of hard-coding only one vertical
  relationship.
- Snapshot tests for multi-relationship class and ER examples through the parser and typed model.
- Documentation updates for shipped subsets and remaining explicit diagnostics.

## Out Of Scope

- A second Mermaid parser.
- SVG layout reuse or pixel-to-character quantization.
- ANSI/HTML color output or class/style role rendering.
- State diagram ASCII rendering.
- Full graph-theory layout parity for every possible class/ER topology.
- Byte-for-byte compatibility with `beautiful-mermaid`.

## Starting Assumptions

| Assumption | Confidence | Evidence | Consequence if wrong |
| --- | --- | --- | --- |
| Class and ER can share terminal box placement without sharing relationship semantics. | Medium | Both render node/entity boxes plus typed relationships, but markers/cardinality differ. | Keep only placement shared and leave relation drawing diagram-specific. |
| Small deterministic layouts are better than a broad graph engine for this lane. | High | Current product goal is stable terminal output, not force-directed graph drawing. | Split harder topologies into follow-ons rather than generalizing prematurely. |
| Existing single-relationship snapshots are compatibility anchors. | High | They are user-visible ASCII output and covered by tests. | If changed, document why the new graph contract is better. |

## Architecture Direction

Introduce a small relationship-graph boundary with two responsibilities:

1. Place rendered boxes into deterministic rows/columns.
2. Provide edge lanes between placed boxes for diagram-specific drawing.

Keep diagram semantics at the edge adapters:

```text
ClassDiagram / ErDiagramRenderModel
  -> diagram-specific box render + typed relationship adapter
  -> shared terminal relationship graph placement
  -> diagram-specific marker/cardinality/label drawing
  -> stable ASCII/Unicode text
```

Do not make the shared seam own Mermaid concepts such as `RelationShape`, ER cardinality strings, or
parser syntax. Those remain in class/ER adapters.

## Refactor Brief

- Intent: remove the single-relation layout ceiling without adding a second parser or copying SVG
  layout.
- Scope: `crates/merman-ascii/src/class/`, `crates/merman-ascii/src/er/`, tests, and support docs.
- Deletion plan: retire or narrow one-off single-relationship guards once a shared layout can render
  the same cases honestly.
- Boundary plan: shared placement/routing primitives are terminal-layout concepts; class/ER modules
  keep marker, cardinality, label, and diagnostic ownership.
- Testing plan: start with parser-backed tests for currently unsupported multiple relationships,
  then keep snapshots stable through package and public feature gates.
- Risk plan: avoid silent edge loss; prefer `AsciiError::UnsupportedFeature` for crossing or dense
  topologies until they have tests.
- Workflow plan: durable workstream with vertical tasks; set Codex goals only for bounded TODO
  tasks, not for the whole lane.

## Closeout Condition

This lane can close when:

- class and ER render at least one useful multi-relationship topology each,
- the shared layout boundary is either justified by both renderers or removed,
- unsupported topologies have structured diagnostics,
- docs describe the shipped subset,
- and `merman-ascii`, `merman --features ascii`, and `merman-cli --features ascii` gates pass.
