# ASCII Architecture Deepening — TODO

Status: Active
Last updated: 2026-06-26

## M0 — Scope And Evidence Freeze

- [x] AAD-010 [owner=planner] [deps=none] [scope=docs/workstreams/ascii-architecture-deepening]
  Goal: Freeze the five deepening targets, task order, and verification gates.
  Validation: `git diff --check -- docs/workstreams/ascii-architecture-deepening`
  Review: Planner self-review before implementation starts.
  Evidence: `docs/workstreams/ascii-architecture-deepening/DESIGN.md`
  Context: `docs/workstreams/ascii-architecture-deepening/CONTEXT.jsonl`
  Handoff: DONE on 2026-05-30. Workstream docs agree and document gate passed.

## M1 — Shared Styled Text/Cell Module

- [x] AAD-020 [owner=codex] [deps=AAD-010] [scope=crates/merman-ascii/src/text.rs,crates/merman-ascii/src/sequence/text.rs,crates/merman-ascii/src/xychart/render.rs]
  Goal: Introduce one internal styled text/cell module and migrate at least two existing line
  buffers to it without changing rendered plain output.
  Validation: `cargo nextest run -p merman-ascii canvas color`
  Review: Ensure the new module has depth and is not a pass-through wrapper.
  Evidence: focused tests for trim, padding, role preservation, and ANSI/HTML finalization.
  Context: `docs/adr/0067-ascii-color-role-api.md`, this workstream context manifest.
  Handoff: DONE on 2026-05-30. `StyledCell` and `StyledLine` now back sequence and XYChart line
  buffers. Relation graph line migration remains available for AAD-040.

## M2 — Graph Route Planning And Painting Seam

- [x] AAD-030 [owner=codex] [deps=AAD-020] [scope=crates/merman-ascii/src/graph/routing.rs,crates/merman-ascii/src/graph/routing/plan.rs]
  Goal: Split a testable graph route-planning seam from canvas painting for at least one important
  route family, then extend if the seam proves useful.
  Validation: `cargo nextest run -p merman-ascii flowchart`
  Review: Verify route tests exercise planning without requiring whole-diagram snapshots.
  Evidence: graph routing tests and unchanged or intentionally updated flowchart snapshots.
  Context: `docs/adr/0065-ascii-output-boundary.md`, `crates/merman-ascii/FLOWCHART_SUPPORT.md`.
  Handoff: DONE on 2026-05-30. Top-down direct routes now plan connector, route cells, arrow, and
  label anchors before painting. Other route families remain on the old drawing path.

## M3 — Relation Graph Adapter Deepening

- [x] AAD-040 [owner=codex] [deps=AAD-020] [scope=crates/merman-ascii/src/relation_graph.rs,crates/merman-ascii/src/class/render.rs,crates/merman-ascii/src/er/render.rs,crates/merman-ascii/src/text.rs]
  Goal: Deepen the relation graph module so class and ER provide family vocabulary while shared
  placement and painting behavior moves behind the relation graph seam.
  Validation: `cargo nextest run -p merman-ascii class er`
  Review: Confirm duplicated class/ER rendering code is deleted or justified.
  Evidence: class and ER regression tests covering relation routing and color roles.
  Context: this workstream context manifest plus class/ER support tests.
  Handoff: DONE on 2026-05-30. `RelationGraphLine` now uses the shared styled substrate; relation
  graph owns box row construction, relation line merging, and centered relation text writing.
  Class/ER still own diagram-specific relation semantics and charset selection.

## M4 — Sequence Event Plan Seam

- [x] AAD-050 [owner=codex] [deps=AAD-020] [scope=crates/merman-ascii/src/sequence.rs,crates/merman-ascii/src/sequence/plan.rs,crates/merman-ascii/src/sequence/render.rs]
  Goal: Separate sequence event-state planning from row painting for lifecycle, activation,
  visibility, and control-frame state.
  Validation: `cargo nextest run -p merman-ascii sequence`
  Review: Ensure the planner interface is behavior-bearing and not only a relocated render loop.
  Evidence: sequence tests for lifecycle/control/activation behavior.
  Context: `crates/merman-ascii/SEQUENCE_SUPPORT.md`, sequence parity workstream docs.
  Handoff: DONE on 2026-05-30. `SequenceEventPlan` now owns activation counts, actor visibility,
  control frame state, lifecycle visibility updates, and control ordering errors. Row painting stays
  in the existing render path.

## M5 — ASCII Gap Registry

- [x] AAD-060 [owner=codex] [deps=AAD-030,AAD-040,AAD-050] [scope=crates/merman-ascii/ASCII_GAP_REGISTRY.md,crates/merman-ascii/README.md,docs/workstreams/ascii-architecture-deepening]
  Goal: Create a single ASCII gap registry that maps remaining feature gaps to owning modules,
  dependencies, and validation gates.
  Validation: `git diff --check -- crates/merman-ascii docs/workstreams/ascii-architecture-deepening`
  Review: Registry entries should be actionable and not duplicate support matrices verbatim.
  Evidence: new or updated registry document linked from `crates/merman-ascii/README.md`.
  Context: flowchart, sequence, graph fixture gap, and sequence fixture gap docs.
  Handoff: DONE on 2026-05-30. `ASCII_GAP_REGISTRY.md` maps follow-on gaps to modules,
  dependencies, gates, and support-doc sources; README links to it.

