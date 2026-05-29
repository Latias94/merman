# ASCII Reference Implementation Expansion — Handoff

Status: Active
Last updated: 2026-05-29

## Current State

This lane governs how `merman-ascii` should learn from `repo-ref/mermaid-ascii` and
`repo-ref/beautiful-mermaid` while preserving the model-driven boundary. Reference provenance is
tracked. classDiagram ASCII now renders boxes plus an expanded single-relationship subset from
`RenderSemanticModel::Class`, and ER ASCII now renders entity boxes plus a first single-relationship
subset from `RenderSemanticModel::Er`.

## Active Task

- Task ID: ARI-040
- Owner: codex
- Files:
  - `crates/merman-ascii/src/lib.rs`
  - `crates/merman-ascii/src/er/*`
  - `crates/merman-ascii/tests/er_model.rs`
  - `crates/merman-ascii/README.md`
  - `docs/workstreams/ascii-reference-implementation-expansion/*`
- Validation: `cargo nextest run -p merman-ascii --test er_model`;
  `cargo nextest run -p merman-ascii er`; `cargo nextest run -p merman-ascii`;
  `cargo fmt --all --check`; `cargo clippy -p merman-ascii --all-targets -- -D warnings`
- Status: DONE
- Review: self-review found no blocking findings; broader planner review can still inspect the
  remaining multi-relationship ER layout scope before closing M2.
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

## Blockers

- None for ARI-040.

## Next Recommended Action

Continue with ARI-050 for xychart ASCII. A separate follow-on should handle multi-relationship
graph layout shared by class and ER if that becomes the next priority.
