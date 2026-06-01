# ASCII Architecture Deepening — Milestones

Status: Active
Last updated: 2026-06-02

## M0 — Scope And Evidence Freeze

Exit criteria:

- The five deepening targets are named.
- Relevant ADRs and support docs are linked.
- Task dependencies and validation gates are explicit.

Primary evidence:

- `docs/workstreams/ascii-architecture-deepening/DESIGN.md`
- `docs/workstreams/ascii-architecture-deepening/TODO.md`

## M1 — Shared Styled Text/Cell Module

Exit criteria:

- A shared internal styled text/cell module exists.
- At least two existing family line buffers use the shared implementation.
- Plain output remains stable unless a test documents an intentional correction.

Primary gates:

- `cargo nextest run -p merman-ascii canvas color`
- targeted family tests for migrated callers

## M2 — Graph Route Planning And Painting Seam

Exit criteria:

- At least one route family is planned before painting.
- Route planning has focused tests that do not require full diagram snapshots.
- Edge labels and foreground style behavior remain covered.

Primary gates:

- `cargo nextest run -p merman-ascii flowchart`

## M3 — Relation Graph Adapter Deepening

Exit criteria:

- Class and ER relation rendering share a deeper relation graph adapter surface.
- Duplicated relation drawing code is deleted or explicitly justified.
- Class and ER color-role behavior remains covered.

Primary gates:

- `cargo nextest run -p merman-ascii class er`

## M4 — Sequence Event Plan Seam

Exit criteria:

- Sequence event-state planning is separable from row painting.
- Lifecycle, activation, visibility, and control frame tests still pass.
- Follow-on features are split instead of widening the refactor.

Primary gates:

- `cargo nextest run -p merman-ascii sequence`

## M5 — ASCII Gap Registry

Exit criteria:

- A single registry lists remaining ASCII gaps, owners, dependencies, and gates.
- The registry is linked from the ASCII README.
- Existing support docs remain the source of shipped/unsupported behavior.

Primary gates:

- `git diff --check -- crates/merman-ascii docs/workstreams/ascii-architecture-deepening`

## M6 — Final Verification And Closeout

Exit criteria:

- `cargo nextest run -p merman-ascii` passes.
- `cargo fmt --all --check` passes.
- `cargo clippy -p merman-ascii --all-targets -- -D warnings` passes.
- `git diff --check` passes.
- `WORKSTREAM.json`, `TODO.md`, `EVIDENCE_AND_GATES.md`, and `HANDOFF.md` reflect final state.

Closeout result:

- Completed on 2026-05-30.
- All five architecture targets landed.
- Final package, format, clippy, and whitespace gates passed.

## M7 — Local Subgraph Direction Subset

Exit criteria:

- A bounded `FlowSubgraph.dir` subset is shipped for canonical `LR` subgraphs inside canonical `TD`
  roots.
- Internal subgraph edges adopt the local direction.
- Support docs and the gap registry describe the shipped subset and remaining work precisely.

Primary gates:

- `cargo nextest run -p merman-ascii flowchart subgraph`
- `cargo nextest run -p merman-ascii graph_fixture`

Closeout result:

- Completed on 2026-06-01.
- Landed as commit `3dbd5a3b`.

## M8 — Cross-Boundary Mixed-Direction Routing Seam

Exit criteria:

- Cross-boundary edges are classified explicitly by routing context instead of relying on layout
  fallback.
- At least one shipped boundary-routing slice exists for the reopened `A-GRAPH-010` family.
- Route planning remains testable before painting, with dedicated tests for entering/leaving a
  direction-bearing subgraph.

Primary gates:

- targeted route-plan tests for boundary classification and segment planning
- `cargo nextest run -p merman-ascii flowchart subgraph`
- `cargo nextest run -p merman-ascii flowchart`
