# ASCII Sequence Rect And ParOver Blocks - Handoff

Status: Active
Last updated: 2026-05-29

## Current State

ASRP-010 and ASRP-020 are complete. The lane is open and scoped to the two remaining non-nested
Mermaid sequence control forms that were deferred by `ascii-sequence-control-blocks`: `rect` and
`par_over`.

The current renderer still rejects both as `control messages`. ASRP-020 freezes the source-of-truth
line types and diagnostic boundary:

- `rect` uses line types 22/23.
- `par_over` uses line type 32 followed by line type 21.

## Active Task

- Task ID: ASRP-030
- Owner: codex
- Files: `crates/merman-ascii/src/sequence`, `crates/merman-ascii/tests/sequence_model.rs`, `crates/merman-ascii/SEQUENCE_SUPPORT.md`
- Validation: `cargo fmt --all --check`; `cargo nextest run -p merman-ascii sequence_rect`; `cargo nextest run -p merman-ascii sequence_golden`; `git diff --check`
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

Implement ASRP-030 by rendering `rect <style>` as a single-section terminal frame while preserving
the style expression as text.
