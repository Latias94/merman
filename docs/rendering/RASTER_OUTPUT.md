# SVG, PNG, JPEG, and PDF Output

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

### Host fonts and embedded images

The native PNG, JPEG, and PDF exporters discover fonts from the host system on their first use.
That scan is cached process-wide and shared by subsequent exports. It is host-dependent: the same
SVG can select different installed fonts, metrics, or fallback glyphs on different machines. Font
discovery is not covered by Merman's resource profiles or raster/PDF allocation budgets, so hosts
that require isolation or a hard memory/latency ceiling must provide it at the process boundary.

Embedded images have a narrower input contract. The native exporters resolve only `data:` URLs;
they do not read image paths from the filesystem and do not fetch images over the network. The
default decode limits are 16,777,216 bytes and 16,777,216 intrinsic pixels per image, with
33,554,432 bytes and 33,554,432 intrinsic pixels across the output. These limits apply before
PNG/JPEG/PDF encoding and are independent of system-font discovery and the general resource
profile.

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

The default decoded-image budget accepts the byte and intrinsic-pixel limits described above,
including recursively embedded SVG resources. This budget is separate from the final PNG/JPEG
dimensions.

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

The `mmdc` command's PDF export without `--pdfFit` uses a fixed 612-by-792-point Letter
approximation. With `--pdfFit`, Merman uses `FitCssWidth`: `--width` supplies the CSS viewport
width (800 pixels by default), responsive SVG width is constrained to that viewport, and the
result is converted at 72/96 PDF points per CSS pixel. This matches the sizing units used by
Chromium's CSS-to-PDF path; the resulting drawing is still produced by Merman's browserless
vector backend.

PDF filter sampling is controlled by `--pdf-filter-scale`, `--pdf-max-filter-pixels`, and
`--pdf-filter-unbounded`. Embedded image decoding is controlled for both raster and PDF export by
`--embedded-image-max-pixels`, `--embedded-image-max-total-pixels`, and
`--embedded-images-unbounded`. Markdown batch work is admitted against the selected resource
profile's aggregate scheduling-weight budget. Use
`--resource-limit max_scheduling_weight_bytes=BYTES` with `--jobs` only when a scoped override is
required.

## Library usage

Enable only the binary outputs the application uses (`png`, `jpeg`, and/or `pdf`) on the `merman`
crate. PNG and JPEG share private bitmap preparation; PDF remains a separate vector export path.
The following example uses both `png` and `pdf`:

```toml
[dependencies]
merman = { version = "=0.8.0-alpha.5", default-features = false, features = ["png", "pdf"] }
```

```rust
use merman::svg::export::{PdfOptions, PdfPagePolicy, RasterFitBox, RasterOptions};
use merman::{
    OperationControl, PdfRequest, PngRequest, RenderOutput, RenderRequest, Renderer, SvgRequest,
};

let renderer = Renderer::new();
let source = "flowchart TD; A[Layer 7\\nHTTP]-->B;";
let svg = SvgRequest {
    options: merman::svg::SvgRenderOptions {
        diagram_id: Some("export-doc-example".to_string()),
        ..Default::default()
    },
    ..Default::default()
};

let raster = RasterOptions::default()
    .with_fit_to(RasterFitBox::contain(960, 540))
    .with_scale(2.0)
    .with_background("white");
let RenderOutput::Png(Some(png)) = renderer.render(RenderRequest::png(
    source,
    OperationControl::new(),
    PngRequest {
        svg: svg.clone(),
        options: raster,
    },
))? else {
    return Err("no Mermaid diagram detected".into());
};

let pdf = PdfOptions::default().with_page_policy(PdfPagePolicy::FitCssWidth {
    max_width_px: 800.0,
});
let RenderOutput::Pdf(Some(pdf)) = renderer.render(RenderRequest::pdf(
    source,
    OperationControl::new(),
    PdfRequest { svg, options: pdf },
))? else {
    return Err("no Mermaid diagram detected".into());
};

# let _ = (png.bytes, pdf.bytes);
# Ok::<(), Box<dyn std::error::Error>>(())
```

The same path is available as a runnable repository example:

```sh
cargo run -p merman --features png --example render_png
```

If an application already owns SVG text, finalize it before calling low-level encoders. Those
encoders accept only the sealed `ResvgCompatibleSvg` artifact:

```toml
[dependencies]
merman-core = { version = "=0.8.0-alpha.5", default-features = false }
merman-render = { version = "=0.8.0-alpha.5", default-features = false }
merman-export = { version = "=0.8.0-alpha.5", default-features = false, features = ["png", "jpeg", "pdf"] }
```

```rust
use merman_core::OperationControl;
use merman_export::{
    PdfOptions, RasterOptions, svg_to_jpeg_controlled, svg_to_pdf_controlled,
    svg_to_png_controlled,
};
use merman_render::{environment::RenderEnvironment, svg::finalize_resvg_svg};

let source = "<svg xmlns=\"http://www.w3.org/2000/svg\" width=\"10\" height=\"10\"/>";
let control = OperationControl::new();
let session = RenderEnvironment::parity().begin_session_with_control(control.clone())?;
let svg = finalize_resvg_svg(source, &session)?;

let raster = RasterOptions::default().with_scale(2.0);
let png = svg_to_png_controlled(&svg, &raster, control.clone())?;
let jpeg = svg_to_jpeg_controlled(&svg, &raster, control.clone())?;
let pdf = svg_to_pdf_controlled(&svg, &PdfOptions::default(), control)?;

# let _ = (png, jpeg, pdf);
# Ok::<(), Box<dyn std::error::Error>>(())
```

The `pdf` feature exposes no PNG or JPEG API. Its current `krilla-svg` dependency still brings a
transitive SVG/raster implementation closure, so this is an API and direct-feature boundary rather
than a claim that a PDF-only binary contains no raster implementation at all. The artifact-profile
closure checks record that residual explicitly.

Use `prepare_raster` or `prepare_pdf` when a host needs to inspect the allocation plan or reserve
memory before encoding. Their scheduling weights include the native recursive-backend worker stack
in addition to output pixels, decoded images, and encoder overhead.

## Known gaps

- The `<text>` fallback for browser-only HTML labels is approximate and is not expected to be
  pixel-identical to Chromium.
- Fallback typography is resolved from the source element context before `foreignObject` removal.
  Host CSS injected after that point can restyle generated text, but cannot recompute the wrapping
  or placement that Merman already measured.
- Complex nested HTML, icons, rich CSS, browser fonts, and filter rendering may differ from the
  upstream browser result.
- Browser print pagination, margins, and CSS print behavior are outside the pure-Rust PDF contract.
