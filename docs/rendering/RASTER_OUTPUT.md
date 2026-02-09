# Raster Output (PNG/JPG/PDF)

`merman-cli` can render Mermaid diagrams to **SVG** (authoritative in this repo) and also to
**PNG/JPG/PDF** for quick previews and integrations.

## Why raster output is “best effort”

Upstream Mermaid renders in the browser and relies heavily on HTML inside SVG via
`<foreignObject>` for labels (flowchart/state/class/mindmap/kanban/...).

The pure-Rust raster stack used by `merman-cli`:

- parses SVG via `usvg`
- rasterizes via `resvg` (PNG/JPG)
- converts via `svg2pdf` (PDF)

does **not** fully support rendering `<foreignObject>` HTML content. Without additional handling,
many diagrams would rasterize as “geometry only” (boxes/lines) or even look empty.

## Current approach

To keep SVG parity strictness focused on `merman-render` while still producing useful raster
outputs, `merman-cli` applies a **raster-only SVG preprocessing pass**:

- For raster formats (PNG/JPG/PDF), convert common `<foreignObject>` label patterns into SVG
  `<text>/<tspan>` elements (approximate alignment + line breaks).
- The fallback accounts for common Mermaid positioning patterns where labels are placed via parent
  `<g transform="translate(x,y)">` wrappers (so overlays land in the right place for kanban/mindmap).
- Keep the default SVG output unchanged (no impact on upstream SVG baselines).

This makes `tools/preview/export-fixtures-png.ps1` produce readable previews across most diagrams
without bundling a browser engine.

## Library usage

If you want the same PNG/JPG/PDF output without spawning the CLI, enable the `raster` feature on
the `merman` crate and call the helpers directly:

```rust
use merman::render::raster::{svg_to_jpeg, svg_to_pdf, svg_to_png, RasterOptions};

let svg = "<svg><!-- ... --></svg>";

let mut opts = RasterOptions::default();
opts.scale = 2.0;
opts.background = Some("white".to_string());

let png = svg_to_png(svg, &opts)?;
let jpg = svg_to_jpeg(svg, &opts)?;
let pdf = svg_to_pdf(svg)?;
# Ok::<(), Box<dyn std::error::Error>>(())
```

Note: raster helpers intentionally apply the `<foreignObject>` readability fallback described
above. Strict upstream SVG baselines are still generated from the original SVG output.

## Known gaps

- The `<text>` fallback is an approximation and is not expected to be pixel-identical to upstream.
- Complex HTML labels (nested markup, icons, rich styling) may degrade in raster output until we
  add more specialized conversions or a dedicated text layout pipeline.
