# ASCII Sequence Rect And ParOver Blocks - Handoff

Status: Active
Last updated: 2026-05-29

## Current State

ASRP-010 is complete. The lane is open and scoped to the two remaining non-nested Mermaid sequence
control forms that were deferred by `ascii-sequence-control-blocks`: `rect` and `par_over`.

The current renderer still rejects both as `control messages`. Source-of-truth references show:

- `rect` uses line types 22/23.
- `par_over` uses line type 32 followed by line type 21.

## Active Task

- Task ID: ASRP-020
- Owner: codex
- Files: `crates/merman-ascii/tests/sequence_model.rs`, `crates/merman-ascii/SEQUENCE_SUPPORT.md`
- Validation: `cargo fmt --all --check`; `cargo nextest run -p merman-ascii sequence_rect_par_over`; `git diff --check`
- Status: Ready
- Review: Required before accepting completion.
- Evidence: Pending.

## Decisions Since Last Update

- `rect` should render as a region frame that preserves the style/color expression as text, not as
  ANSI color.
- `par_over` should display as `par_over`, not as plain `par`.
- Nested control blocks and empty sections remain out of scope unless explicitly pulled in.

## Blockers

- None.

## Next Recommended Action

Implement ASRP-020 by adding parser/render boundary tests for `rect` and `par_over`.
