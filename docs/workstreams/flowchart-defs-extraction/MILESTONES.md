# Flowchart Defs Extraction - Milestones

Status: Complete
Last updated: 2026-05-28

## M0 - Scope Freeze

Status: Done

Exit criteria:

- Defs boundary is explicit.
- Validation gates are recorded.

## M1 - Extract Defs Module

Status: Done

Exit criteria:

- `flowchart/defs.rs` owns marker id formatting and marker emission.
- `flowchart/css.rs` no longer owns marker/defs code.
- Focused flowchart gates pass.

## M2 - Verification And Closeout

Status: Done

Exit criteria:

- Package gates pass.
- Evidence is recorded.
- `REFACTOR_TODO.md` reflects the completed flowchart defs slice.