## M6 — Final Verification And Closeout

- [x] AAD-070 [owner=planner] [deps=AAD-060] [scope=docs/workstreams/ascii-architecture-deepening]
  Goal: Run final gates, record evidence, close or split any unfinished target, and update handoff.
  Validation: `cargo nextest run -p merman-ascii`; `cargo fmt --all --check`;
  `cargo clippy -p merman-ascii --all-targets -- -D warnings`; `git diff --check`
  Review: Run workstream review before closing.
  Evidence: `EVIDENCE_AND_GATES.md`, `WORKSTREAM.json`, closeout journal note.
  Context: this workstream context manifest.
  Handoff: DONE on 2026-05-30. Final gates passed; no blocking review findings remain. Follow-on
  feature gaps are tracked in `crates/merman-ascii/ASCII_GAP_REGISTRY.md`.

## M7 — Subgraph Local Direction Subset

- [x] AAD-080 [owner=codex] [deps=AAD-070] [scope=crates/merman-ascii/src/graph,crates/merman-ascii/tests/flowchart_model.rs,crates/merman-ascii/FLOWCHART_SUPPORT.md,crates/merman-ascii/ASCII_GAP_REGISTRY.md,docs/workstreams/ascii-architecture-deepening]
  Goal: Ship a narrow but honest `FlowSubgraph.dir` subset by supporting local canonical `LR`
  subgraph layout inside canonical `TD` roots when routed edges remain fully internal to the
  subgraph, and document the remaining mixed-direction boundary gaps explicitly.
  Validation: `cargo nextest run -p merman-ascii flowchart subgraph`; `cargo nextest run -p merman-ascii graph_fixture`
  Review: Ensure routed edges only adopt local direction when both endpoints remain inside the same
  direction-bearing subgraph, and preserve global fallback for cross-boundary cases.
  Evidence: parser-backed flowchart model tests for shipped local `LR` behavior and documented
  fallback coverage for cross-boundary edges.
  Context: `crates/merman-ascii/ASCII_GAP_REGISTRY.md`, `crates/merman-ascii/FLOWCHART_SUPPORT.md`.
  Handoff: DONE. Later graph-routing work expanded this subset into explicit boundary-aware route
  planning; remaining graph gaps are tracked in `ASCII_GAP_REGISTRY.md`.

## M8 — Cross-Boundary Mixed-Direction Routing Seam

- [x] AAD-090 [owner=codex] [deps=AAD-080] [scope=crates/merman-ascii/src/graph/routing,crates/merman-ascii/tests/flowchart_model.rs,docs/workstreams/ascii-architecture-deepening]
  Goal: Introduce an explicit route-planning seam for edges that enter or leave a direction-bearing
  subgraph, starting with context classification and one bounded root `TD` / local `LR` strategy.
  Validation: targeted route-plan tests; `cargo nextest run -p merman-ascii flowchart subgraph`; `cargo nextest run -p merman-ascii flowchart`
  Review: Mixed-direction boundary behavior must be owned by routing context and planned segments,
  not by layout-only heuristics that incidentally move unrelated nodes.
  Evidence: route-plan classification tests, parser-backed subgraph boundary tests, and updated
  design/evidence docs.
  Context: `crates/merman-ascii/ASCII_GAP_REGISTRY.md`, `docs/workstreams/ascii-architecture-deepening/DESIGN.md`, `F:\\SourceCodes\\Rust\\merman\\repo-ref\\beautiful-mermaid\\src\\layout-engine.ts`.
  Handoff: DONE. Route seam has explicit request objects, boundary segment markers, TD same-rank
  merge coverage, turn-glyph regression tests, parser-backed boundary fixtures, and explicit
  canvas-extent ownership in `RoutePlan`.

## M9 — Route Extent And Group Topology Hardening

- [x] AAD-100 [owner=codex] [deps=AAD-090] [scope=crates/merman-ascii/src/graph/routing,crates/merman-ascii/src/graph/layout,docs/workstreams/ascii-architecture-deepening]
  Goal: Remove duplicate route canvas extent computation and replace repeated group membership /
  parent-depth reconstruction with a shared graph topology model.
  Validation: `cargo nextest run -p merman-ascii`; `cargo clippy -p merman-ascii --all-targets -- -D warnings`; `git diff --check`
  Review: Back-lane and top-down back-edge width contracts must remain explicit route-plan
  properties, not global padding. Boundary routing and group layout must read group membership from
  the same topology index.
  Evidence: `a48417a8e refactor(ascii): make route plans own canvas extent`;
  `3c1ee58c1 refactor(ascii): share graph group topology`.
  Context: `crates/merman-ascii/src/graph/routing/plan.rs`,
  `crates/merman-ascii/src/graph/topology.rs`.
  Handoff: DONE on 2026-06-26. All `merman-ascii` tests, clippy, and diff check passed.
