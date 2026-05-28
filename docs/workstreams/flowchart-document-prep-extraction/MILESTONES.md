# Flowchart Document Prep Extraction - Milestones

Status: Complete
Last updated: 2026-05-28

## M0 - Scope Freeze

Status: Done

Exit criteria:

- Document prep boundary is explicit.
- Validation gates are recorded.

## M1 - Extract Document Prep

Status: Done

Exit criteria:

- `flowchart/document.rs` owns root viewport formatting and root SVG opening.
- `flowchart/svg_emit.rs` no longer formats root attrs directly.
- Focused flowchart gates pass.

## M2 - Verification And Closeout

Status: Done

Exit criteria:

- Package gates pass.
- Evidence is recorded.
- `REFACTOR_TODO.md` reflects the completed flowchart split slice.
