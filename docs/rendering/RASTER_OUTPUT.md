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
outputs, `merman-cli` and the `merman::render::raster::render_*_sync` helpers apply the explicit
`SvgPipeline::resvg_safe()` preset:

- For raster formats (PNG/JPG/PDF), convert common `<foreignObject>` label patterns into SVG
  `<text>` elements (approximate alignment + line breaks).
- The fallback accounts for common Mermaid positioning patterns where labels are placed via parent
  `<g transform="translate(x,y)">` wrappers (so overlays land in the right place for kanban/mindmap).
- Strip the original `<foreignObject>` elements after the fallback overlay is inserted.
- Remove common `usvg` / `resvg` hazards such as unsupported `@keyframes` / `:root` CSS blocks,
  animation declarations, CSS `deg` units, empty visual attributes, and non-finite values such as
  `NaN`.
- Keep the default SVG output unchanged (no impact on upstream SVG baselines).

This makes `tools/preview/export-fixtures-png.ps1` produce readable previews across most diagrams
without bundling a browser engine.

Note on sizing:

- For raster formats, the output pixel size is derived from the root `viewBox` (when present).
- We round the base `viewBox` width/height up to whole pixels and then apply `--scale` so
  integer scaling behaves as expected (e.g. `--scale 2` produces exactly 2× the pixels of
  `--scale 1` for the same SVG).

## Library usage

If you want render-and-raster output without spawning the CLI, enable the `raster` feature on the
`merman` crate and call `HeadlessRenderer`:

```rust
use merman::render::{
    HeadlessRenderer,
    raster::{RasterFitBox, RasterOptions},
};

let renderer = HeadlessRenderer::new().with_diagram_id("raster-doc-example");
let raster = RasterOptions::default()
    .with_fit_to(RasterFitBox::contain(960, 540))
    .with_scale(2.0)
    .with_background("white");
let bytes = renderer
    .render_png_sync("flowchart TD; A[Layer 7\\nHTTP]-->B;", &raster)?
    .unwrap();
# let _ = bytes;
# Ok::<(), Box<dyn std::error::Error>>(())
```

The same path is available as a runnable repository example:

```sh
cargo run -p merman --features raster --example example_05_raster_output
```

If you already have an SVG string and want the same preprocessing before calling the lower-level
`svg_to_*` functions, apply the pipeline first:

```rust
use merman::render::{
    raster::{svg_to_jpeg, svg_to_pdf, svg_to_png, RasterOptions},
    svg_resvg_safe,
};

let svg = "<svg><!-- ... --></svg>";
let svg = svg_resvg_safe(svg)?;

let mut opts = RasterOptions::default();
opts.scale = 2.0;
opts.background = Some("white".to_string());

let png = svg_to_png(svg, &opts)?;
let jpg = svg_to_jpeg(svg, &opts)?;
let pdf = svg_to_pdf(svg)?;
# Ok::<(), Box<dyn std::error::Error>>(())
```

Strict upstream SVG baselines are still generated from the original parity SVG output.

## Known gaps

- The `<text>` fallback is an approximation and is not expected to be pixel-identical to upstream.
- Complex HTML labels (nested markup, icons, rich styling) may degrade in raster output until we
  add more specialized conversions or a dedicated text layout pipeline.
