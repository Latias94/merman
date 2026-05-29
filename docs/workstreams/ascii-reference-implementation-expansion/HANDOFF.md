# ASCII Reference Implementation Expansion — Handoff

Status: Active
Last updated: 2026-05-29

## Current State

This lane governs how `merman-ascii` should learn from `repo-ref/mermaid-ascii` and
`repo-ref/beautiful-mermaid` while preserving the model-driven boundary. Reference provenance is
tracked, and the first classDiagram ASCII slice now renders from `RenderSemanticModel::Class`.

## Active Task

- Task ID: ARI-020
- Owner: codex
- Files:
  - `crates/merman-ascii/src/lib.rs`
  - `crates/merman-ascii/src/class/*`
  - `crates/merman-ascii/tests/class_model.rs`
  - `crates/merman-ascii/README.md`
  - `docs/workstreams/ascii-reference-implementation-expansion/*`
- Validation: `cargo nextest run -p merman-ascii class`; `cargo nextest run -p merman-ascii`;
  `cargo fmt --all --check`; `cargo clippy -p merman-ascii --all-targets -- -D warnings`
- Status: DONE
- Review: self-review found no blocking findings; broader planner review can still inspect the
  follow-on relationship expansion.
- Evidence: `EVIDENCE_AND_GATES.md`

## Decisions Since Last Update

- `beautiful-mermaid` is a reference implementation, not a spec.
- New ASCII diagram renderers must consume `merman-core` typed models.
- Do not port or duplicate `beautiful-mermaid`'s parser or SVG renderer into `merman-ascii`.
- Class, ER, and xychart are separate vertical slices.
- The ARI-020 class slice supports class boxes, members, methods, ASCII/Unicode borders, and one
  solid extension relationship.
- Class relationship labels, non-extension relationship kinds, non-solid lines, multiple
  relationships, relation layouts with unrelated classes, namespaces, notes, and styling remain
  follow-on work with explicit diagnostics where the first slice encounters them.

## Blockers

- None for ARI-020.

## Next Recommended Action

Continue with ARI-030 if the priority is class relationship parity, especially dependency,
aggregation, composition, relationship labels, and orientation. ARI-040 and ARI-050 can also start
independently because ER and xychart consume separate typed render models.
