# Flowchart Render Config And ViewBox Extraction - Milestones

Status: Complete
Last updated: 2026-05-28

## M0 - Scope Freeze

Status: Done

## M1 - Extract Render Configuration

Status: Done

Exit criteria:

- `flowchart/render_config.rs` owns effective render configuration preparation.
- `svg_emit.rs` receives a prepared config value instead of computing theme/font/label settings
  inline.
- Focused flowchart gate passes.

## M2 - Extract ViewBox And Content Bounds

Status: Done

Exit criteria:

- `flowchart/viewbox.rs` owns rendered content bounds and final viewBox preparation.
- Edge curve bbox union, recursive-root y expansion, and title bbox merging are outside
  `svg_emit.rs`.
- Flowchart DOM parity gate passes.

## M3 - Verification And Closeout

Status: Done

Exit criteria:

- Package gates pass.
- Evidence is recorded.
- `REFACTOR_TODO.md` reflects the completed flowchart split slice.
