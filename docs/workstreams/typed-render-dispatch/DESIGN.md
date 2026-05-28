# Typed Render Dispatch

Status: Complete
Last updated: 2026-05-28

## Why This Lane Exists

The typed render pipeline has one typed model per supported diagram, but diagram aliases and model
kind metadata are still repeated across parse, layout, and render stages. This makes every diagram
addition or alias change touch multiple hand-maintained matches.

This lane reduces the parallel dispatch tables without changing the public rendering behavior.

## Relevant Authority

- Prior architecture review: local report `merman-architecture-review-20260528-101115.html`
- Related workstream:
  - `docs/workstreams/architecture-indexed-fcose/`
- Source boundaries:
  - `crates/merman-core/src/diagram/mod.rs`
  - `crates/merman-core/src/lib.rs`
  - `crates/merman-render/src/lib.rs`
  - `crates/merman-render/src/svg/parity.rs`

## Problem

Aliases such as `flowchart-v2` / `flowchart` / `flowchart-elk`, `sequence` / `zenuml`, and
`er` / `erDiagram` are encoded separately in core parsing and renderer layout dispatch. Model kind
strings are also owned by `Engine` instead of the typed model enum.

This is not a runtime hotspot; it is maintenance risk and accidental coordination cost.

## Target State

- `RenderSemanticModel` owns model kind and diagram-type compatibility knowledge.
- Renderer layout dispatch can rely on the typed model variant instead of repeating alias patterns.
- JSON fallback behavior remains unchanged.
- Broader generated dispatch consolidation remains a follow-on unless a safe single-source table is
  proven in this lane.

## In Scope

- Add `RenderSemanticModel` metadata methods for model kind and diagram-type compatibility.
- Remove duplicate alias checks from typed layout dispatch.
- Replace `Engine::render_model_kind` with the model-owned method.
- Add focused tests for alias compatibility.

## Out Of Scope

- Rewriting the parser registry.
- Changing `RenderSemanticModel` enum variants.
- Replacing the entire render SVG match in one step.
- Changing public `merman` render APIs.
- Changing JSON compatibility paths.

## Starting Assumptions

| Assumption | Confidence | Evidence | Consequence if wrong |
| --- | --- | --- | --- |
| Typed parser already guarantees variant/type consistency for normal callers. | High | `Engine::parse_render_semantic_model` constructs the enum variant based on `meta.diagram_type`. | Keep an explicit compatibility check before variant-only layout dispatch. |
| Alias drift is the first safe duplication to remove. | High | Alias patterns repeat in core parse and render layout. | If tests reveal callers can build inconsistent `ParsedDiagramRender`, keep validation and fail fast. |
| A fully generated dispatch table is riskier than a first metadata extraction. | Medium | Layout/render function signatures differ by diagram. | Split generated dispatch into a follow-on task only after metadata extraction is green. |

## Architecture Direction

The model enum becomes the owner of type metadata:

1. `RenderSemanticModel::kind()` returns the canonical model kind.
2. `RenderSemanticModel::supports_diagram_type(diagram_type)` owns alias compatibility.
3. Renderer layout dispatch validates compatibility once, then matches on the enum variant only.

This keeps type ownership in `merman-core` while avoiding a cross-crate trait-object registry.

## Closeout Condition

This lane can close when:

- duplicate alias checks are removed from typed layout dispatch,
- focused core/render tests pass,
- package gates pass,
- and any deeper generated dispatch work is either completed or split as a follow-on.
