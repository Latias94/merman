# ADR 0057: Headless SVG Text `getBBox()` Approximation

- Status: Proposed
- Date: 2026-02-03
- Baseline: Mermaid `@11.12.2`

## Context

Mermaid derives many diagram root viewports via browser DOM measurement:

- `setupGraphViewbox(svg, padding, useMaxWidth)` computes `viewBox` from `svg.getBBox()` and adds `padding * 2`.
- For diagrams that emit SVG `<text>` (non-HTML labels), the measured `getBBox()` depends on:
  - font-family / font-size / font-weight (often injected via a `<style>` element)
  - whitespace tokenization into multiple `<tspan>` runs (Mermaid does this in `createText`)
  - anchor/alignment (`text-anchor`, baseline attributes), including inheritance
  - browser quantization/hinting and sub-pixel placement (often on a 1/64px lattice)

`merman` is headless, so we must approximate browser `getBBox()` to achieve DOM parity-root
(`viewBox` and root `style="max-width: ...px"`).

We already vendor font metrics tables and use them for:

- deterministic HTML/SVG label sizing in layout (Dagre)
- specific viewBox parity fixes (e.g. pie/c4)

However, the generic emitted-SVG bbox pass currently only accounts for element geometry
(`rect/path/circle/foreignObject/...`) and cannot model CSS-influenced `<text>` sizing unless we
explicitly add it.

## Decision

We will treat headless `svg.getBBox()` approximation as a **two-layer system**:

1) **Geometry layer (pure SVG attributes):**
   - Parse emitted SVG and union element bounds for the geometry we emit:
     - `rect/circle/ellipse/line/path/polyline/polygon/foreignObject`
   - Support `transform="translate(x,y)"` stacking on `<g>` (already in place).
   - Ensure attribute lookup matches whole attribute names (avoid substring collisions like
     `d="..."` matching inside `id="..."`).

2) **Text layer (best-effort, diagram-aware):**
   - For diagrams where text is emitted without a concrete bounding geometry (e.g. Architecture
     service labels where the `<rect class="background"/>` has no width/height), the renderer
     must union a headless text bbox estimate into the root content bounds.
   - Text sizing uses `VendoredFontMetricsTextMeasurer` and Mermaid-like SVG bbox extents
     (`measure_svg_text_bbox_x`), including Mermaid whitespace tokenization behavior.
   - Diagram renderers may maintain their own higher-level bounds (e.g. service label bounds) and
     union them into the root viewport computation rather than attempting to fully parse and
     interpret nested `<tspan>` positioning and inherited presentation attributes.

This decision keeps the generic emitted-SVG bbox logic stable while allowing targeted fixes for
parity-root mismatches driven by `<text>`.

## Alternatives Considered

### A) Full XML + CSS + text layout engine for `getBBox()`

Pros:
- Most accurate in principle.

Cons:
- Large scope, many dependencies, and high risk of divergent behavior vs Chromium.
- Requires implementing style cascade/inheritance, font fallback, and SVG text layout.

### B) Ignore `<text>` in bbox and rely on container geometry

Pros:
- Simple and stable.

Cons:
- Fails when Mermaid emits `<text>` without explicit geometry bounds (Architecture service labels,
  some titles/axis labels), producing incorrect root `viewBox/max-width` in parity-root mode.

### C) Add generic `<text>` parsing to emitted-SVG bbox pass

Pros:
- More complete headless bbox.

Cons:
- Requires interpreting nested `<tspan>` positioning and inherited presentation attributes,
  which can destabilize already-passing diagrams and is hard to match exactly.

We choose the two-layer approach (Decision) as the best cost/benefit trade-off.

## Consequences

- Root viewport parity improvements will be implemented incrementally per diagram.
- The bbox pass remains robust and deterministic; diagram renderers opt in to text-layer unions
  when needed for parity.
- Some remaining parity-root deltas may still come from upstream browser quantization and layout
  subtleties; we will address those with fixture-driven calibration where necessary.

