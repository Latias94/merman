# ASCII Sequence Rect And ParOver Blocks - Handoff

Status: Closed
Last updated: 2026-05-29

## Current State

ASRP-010 through ASRP-060 are complete. The lane is closed.

The renderer now supports `rect <style>` as a labeled single-section frame in ASCII and Unicode.
The style/color expression is preserved as text; it is not interpreted as terminal color or
background fill.

The renderer also supports `par_over <label>` as a labeled single-section frame. ASRP-040 handles
Mermaid/core's asymmetric representation explicitly: `par_over` starts with line type 32 and closes
with the normal `par` end line type 21.

ASRP-050 covers the remaining edge policy. Notes, activations, create/destroy lifecycle rows,
participant boxes, nested blocks, empty sections, and malformed ordering are all covered or
explicitly rejected for the `rect` / `par_over` subset.

ASRP-060 packages the final manual examples, updates closeout docs, and confirms the lane is closed.

- `rect` uses line types 22/23.
- `par_over` uses line type 32 followed by line type 21.

## Active Task

- Task ID: None
- Owner: codex
- Files: `crates/merman-ascii/src/sequence`, `crates/merman-ascii/tests/sequence_model.rs`, `crates/merman-ascii/SEQUENCE_SUPPORT.md`
- Validation: `cargo fmt --all --check`; `cargo nextest run -p merman-ascii`; `cargo nextest run -p merman --features ascii`; `cargo nextest run -p merman-cli --features ascii`; `cargo clippy -p merman-ascii -p merman --features ascii --all-targets -- -D warnings`; `git diff --check`
- Status: Closed
- Review: No blocking findings remained at closeout.
- Evidence: Manual outputs and closeout gate results recorded in `EVIDENCE_AND_GATES.md`.

## Decisions Since Last Update

- `rect` should render as a region frame that preserves the style/color expression as text, not as
  ANSI color.
- `par_over` should display as `par_over`, not as plain `par`.
- Nested control blocks and empty sections remain out of scope unless explicitly pulled in.
- ASRP-030 intentionally reuses the existing single-section control frame path for `rect`.
- ASRP-040 uses explicit `ParOver` start matching with the normal `Par` end signal instead of
  collapsing display text to `par`.
- ASRP-050 adds broad sequence edge-policy coverage and a box-background fix so control-frame labels
  stay readable when group boxes overlap them.
- ASRP-060 closes the lane with generated manual examples and final closeout gates.

## Blockers

- None.

## Next Recommended Action

Lane closed. Any future parity work should start a new follow-on.
