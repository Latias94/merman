# Flowchart Render Input Extraction - Milestones

Status: Complete
Last updated: 2026-05-28

## M0 - Scope Freeze

Status: Done

## M1 - Extract Render Input Module

Status: Done

Exit criteria:

- `flowchart/render_input.rs` owns render edge/helper node preparation.
- `svg_emit.rs` no longer contains self-loop expansion logic.
- Focused flowchart gates pass.

## M2 - Verification And Closeout

Status: Done

Exit criteria:

- Package gates pass.
- Evidence is recorded.
- `REFACTOR_TODO.md` reflects the completed flowchart split slice.
