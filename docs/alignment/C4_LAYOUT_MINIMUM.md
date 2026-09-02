# C4 Layout Minimum Contract

Baseline: pinned Mermaid `@11.17.2`.

This document defines the minimum compatibility contract for **headless layout** of C4 diagrams in
`merman-render`.

Scope: typed geometry and compatibility layout snapshots. The goal is to match the pinned Mermaid
layout math in `packages/mermaid/src/diagrams/c4/c4Renderer.js`; SVG root emission is owned by the
shared Root Viewport protocol.

## Target behavior

- Boundary and shape placement follows Mermaid's `Bounds.insert(...)` algorithm:
  - row wrapping is controlled by both `c4ShapeInRow` and `widthLimit` (`>=` comparisons).
  - the initial placement in a row uses `margin` (not `margin * 2`), subsequent placements use
    `margin * 2`.
  - `bumpLastMargin(c4ShapeMargin)` is applied after drawing a non-empty shape array.
- Boundary recursion follows Mermaid's `drawInsideBoundary(...)`:
  - child boundary `widthLimit = parent.widthLimit / min(c4BoundaryInRow, childCount)`.
  - the per-boundary `setData(...)` uses `diagramMarginX/Y` and the boundary’s header text height
    (`Y` accumulator) to offset the inner content.

## Container-dependent width

Pinned Mermaid uses `screen.availWidth` as the root C4 `widthLimit`.

In a headless Rust context there is no DOM or `screen`. The operation supplies
`LayoutOptions::container_width` and `LayoutOptions::container_height`, which default to `800px` and
`600px`, plus optional `LayoutOptions::screen_available_width`. C4 maps the explicit screen value to
Mermaid's root `widthLimit`; when it is absent, deterministic headless rendering falls back to the
container width. Binding callers use `layout.screen_available_width` when they host rendering in a
browser and need to project `screen.availWidth` exactly.

Compatibility layout JSON always contains `container_width` and `container_height`, and includes
`screen_available_width` when the host supplied it. `viewportWidth` and `viewportHeight` do not
exist. The removed binding names `layout.viewport_width` and `layout.viewport_height` are rejected
rather than treated as aliases.

## Required layout snapshot fields

Layout snapshots (`fixtures/c4/*.layout.golden.json`) must contain enough information to:

- reproduce Mermaid’s node/boundary geometry (x/y/width/height) and wrapping decisions.
- reproduce Mermaid’s text block sizing and vertical offsets used by the SVG renderer.
- reproduce Mermaid’s relationship line endpoints (intersection points).

Minimum fields:

- Diagram:
  - `bounds` (min/max box in diagram coordinates, excluding outer margins)
  - `width` / `height` (including `diagramMarginX/Y`)
  - `use_max_width`
  - `container_width` / `container_height` (operation layout-container dimensions)
  - optional `screen_available_width` (browser `screen.availWidth` projection)
  - `c4_type` and optional `title`
- Shapes:
  - `alias`, `parent_boundary`, `type_c4_shape`
  - `x`, `y`, `width`, `height`, `margin`
  - `image` block: `{ width, height, y }`
  - text blocks: `type_block`, `label`, optional `ty`, optional `techn`, optional `descr`:
    `{ text, y, width, height, line_count }`
- Boundaries:
  - `alias`, `parent_boundary`
  - `x`, `y`, `width`, `height`
  - `image` block: `{ width, height, y }`
  - text blocks: `label`, optional `ty`, optional `descr`:
    `{ text, y, width, height, line_count }`
- Relationships (`rels`):
  - source-order identity, `from`, `to`, `rel_type`
  - `start_point`, `end_point`
  - optional `offset_x`, `offset_y`, resolved by their named keys rather than positional sparsity
  - text blocks: `label`, optional `techn`, optional `descr`:
    `{ text, y, width, height, line_count }`

The SVG admission layer keeps relation identity, label role, line geometry, and explicit
presentation together. A DOM-order or text-only match is insufficient; see
`SEMANTIC_LABEL_PARITY.md`.

## Known Mermaid quirks to match

- `Bounds.setData(...)` does **not** reset the row counter (`nextData.cnt`), so the counter may
  carry over across boundary placements within the same `drawInsideBoundary(...)` call.
  The headless layout must mirror this behavior for parity with upstream.
