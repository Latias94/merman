# ASCII Class ER Graph Layout - Handoff

Status: Closed
Last updated: 2026-05-30

## Current State

This lane was a follow-on from the closed
`docs/workstreams/ascii-reference-implementation-expansion/` lane. It exists because class and ER
ASCII rendering had useful first slices but still intentionally rejected multi-relationship layouts.

Current class support renders boxes, members, methods, labels, single-relationship layouts, and
layered extension chains/stars for extension, dependency, aggregation, and composition. Current ER
support renders entity boxes, attributes, labels, identifying/non-identifying relationships, common
cardinality markers, and layered relationship chains/stars. Crossing, cyclic, parallel, and
unrelated graph shapes remain explicit diagnostics for both families.

ACEG-060 verified the public package, library, and CLI gates and updated public support docs. The
lane is closed.

## Final Task

- Task ID: ACEG-060
- Owner: codex
- Files:
  - `README.md`
  - `crates/merman-cli/README.md`
  - `crates/merman-ascii/README.md`
  - `crates/merman-render/src/math.rs`
  - `docs/workstreams/ascii-class-er-graph-layout/*`
- Validation: `cargo nextest run -p merman-ascii`; `cargo nextest run -p merman --features ascii`;
  `cargo nextest run -p merman-cli --features ascii`; `cargo clippy -p merman-ascii --all-targets -- -D warnings`;
  `cargo clippy -p merman --features ascii --all-targets -- -D warnings`;
  `cargo clippy -p merman-cli --features ascii --all-targets -- -D warnings`;
  `cargo clippy -p merman-render --all-targets -- -D warnings`; `cargo fmt --all --check`;
  `git diff --check`
- Status: DONE
- Review: Broad gates pass. Layered class/ER planner duplication is acceptable for this lane because
  shared code remains terminal-layout-only and typed relationship semantics stay in adapters.
- Evidence: `EVIDENCE_AND_GATES.md`

## Follow-Ons

- Extract a shared terminal-layout-only layered planner from the class and ER adapters if the next
  class/ER topology slice needs the same level assignment, ordering, and crossing detection.
- Add a separate dense/crossing topology routing lane for classDiagram and erDiagram. It should keep
  explicit diagnostics until every relationship can be shown honestly.
- Keep color/style, state ASCII, true BT/RL flowchart directions, and richer XYChart layout in their
  existing or future dedicated lanes.

## Constraints

- Do not port a Mermaid parser from any reference implementation.
- Do not reuse SVG layout or browser measurement as the ASCII source of truth.
- Do not silently omit relationships.
- Keep relationship semantics in class/ER adapters; keep any shared module terminal-layout-only.
- Stage only files for the active task.
