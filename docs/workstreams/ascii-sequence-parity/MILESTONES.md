# ASCII Sequence Parity - Milestones

Status: Active
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
- Wrapped notes remain explicitly unsupported.
- Existing copied upstream sequence fixtures stay exact.

## M4 - Remaining Rich Constructs

Exit criteria:

- Sequence boxes, activations, create/destroy, and wrapping each have an independent next task or
  documented blocker.

## M5 - Verification And Commit

Exit criteria:

- Focused gates pass.
- Broad crate gates pass or any skipped gate is named with reason.
- Work is committed.
