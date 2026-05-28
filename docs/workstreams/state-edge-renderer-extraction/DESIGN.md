# State Edge Renderer Extraction

Status: Complete
Last updated: 2026-05-28

## Why This Lane Exists

`crates/merman-render/src/svg/parity/state/render.rs` is the largest SVG parity renderer file and
mixes root orchestration, cluster emission, edge geometry, edge labels, and node shape rendering.
The edge path and label logic is cohesive enough to extract first without changing public APIs or
diagram behavior.

## Relevant Authority

- Existing docs:
  - `docs/rendering/REFACTOR_TODO.md`
  - `docs/rendering/FEARLESS_REFACTORING_SVG_PARITY.md`
- Related workstreams:
  - `docs/workstreams/svg-parity-helper-consolidation/`
  - `docs/workstreams/svg-debug-pointlist-consolidation/`

## Problem

State edge rendering has high local complexity: cluster boundary clipping, cyclic self-loop helper
edges, curve-basis path encoding, `data-points` serialization, and label repositioning all live
inside the same root render module. This makes future state renderer work harder to review and
raises the risk of accidental output drift.

## Target State

State root orchestration stays in `state/render.rs`; edge path and label rendering move to
`state/edge.rs` behind a narrow module boundary. Emitted SVG remains unchanged.

## In Scope

- Extract state edge helper types and functions to `crates/merman-render/src/svg/parity/state/edge.rs`.
- Wire `state/mod.rs` and `state/render.rs` to call the extracted module.
- Preserve current behavior for cluster clipping, self-loop helper edges, edge labels, and
  `data-points`.
- Record fresh validation evidence.

## Out Of Scope

- Splitting state node shape rendering.
- Reworking root viewport derivation.
- Changing state layout, parser behavior, or generated overrides.
- Performance benchmarking.

## Starting Assumptions

| Assumption | Confidence | Evidence | Consequence if wrong |
| --- | --- | --- | --- |
| Edge logic is cohesive enough to extract without new public APIs. | High | `render.rs` edge helpers are grouped between root/cluster and node rendering. | Extraction may need a smaller boundary or helper context type. |
| Existing state tests and compare-state SVG gate cover the risky output paths. | Medium | State layout/SVG tests and `xtask compare-state-svgs` already exist. | Add focused regression coverage before closeout. |
| This is a behavior-preserving move, not a semantic rewrite. | High | No data model or layout contract changes are planned. | Any SVG diff is a rollback signal. |

## Architecture Direction

Keep `StateRenderCtx` as the shared render context for now. `state/render.rs` owns traversal and
DOM insertion order; `state/edge.rs` owns edge-local geometry and markup. Later lanes can extract
state node shape rendering and root orchestration using the same pattern.

## Closeout Condition

This lane is complete. State edge rendering now compiles from `state/edge.rs`, validation gates
passed, and follow-on state node/root splits are deferred to later bounded lanes.
