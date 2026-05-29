# ASCII Reference Implementation Expansion — Handoff

Status: Active
Last updated: 2026-05-29

## Current State

This lane governs how `merman-ascii` should learn from `repo-ref/mermaid-ascii` and
`repo-ref/beautiful-mermaid` while preserving the model-driven boundary. Reference provenance is
tracked. classDiagram ASCII now renders boxes plus an expanded single-relationship subset from
`RenderSemanticModel::Class`, and ER ASCII now renders entity boxes plus a first single-relationship
subset from `RenderSemanticModel::Er`. XYChart ASCII now renders compact bars, lines, mixed plots,
horizontal bars, and Unicode/ASCII chart characters from `RenderSemanticModel::XyChart`. Flowchart
delta triage against `beautiful-mermaid` is recorded, and thick edges now render from the typed
flowchart stroke model.

## Active Task

- Task ID: ARI-060
- Owner: codex
- Files:
  - `crates/merman-ascii/src/graph/*`
  - `crates/merman-ascii/tests/flowchart_model.rs`
  - `crates/merman-ascii/FLOWCHART_SUPPORT.md`
  - `crates/merman-ascii/README.md`
  - `docs/workstreams/ascii-reference-implementation-expansion/*`
- Validation: `cargo nextest run -p merman-ascii flowchart`;
  `cargo nextest run -p merman-ascii`; `cargo fmt --all --check`;
  `cargo clippy -p merman-ascii --all-targets -- -D warnings`; `git diff --check`
- Status: DONE
- Review: self-review found no blocking findings; broader planner review can still inspect whether
  the deferred `beautiful-mermaid` graph deltas should become separate workstreams.
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

## Blockers

- None for ARI-060.

## Next Recommended Action

Continue with ARI-070 for public API/docs integration and broader ASCII feature gates. Split true
BT/RL graph direction transforms or color/style roles into follow-on lanes if they become the next
priority.
