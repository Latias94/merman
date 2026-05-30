# ASCII Class ER Graph Layout - Handoff

Status: Active
Last updated: 2026-05-30

## Current State

This lane is a follow-on from the closed
`docs/workstreams/ascii-reference-implementation-expansion/` lane. It exists because class and ER
ASCII rendering are now useful but still intentionally reject multi-relationship layouts.

Current class support renders boxes, members, methods, labels, single-relationship layouts, and
layered extension chains/stars for extension, dependency, aggregation, and composition. Crossing,
cyclic, parallel, and unrelated-class graph shapes remain explicit diagnostics. Current ER support
renders entity boxes, attributes, labels, identifying/non-identifying relationships, and common
cardinality markers.

## Active Task

- Task ID: ACEG-040
- Owner: codex
- Files:
  - `crates/merman-ascii/src/relation_graph.rs`
  - `crates/merman-ascii/src/class/render.rs`
  - `crates/merman-ascii/tests/class_model.rs`
  - `crates/merman-ascii/README.md`
  - `docs/workstreams/ascii-class-er-graph-layout/*`
- Validation: `cargo nextest run -p merman-ascii class`;
  `cargo clippy -p merman-ascii --all-targets -- -D warnings`; `cargo fmt --all --check`;
  `git diff --check`
- Status: DONE
- Review: Class multi-relationship rendering is limited to layered DAG shapes where every supported
  relation can be shown; crossing/cyclic/parallel/unrelated graph shapes stay explicit diagnostics.
- Evidence: `EVIDENCE_AND_GATES.md`

## Next Recommended Action

Run ACEG-050:

- Reuse the shared terminal placement seam for ER multi-relationship rendering.
- Start with low-risk ER chain/star topologies that preserve cardinality and identifying line style.
- Keep unsupported diagnostics explicit when every relationship cannot be rendered honestly.

## Constraints

- Do not port a Mermaid parser from any reference implementation.
- Do not reuse SVG layout or browser measurement as the ASCII source of truth.
- Do not silently omit relationships.
- Keep relationship semantics in class/ER adapters; keep any shared module terminal-layout-only.
- Stage only files for the active task.
