# ASCII Class ER Graph Layout - Handoff

Status: Active
Last updated: 2026-05-30

## Current State

This lane is a follow-on from the closed
`docs/workstreams/ascii-reference-implementation-expansion/` lane. It exists because class and ER
ASCII rendering are now useful but still intentionally reject multi-relationship layouts.

Current class support renders boxes, members, methods, labels, single-relationship layouts, and
layered extension chains/stars for extension, dependency, aggregation, and composition. Current ER
support renders entity boxes, attributes, labels, identifying/non-identifying relationships, common
cardinality markers, and layered relationship chains/stars. Crossing, cyclic, parallel, and
unrelated graph shapes remain explicit diagnostics for both families.

## Active Task

- Task ID: ACEG-050
- Owner: codex
- Files:
  - `crates/merman-ascii/src/er/render.rs`
  - `crates/merman-ascii/tests/er_model.rs`
  - `crates/merman-ascii/README.md`
  - `docs/workstreams/ascii-class-er-graph-layout/*`
- Validation: `cargo nextest run -p merman-ascii er`;
  `cargo clippy -p merman-ascii --all-targets -- -D warnings`; `cargo fmt --all --check`;
  `git diff --check`
- Status: DONE
- Review: ER multi-relationship rendering is limited to layered DAG shapes where every supported
  relationship can be shown while preserving typed cardinality, line style, and labels.
- Evidence: `EVIDENCE_AND_GATES.md`

## Next Recommended Action

Run ACEG-060:

- Run broad public gates for `merman-ascii`, `merman --features ascii`, and
  `merman-cli --features ascii`.
- Review whether class/ER layered placement duplication should be split into a follow-on refactor
  or closed as acceptable for this lane.
- Update public support docs and close or split remaining dense-topology gaps.

## Constraints

- Do not port a Mermaid parser from any reference implementation.
- Do not reuse SVG layout or browser measurement as the ASCII source of truth.
- Do not silently omit relationships.
- Keep relationship semantics in class/ER adapters; keep any shared module terminal-layout-only.
- Stage only files for the active task.
