# C4 Layout Minimum Slice (Phase 1)

Baseline: Mermaid `@11.12.2`.

This document defines the initial, test-driven minimum slice for **headless layout** of C4
diagrams in `merman-render`.

Scope: geometry only (no SVG rendering yet). The goal is to match Mermaid’s layout math in
`packages/mermaid/src/diagrams/c4/c4Renderer.js`.

## Target behavior

- Boundary and shape placement follows Mermaid’s `Bounds.insert(...)` algorithm:
  - row wrapping is controlled by both `c4ShapeInRow` and `widthLimit` (`>=` comparisons).
  - the initial placement in a row uses `margin` (not `margin * 2`), subsequent placements use
    `margin * 2`.
  - `bumpLastMargin(c4ShapeMargin)` is applied after drawing a non-empty shape array.
- Boundary recursion follows Mermaid’s `drawInsideBoundary(...)`:
  - child boundary `widthLimit = parent.widthLimit / min(c4BoundaryInRow, childCount)`.
  - the per-boundary `setData(...)` uses `diagramMarginX/Y` and the boundary’s header text height
    (`Y` accumulator) to offset the inner content.

## Viewport-dependent width

Mermaid’s C4 renderer uses `screen.availWidth` as the root `widthLimit`.

In a headless Rust context there is no DOM/screen; for determinism and upstream parity with the
Mermaid CLI default, `merman-render` uses a **configurable viewport width** (default `800px`,
matching `@mermaid-js/mermaid-cli`’s default `-w 800`).

## Required layout snapshot fields (Phase 1)

Layout snapshots (`fixtures/c4/*.layout.golden.json`) must contain enough information to:

- reproduce Mermaid’s node/boundary geometry (x/y/width/height) and wrapping decisions.
- reproduce Mermaid’s text block sizing and vertical offsets used by the SVG renderer.
- reproduce Mermaid’s relationship line endpoints (intersection points).

Minimum fields:

- Diagram:
  - `bounds` (min/max box in diagram coordinates, excluding outer margins)
  - `width` / `height` (including `diagramMarginX/Y`)
  - `viewportWidth` / `viewportHeight` (used for parity/debugging)
- Shapes:
  - `alias`, `parentBoundary`, `typeC4Shape`
  - `x`, `y`, `width`, `height`, `margin`
  - `image` block: `{ width, height, y }`
  - text blocks: `typeC4Shape`, `label`, optional `type`, optional `techn`, optional `descr`:
    `{ text, y, width, height, lineCount }`
- Boundaries:
  - `alias`, `parentBoundary`
  - `x`, `y`, `width`, `height`
  - text blocks: `label`, optional `type`, optional `descr`:
    `{ text, y, width, height, lineCount }`
- Relationships:
  - `from`, `to`, `type`
  - `startPoint`, `endPoint`
  - optional `offsetX`, `offsetY`
  - text blocks: `label`, optional `techn`, optional `descr`:
    `{ text, width, height, lineCount }`

## Known Mermaid quirks to match

- `Bounds.setData(...)` does **not** reset the row counter (`nextData.cnt`), so the counter may
  carry over across boundary placements within the same `drawInsideBoundary(...)` call.
  The headless layout must mirror this behavior for parity with upstream.

