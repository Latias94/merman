# 0048: Viewport-Dependent Layout Width (Headless)

Date: 2026-01-19

## Status

Accepted

## Context

Some Mermaid diagrams derive layout geometry from browser/DOM state rather than purely from the
diagram definition and Mermaid config. A notable example is C4, where the renderer uses
`screen.availWidth` as the root `widthLimit` for row wrapping in `c4Renderer.js`.

In `merman`, the core goal is full parity with Mermaid `@11.12.2` while remaining **headless** and
usable by multiple UI frameworks. This requires a deterministic, explicit replacement for
DOM/screen-derived values.

## Decision

- `merman-render` exposes a **viewport size** in `LayoutOptions`.
- C4 headless layout uses `LayoutOptions.viewport_width` / `LayoutOptions.viewport_height` as the
  equivalent of Mermaid’s `screen.availWidth` / viewport, with defaults matching the Mermaid CLI:
  - `viewport_width = 800`
  - `viewport_height = 600`
- Diagram-specific layout code may choose to use (or ignore) the viewport values depending on the
  upstream renderer behavior. The default is to ignore viewport unless required for parity.

## Consequences

- Layout snapshots become deterministic and reproducible across environments.
- Upstream SVG baselines generated via Mermaid CLI can be compared meaningfully by using the same
  default viewport width (or explicitly passing `-w` when needed).
- Consumers embedding `merman` can tune the viewport to match their target container width, while
  still producing Mermaid-compatible layouts.

