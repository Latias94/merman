# HPD-080 - Sequence Autonumber Activation Bounds

Date: 2026-06-03

## Outcome

- Reproduced the user-visible Sequence defect with the reported `autonumber` + activation sample:
  local output placed sequence numbers `2` and `4` on the right edge of the Server activation
  rectangle, while Mermaid places them on the left edge; local `5` was on the left while Mermaid
  places it on the right.
- Confirmed pinned Mermaid 11.15 computes `autonumberX` from current activation bounds, arrow
  direction, and reverse-arrow type. It does not use the message line's first point as the marker
  anchor.
- Added a Sequence SVG regression that checks the same sample against the Server activation rect:
  `2` and `4` must sit at `activationLeft + 1`, and `5` must sit at `activationRight - 1`.
- Updated the Sequence SVG message renderer to maintain a render-pass activation-bounds state while
  iterating messages. `ACTIVE_START` / `ACTIVE_END` directives now update that state, and ordinary
  message autonumber markers use the Mermaid 11.15 `activationBounds(...)` / `fromBounds` /
  `toBounds` formula.
- Moved `activationWidth` into `SequenceRenderSettings` so activation rectangles and sequence-number
  marker placement consume the same parsed config value.

## Source Evidence

- `repo-ref/mermaid/packages/mermaid/src/diagrams/sequence/sequenceRenderer.ts`
  - `activationBounds(actor, actors)` returns active activation min/max bounds with a
    center `-1/+1` fallback.
  - `buildMessageModel(...)` stores `fromBounds` and `toBounds` as the min/max of both endpoint
    activation bounds.
  - The autonumber drawing path selects `fromBounds + 1` or `toBounds - 1` from self-message,
    left-to-right, and reverse-arrow cases.

## Focused Verification

- `cargo nextest run -p merman-render sequence_autonumber_anchors_to_current_activation_bounds_like_mermaid_11_15`
- `cargo nextest run -p merman-render --test sequence_svg_test`
- `cargo nextest run -p merman-render`
- `cargo fmt -p merman-render --check`
- `git diff --check`

## Residual Note

- This slice fixes a visible marker-anchor bug. It does not claim full Sequence pixel parity or
  change Sequence text measurement/root-width residuals.
- The render-side activation-bounds state intentionally mirrors Mermaid's current message pass.
  The older layout-side activation state still has narrower responsibilities and should be audited
  separately if nested activation layout residuals appear.
