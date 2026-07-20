# SVG, Raster, and PDF Output

Merman exposes four output contracts from the same headless render operation. SVG is the pure
vector path. PNG and JPEG allocate a final pixel buffer. PDF keeps ordinary SVG geometry as vector
content and rasterizes only operations such as filters that require a bitmap.

PNG/JPEG and PDF export start from the explicit `SvgPipeline::resvg_safe()` contract. Callers can
also request that contract for SVG output. The pipeline converts common `<foreignObject>` labels
into SVG `<text>` fallbacks, removes the original HTML, strips active SVG content and unsupported
CSS, and returns a sealed `ResvgCompatibleSvg` artifact. The default Mermaid-parity SVG path
remains unchanged.

## Output and resource contracts

| Output | Sizing policy | Default resource boundary |
| --- | --- | --- |
| SVG | Preserves the computed vector `viewBox` and root dimensions. | No global width or height cap. Source, layout, label, and SVG-byte budgets still apply. |
| PNG/JPEG | Derives a pixel size from the SVG, optional fit box, and scale. | `RasterOptions` limits each side to 4096 pixels and the image to 16,777,216 pixels before allocation. |
| PDF | Uses `PdfOptions` and a vector page policy independent of `RasterOptions`. | No page-pixel budget. Localized filter bitmaps and embedded raster images have separate budgets. |

An SVG with a very large but finite `viewBox` can therefore be a valid result. A browser can paint
that vector document into a small viewport without allocating a pixmap at the intrinsic SVG size.
PNG and JPEG cannot: the encoder must allocate the final pixels, so their limits are intentional.

The sealed resvg-compatible contract also has a non-optional structural capability boundary. It
checks both the final XML tree and the `usvg` tree after references and subresources are resolved.
Native builds accept at most 256 levels and run `usvg`, `resvg`, and `krilla-svg` work on a bounded
8 MiB worker stack; WebAssembly accepts at most 64 levels because it cannot create that worker
stack. Raw parity SVG does not enter this recursive backend and is not subject to this depth cap.

### PNG and JPEG

`RasterOptions` controls four independent concerns:

- `fit_to` constrains the displayed SVG to a CSS-pixel box while preserving its aspect ratio;
- `scale` applies device-pixel scaling after the fit;
- `size_limit` constrains the final pixmap before allocation;
- `embedded_image_limit` checks embedded PNG/JPEG/GIF/WebP bytes and dimensions before decode.

The default `RasterSizeLimit` is 4096 pixels per side and 16,777,216 total pixels. If a requested
output exceeds a limit, Merman reduces it proportionally instead of first allocating the oversized
pixmap. `RasterPlan` exposes both requested and final dimensions. JPEG also has the format's
65,535-pixel per-side encoder limit.

The default decoded-image budget accepts at most 16,777,216 intrinsic pixels in one embedded image
and 33,554,432 across all embedded images, including recursively embedded SVG resources. This
budget is separate from the final PNG/JPEG dimensions.

### Vector PDF

`PdfOptions` intentionally does not reuse the PNG/JPEG pixmap limit. Its `PdfPagePolicy` supports:

- `FitSvg` (the library default), which uses the intrinsic SVG dimensions as PDF points;
- `Fixed { width_pt, height_pt }`, which fits and centers the SVG on a fixed page;
- `FitCssWidth { max_width_px }`, which models a responsive CSS viewport and converts 96 CSS
  pixels per inch to 72 PDF points per inch.

The page remains vector regardless of its dimensions. SVG filters may require localized bitmap
sampling inside the PDF; `PdfOptions` requests a filter scale of 4 by default and caps the aggregate
filter bitmap plan at 33,554,432 pixels. Merman lowers the effective filter scale when necessary.
Embedded raster images use the same independent decoded-image policy described above.

Unbounded modes are scoped deliberately. `RasterOptions::with_unbounded_size()` affects only the
final PNG/JPEG pixmap. `PdfOptions::with_unbounded_filter_images()` affects only localized PDF
filter bitmaps. `EmbeddedImageLimit::unbounded()` affects only embedded image decoding. None of
them disables parser, layout, label, SVG-byte, or recursive-backend capability limits.

