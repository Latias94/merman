# ASCII Class ER Graph Layout - Handoff

Status: Active
Last updated: 2026-05-30

## Current State

This lane is a follow-on from the closed
`docs/workstreams/ascii-reference-implementation-expansion/` lane. It exists because class and ER
ASCII rendering are now useful but still intentionally reject multi-relationship layouts.

Current class support renders boxes, members, methods, labels, and single-relationship layouts for
extension, dependency, aggregation, and composition. Current ER support renders entity boxes,
attributes, labels, identifying/non-identifying relationships, and common cardinality markers.

## Active Task

- Task ID: ACEG-020
- Owner: codex
- Files:
  - `crates/merman-ascii/tests/class_model.rs`
  - `crates/merman-ascii/tests/er_model.rs`
  - `docs/workstreams/ascii-class-er-graph-layout/*`
- Validation: `cargo nextest run -p merman-ascii class`;
  `cargo nextest run -p merman-ascii er`
- Status: DONE
- Review: Public parser-backed diagnostics are now locked; no layout internals were added.
- Evidence: `EVIDENCE_AND_GATES.md`

## Next Recommended Action

Run ACEG-030:

- Introduce the smallest shared terminal relationship-graph placement boundary.
- Route existing single-relationship class/ER outputs through it without broad snapshot drift.
- Keep class and ER relationship semantics in their own adapters.

## Constraints

- Do not port a Mermaid parser from any reference implementation.
- Do not reuse SVG layout or browser measurement as the ASCII source of truth.
- Do not silently omit relationships.
- Keep relationship semantics in class/ER adapters; keep any shared module terminal-layout-only.
- Stage only files for the active task.
