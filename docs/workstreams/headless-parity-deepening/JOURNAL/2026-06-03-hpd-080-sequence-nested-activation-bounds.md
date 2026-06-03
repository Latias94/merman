# HPD-080 - Sequence Nested Activation Bounds

Date: 2026-06-03

## Outcome

- Continued the Sequence activation audit after fixing autonumber marker placement and found the
  same source-backed bounds issue in the layout pass.
- Reproduced a visible endpoint defect with a nested activation sample. When a left-side actor sent
  a message to a participant with two active activation rectangles, local layout used only the
  innermost activation's left edge. Mermaid 11.15 uses the minimum left edge across all active
  rectangles for that actor.
- Updated `SequenceActivationState::actor_bounds(...)` to fold the full active stack and return the
  min left / max right bounds, matching Mermaid's `activationBounds(actor, actors)`.
- Added a focused regression proving a nested call from the left targets the outer activation's
  left edge after ordinary arrowhead shortening, while the remaining single activation keeps the
  same expected bound.

## Source Evidence

- `repo-ref/mermaid/packages/mermaid/src/diagrams/sequence/sequenceRenderer.ts`
  - `activationBounds(actor, actors)` reduces over every active activation for that actor.
  - `buildMessageModel(...)` uses those bounds for `startx`, `stopx`, and the
    `isArrowToActivation` adjustment.

## Focused Verification

- `cargo nextest run -p merman-render sequence_layout_nested_activation_bounds_include_full_stack_like_mermaid_11_15`
- `cargo nextest run -p merman-render --test sequence_svg_test`
- `cargo run -p xtask -- compare-sequence-svgs --check-dom --dom-mode parity --dom-decimals 3`
- `cargo run -p xtask -- update-layout-snapshots --filter ...` for the five affected Sequence
  activation fixtures
- `cargo nextest run -p merman-render`
- `cargo fmt -p merman-render --check`
- `git diff --check`

## Residual Note

- This is a source-backed endpoint geometry fix. It does not alter Sequence text measurement,
  note wrapping, or root-width residual handling.
- The SVG autonumber pass already has its own render-side activation-bounds state because Mermaid
  computes marker placement during SVG drawing. Layout and SVG now agree on the same full-stack
  bounds semantics without merging those separate pipeline phases.