## CLI sizing behavior

For PNG/JPEG previews, use `--raster-fit-width` and/or `--raster-fit-height`, then use `--scale` for
device-pixel ratio. The default pixmap budget can be changed with `--raster-max-width`,
`--raster-max-height`, and `--raster-max-pixels`, or disabled for trusted input with
`--raster-unbounded`. When the CLI automatically constrains an output, it reports the requested
and final pixel dimensions on stderr unless `--quiet` is set.

Top-level PDF export without `--pdfFit` uses a fixed 612-by-792-point Letter approximation. With
`--pdfFit`, Merman uses `FitCssWidth`: `--width` supplies the CSS viewport width (800 pixels by
default), responsive SVG width is constrained to that viewport, and the result is converted at
72/96 PDF points per CSS pixel. This matches the sizing units used by Chromium's CSS-to-PDF path;
the resulting drawing is still produced by Merman's browserless vector backend.

PDF filter sampling is controlled by `--pdf-filter-scale`, `--pdf-max-filter-pixels`, and
`--pdf-filter-unbounded`. Embedded image decoding is controlled for both raster and PDF export by
`--embedded-image-max-pixels`, `--embedded-image-max-total-pixels`, and
`--embedded-images-unbounded`. Markdown batch encoding additionally uses
`--encoding-memory-budget-mib` to bound aggregate in-flight encoding memory.

## Library usage

Enable the `raster` feature on the `merman` crate and choose options for the actual output type:

```rust
use merman::render::{
    HeadlessRenderer,
    raster::{PdfOptions, PdfPagePolicy, RasterFitBox, RasterOptions},
};

let renderer = HeadlessRenderer::new().with_diagram_id("export-doc-example");

let raster = RasterOptions::default()
    .with_fit_to(RasterFitBox::contain(960, 540))
    .with_scale(2.0)
    .with_background("white");
let png = renderer
    .render_png_sync("flowchart TD; A[Layer 7\\nHTTP]-->B;", &raster)?
    .unwrap();

let pdf = PdfOptions::default().with_page_policy(PdfPagePolicy::FitCssWidth {
    max_width_px: 800.0,
});
let pdf = renderer
    .render_pdf_with_options_sync("flowchart TD; A[Layer 7\\nHTTP]-->B;", &pdf)?
    .unwrap();

# let _ = (png, pdf);
# Ok::<(), Box<dyn std::error::Error>>(())
```

The same path is available as a runnable repository example:

```sh
cargo run -p merman --features raster --example example_05_raster_output
```

If an application already owns SVG text, finalize it before calling low-level encoders. Those
encoders accept only the sealed `ResvgCompatibleSvg` artifact:

```rust
use merman::render::{
    RenderEnvironment, finalize_resvg_svg,
    raster::{
        PdfOptions, RasterOptions, svg_to_jpeg, svg_to_pdf_with_options, svg_to_png,
    },
};

let source = "<svg xmlns=\"http://www.w3.org/2000/svg\" width=\"10\" height=\"10\"/>";
let session = RenderEnvironment::parity().begin_session()?;
let svg = finalize_resvg_svg(source, &session)?;

let raster = RasterOptions::default().with_scale(2.0);
let png = svg_to_png(&svg, &raster)?;
let jpeg = svg_to_jpeg(&svg, &raster)?;
let pdf = svg_to_pdf_with_options(&svg, &PdfOptions::default())?;

# let _ = (png, jpeg, pdf);
# Ok::<(), Box<dyn std::error::Error>>(())
```

Use `prepare_raster` or `prepare_pdf` when a host needs to inspect the allocation plan or reserve
memory before encoding. Their scheduling weights include the native recursive-backend worker stack
in addition to output pixels, decoded images, and encoder overhead.

## Known gaps

- The `<text>` fallback for browser-only HTML labels is approximate and is not expected to be
  pixel-identical to Chromium.
- Complex nested HTML, icons, rich CSS, browser fonts, and filter rendering may differ from the
  upstream browser result.
- Browser print pagination, margins, and CSS print behavior are outside the pure-Rust PDF contract.
