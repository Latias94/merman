# ASCII Sequence Parity - Milestones

Status: Closed
Last updated: 2026-05-29

## M0 - Scope And Gap Ledger

Exit criteria:

- Copied upstream sequence fixture status is documented.
- First executable sequence product gap is selected.

## M1 - Open Arrow Messages

Exit criteria:

- Typed sequence message types for `->` and `-->` render.
- Existing copied upstream sequence fixtures stay exact.

## M2 - Rich Sequence Constructs Inventory

Exit criteria:

- Notes, boxes, activations, create/destroy, and wrapping are split into follow-on tasks or a new
  lane.

## M3 - Notes Rendering

Exit criteria:

- Single-line notes render for right-of, left-of, and over placements.
- Existing copied upstream sequence fixtures stay exact.

## M4 - Remaining Rich Constructs

Exit criteria:

- Sequence boxes render with documented plain-text limitations.
- Activations, create/destroy, and message/note wrapping render.
- Wrapped actor labels, wrapped boxes, and richer control blocks have documented follow-on
  boundaries.

## M5 - Verification And Commit

Exit criteria:

- Focused gates pass.
- Broad crate gates pass or any skipped gate is named with reason.
- Work is committed.

## M6 - Closeout Review

Exit criteria:

- Workstream docs mark the lane closed.
- Residual unsupported features are listed in `SEQUENCE_SUPPORT.md`.
- Richer Mermaid control blocks are split into a new workstream boundary.
