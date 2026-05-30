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

- Task ID: ACEG-030
- Owner: codex
- Files:
  - `crates/merman-ascii/src/relation_graph.rs`
  - `crates/merman-ascii/src/class/render.rs`
  - `crates/merman-ascii/src/er/render.rs`
  - `docs/workstreams/ascii-class-er-graph-layout/*`
- Validation: `cargo nextest run -p merman-ascii class`;
  `cargo nextest run -p merman-ascii er`; `cargo fmt --all --check`; `git diff --check`
- Status: DONE
- Review: Shared placement code is terminal-layout-only. Class and ER adapters still own marker,
  cardinality, label, and unsupported-feature semantics.
- Evidence: `EVIDENCE_AND_GATES.md`

## Next Recommended Action

Run ACEG-040:

- Add class multi-relationship rendering on top of `relation_graph`.
- Start with low-risk topologies such as chains and stars before dense/crossing layouts.
- Keep unsupported diagnostics explicit when every relation cannot be rendered honestly.

## Constraints

- Do not port a Mermaid parser from any reference implementation.
- Do not reuse SVG layout or browser measurement as the ASCII source of truth.
- Do not silently omit relationships.
- Keep relationship semantics in class/ER adapters; keep any shared module terminal-layout-only.
- Stage only files for the active task.
