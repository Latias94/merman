# ASCII Architecture Deepening — TODO

Status: Closed
Last updated: 2026-05-30

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
