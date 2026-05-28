# Flowchart ViewBox Node Bounds Extraction - Milestones

Status: Complete
Last updated: 2026-05-28

## M0 - Scope Freeze

Status: Done

## M1 - Extract Node Bounds Module

Status: Done

Exit criteria:

- `flowchart/viewbox_node_bounds.rs` owns node rendered-bounds preparation.
- `viewbox.rs` delegates node bounds inclusion through a small function call.
- Focused flowchart gate passes.

## M2 - Consolidate Label Metrics

Status: Done

Exit criteria:

- Layout-node label measurement has one primary helper path.
- Shape-specific fallback semantics are preserved.
- Focused flowchart gate passes.

## M3 - Verification And Closeout

Status: Done

Exit criteria:

- Package gates pass.
- Evidence is recorded.
- `REFACTOR_TODO.md` reflects the completed flowchart split slice.
