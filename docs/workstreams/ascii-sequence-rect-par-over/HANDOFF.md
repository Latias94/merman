# ASCII Sequence Rect And ParOver Blocks - Handoff

Status: Active
Last updated: 2026-05-29

## Current State

ASRP-010 through ASRP-040 are complete. The lane is open and scoped to the two remaining non-nested
Mermaid sequence control forms that were deferred by `ascii-sequence-control-blocks`: `rect` and
`par_over`.

The renderer now supports `rect <style>` as a labeled single-section frame in ASCII and Unicode.
The style/color expression is preserved as text; it is not interpreted as terminal color or
background fill.

The renderer also supports `par_over <label>` as a labeled single-section frame. ASRP-040 handles
Mermaid/core's asymmetric representation explicitly: `par_over` starts with line type 32 and closes
with the normal `par` end line type 21.

ASRP-020 freezes the source-of-truth line types:

- `rect` uses line types 22/23.
- `par_over` uses line type 32 followed by line type 21.

## Active Task

- Task ID: ASRP-050
- Owner: codex
- Files: `crates/merman-ascii/src/sequence`, `crates/merman-ascii/tests/sequence_model.rs`, `crates/merman-ascii/SEQUENCE_SUPPORT.md`
- Validation: `cargo fmt --all --check`; `cargo nextest run -p merman-ascii sequence_rect_par_over sequence_control_blocks`; `cargo nextest run -p merman-ascii`; `git diff --check`
- Status: Ready
- Review: Required before accepting completion.
- Evidence: Pending.

## Decisions Since Last Update

- `rect` should render as a region frame that preserves the style/color expression as text, not as
  ANSI color.
- `par_over` should display as `par_over`, not as plain `par`.
- Nested control blocks and empty sections remain out of scope unless explicitly pulled in.
- ASRP-030 intentionally reuses the existing single-section control frame path for `rect`.
- ASRP-040 uses explicit `ParOver` start matching with the normal `Par` end signal instead of
  collapsing display text to `par`.

## Blockers

- None.

## Next Recommended Action

Implement ASRP-050 by covering supported combinations and explicit edge diagnostics for `rect` and
`par_over`.
