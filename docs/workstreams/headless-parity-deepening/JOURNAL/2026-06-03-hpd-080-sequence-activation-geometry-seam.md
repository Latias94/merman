# HPD-080 - Sequence Activation Geometry Seam

Date: 2026-06-03

## Outcome

- Refactored the Mermaid 11.15 Sequence activation geometry formulas into shared helpers after the
  autonumber and nested-endpoint fixes exposed the same logic in multiple render phases.
- Added `sequence_activation_start_x(...)` for Mermaid's stacked activation offset formula.
- Added `sequence_activation_stack_bounds(...)` for Mermaid's full-stack min-left / max-right
  activation bounds.
- Updated the layout activation state, SVG activation-rectangle plan, and SVG autonumber
  render-pass state to use the shared helpers instead of repeating the formulas.
- Added focused helper unit tests for empty, single, and stacked activation bounds.

## Source Evidence

- `repo-ref/mermaid/packages/mermaid/src/diagrams/sequence/sequenceRenderer.ts`
  - `bounds.newActivation(...)` / `ACTIVE_START` handling computes stacked activation start x from
    actor center, current stack length, and `activationWidth`.
  - `activationBounds(actor, actors)` folds every active activation for the actor into min-left /
    max-right bounds with a center `-1/+1` fallback.

## Focused Verification

- `cargo nextest run -p merman-render activation_start_x_matches_mermaid_stack_offsets activation_stack_bounds_fold_full_active_stack sequence_autonumber_anchors_to_current_activation_bounds_like_mermaid_11_15 sequence_layout_nested_activation_bounds_include_full_stack_like_mermaid_11_15`
- `cargo nextest run -p merman-render --test sequence_svg_test`
- `cargo run -p xtask -- compare-sequence-svgs --check-dom --dom-mode parity --dom-decimals 3`
- `cargo nextest run -p merman-render`

## Residual Note

- This is a seam consolidation, not a new behavior change. The goal is to prevent Sequence layout,
  activation rectangles, and autonumber marker placement from drifting apart again.
- Sequence text measurement and root-width residuals remain separate residual classes.
