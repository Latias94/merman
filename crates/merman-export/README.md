# merman-export

> [!NOTE]
> This README documents the current `main` branch. The operation-scoped `Renderer`,
> `RenderedDocument`, and target-admission examples were introduced after the published
> `0.8.0-alpha.5` tag. Use matching Git revisions for current-main APIs.

`merman-export` is the bounded binary-export layer behind Merman's PNG, JPEG, and PDF output. It encodes SVG that has already passed Merman's terminal resvg-compatible finalizer; it does not parse Mermaid source or choose a layout engine.

Most applications should depend on [`merman`](https://crates.io/crates/merman) and select its
typed PNG, JPEG, or PDF target. Use this crate directly only when the application needs to retain a
validated SVG artifact, inspect complete export evidence and allocation plans, or schedule encoding
separately from Mermaid rendering.

This guide targets the development checkout after published `0.9.3`. Independent presentation data and explicit font-discovery features belong to the next breaking development release; use the `v0.9.3` tagged guide for the maintenance APIs and codec defaults. See the [model/source migration](../../docs/migrations/model-source-boundaries.md) for current imports and output handoffs.

## Choose A Feature

The crate has no default features. Enable only the formats the application emits:

| Feature | Output | Main API |
| --- | --- | --- |
| `png` | Bounded PNG bitmap | `svg_to_png`, `prepare_raster`, `RasterOptions`, `RasterPlan` |
| `jpeg` | Bounded JPEG bitmap | `svg_to_jpeg`, `prepare_raster`, `RasterOptions`, `RasterPlan` |
| `pdf` | Vector PDF with bounded localized raster work | `svg_to_pdf`, `svg_to_pdf_with_options`, `prepare_pdf`, `PdfOptions` |
| `native-fonts` | Explicit font services, independent of output codecs | `NativeFontOptions`, `NativeFontContext`, `NativeFontLimits` |
| `bundled-fonts` | Production Cousine regular, bold, and italic; enables `native-fonts` | `bundled_font_data` |
| `system-fonts` | Explicitly compile native system discovery, independent of codecs | `system_font_discovery_available` |
| `experimental-system-font-query` | Opt-in platform-managed discovery, not a bounded metadata scan | `NativeFontOptions::load_platform` |

The optional native context uses bounded explicit TTF/OTF/TTC sources and shared
font selection. Install it through the facade's
`SvgEnvironment::with_native_font_context` for shared layout, wrapping, SVG
bounds, and computed-length measurement across diagram families, with automatic
reuse by typed binary outputs. All 23 measurement protocol operations are
supported; missing-glyph requests retain deterministic fallback by default. Opt into
`NativeFontContext::with_strict_glyph_coverage(true)` to return a normal error instead of
declining unsupported measurement or silently painting missing glyphs.
This does not promise browser pixel parity or arbitrary HTML/CSS typography.
Direct exporters set
the same context with `RasterOptions::with_font_context` or
`PdfOptions::with_font_context`. PNG, JPEG, and PDF do not enable `system-fonts`.
Without a context, exports require explicitly compiled native system discovery and a usable
font-source policy; otherwise they return `ExportError::FontSourceUnavailable`, including for
an empty discovered database. With `system-fonts` compiled, the context supports
bounded system discovery through `NativeFontOptions::with_system_fonts(true)`;
library callers opt in explicitly. Selected bytes are snapshotted and verified
before measurement and painting. Metadata discovery reads one font file temporarily
under the cumulative scan-byte budget; the independent resident-byte budget applies
when a selected file is snapshotted, so discovery can inspect larger system collections.
The resident-byte budget does not cap temporary scan buffers or process peak memory.
See [CLI font coverage and limitations](../../docs/FEATURES.md#optional-cli-fonts).

Experimental `load_platform` replaces only authorized system discovery with
Fontique queries. Its internal platform metadata/file mappings do not inherit
Merman's scan budgets or preemptive cancellation. Selected font snapshots remain
bounded; all ordinary `load` and default-feature behavior remains unchanged.
The context identity describes policy and initially admitted bytes, not a
cross-process content hash of every subsequently selected platform font.

`png` and `jpeg` share private bitmap preparation. `pdf` is a separate vector export capability. Features are additive, but one output does not implicitly expose another output's API. The published `merman-export`, `merman`, and `merman-render` versions must match because the sealed SVG type crosses their crate boundaries.

Themes can select font-family names and typography values, but caller-supplied theme font files
remain rejected. For explicit native shaping across layout and typed binary outputs, use the
separate `native-fonts` context rather than a theme recipe.

### Memory-Only And Bundled Fonts

`NativeFontContext::from_font_bytes(fonts, limits, &control)` accepts
`impl IntoIterator<Item = Vec<u8>>` and returns `Result<NativeFontContext, NativeFontError>`.
It owns admitted data and performs no file acquisition or ambient discovery, even when
`system-fonts` is compiled. File acquisition and system discovery are excluded from this
crate's WebAssembly code paths; explicit file/directory requests return a source error there.

```toml
merman-export = { version = "0.9.3", default-features = false, features = ["png", "bundled-fonts"] }
```

```rust
use merman_model::OperationControl;
use merman_export::{NativeFontContext, NativeFontError, NativeFontLimits, bundled_font_data};

fn font_context(
    caller_fonts: impl IntoIterator<Item = Vec<u8>>,
    control: &OperationControl,
) -> Result<NativeFontContext, NativeFontError> {
    NativeFontContext::from_font_bytes(
        caller_fonts.into_iter().chain(bundled_font_data()),
        NativeFontLimits::default(),
        control,
    )
    .map(|context| context.with_strict_glyph_coverage(true))
}
```

Caller order is authoritative: earlier equal family/style descriptors win. File and memory
sources share validation, byte deduplication, face admission, and content identity ordering.
`max_files` counts every input file or memory buffer; `max_bytes` charges cumulative input
bytes before deduplication. Exact duplicates do not add resident faces or change identity.
Collection face limits are checked before parsing. Empty sets and malformed fonts return the
existing structured errors; malformed memory inputs identify `<memory-font-N>` rather than a
real filesystem path. Caller allocations already made before admission are not bounded by
these limits.

`bundled-fonts` is opt-in and does not automatically construct or inject a context.
`bundled_font_data()` copies production resources from `assets/fonts/`, never test fixtures.
The pinned font inventory, provenance, and OFL notice live in that directory. Cousine is not
a general CJK/emoji font pack; provide additional fonts for labels it does not cover. Exported
SVG still references font families rather than embedding these bytes.

`with_strict_glyph_coverage` starts an independent scope and includes the coverage policy in
context identity. `for_operation(&control)` resets caches, probe budgets, and failures;
`for_export(&control)` preserves admitted snapshots, replays an existing fatal error, and
isolates subsequent export state. Both preserve strict coverage. A strict missing-glyph error
can fail its operation without poisoning a new operation fork from the reusable context.

### Native Product Compatibility

Products that previously obtained ambient fonts transitively from `png`, `jpeg`, or `pdf`
must now explicitly select `system-fonts`, or supply a context. In this repository, the
integration owner must update the native `rust-export-{png,jpeg,pdf,native-sdk}` recipes and
the corresponding facade/bindings/C SDK recipes (`rust-all`, `rust-native-sdk`,
`rust-bindings-core-native-sdk`, and `c-abi-native`) when retaining that behavior. The complete
CLI recipe already requests `system-fonts`. Forward the feature through product manifests as
needed; keep memory-only WASI recipes free of it. Native C and WASI products choosing bundled
fonts must explicitly enable `bundled-fonts`, construct caller-first contexts, and package the
production font inventory and OFL notice.

## First Export

The `merman` facade owns the shortest source-to-output path. This dependency enables PNG and its
required basic SVG path without Cytoscape, ELK, or math engines:

```toml
[dependencies]
merman = { version = "0.9.3", default-features = false, features = ["diagram-flowchart", "png", "system-fonts"] }
```

```rust
use merman::svg::export::{RasterFitBox, RasterOptions};
use merman::{OperationControl, PngRequest, RenderOutput, RenderRequest, Renderer, SvgRequest};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let options = RasterOptions::default()
        .with_fit_to(RasterFitBox::contain(960, 540))
        .with_scale(2.0)
        .with_matte("white");

    let output = Renderer::new().render(RenderRequest::png(
        "flowchart LR\n  Source --> PNG",
        OperationControl::new(),
        PngRequest {
            svg: SvgRequest::default(),
            options,
        },
    ))?;
    let RenderOutput::Png(Some(png)) = output else {
        return Err("no Mermaid diagram detected".into());
    };

    std::fs::write("diagram.png", png.bytes())?;
    Ok(())
}
```

Replace `png` with `jpeg` or `pdf` when only that format is required. JPEG uses `RasterOptions`; PDF uses its independent `PdfOptions` page and filter policy. Use `into_bytes()` only when deliberately discarding render and target-admission evidence.

`RasterOptions::matte` and `PdfOptions::page_paint` are output-compositing controls. They do not
change the diagram theme canvas or trigger layout. Use theme configuration for diagram-owned
backgrounds, and use these options only when the target file needs paint behind the rendered SVG.

## Direct Encoding

A host that already owns SVG can run the terminal finalizer and encoder under one caller-owned
operation control:

```toml
[dependencies]
merman-model = { version = "0.9.3", default-features = false }
merman-render = { version = "0.9.3", default-features = false }
merman-export = { version = "0.9.3", default-features = false, features = ["png", "system-fonts"] }
```

```rust
use merman_model::OperationControl;
use merman_export::{RasterOptions, svg_to_png_controlled};
use merman_render::{environment::RenderEnvironment, svg::finalize_resvg_svg};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let control = OperationControl::new();
    let session = RenderEnvironment::deterministic()
        .begin_session_with_control(control.clone())?;
    let sealed = finalize_resvg_svg(
        r#"<svg xmlns="http://www.w3.org/2000/svg" width="120" height="40">
              <text x="8" y="24">Render then encode</text>
            </svg>"#,
        &session,
    )?;
    let png = svg_to_png_controlled(&sealed, &RasterOptions::default(), control)?;
    std::fs::write("diagram.png", png)?;
    Ok(())
}
```

The encoder APIs accept `merman_render::svg::ResvgCompatibleSvg`, whose inner string cannot be forged directly. This type attests that the terminal resvg-compatible finalizer ran. It does not attest that the input originated from Mermaid: `merman_render::svg::finalize_resvg_svg` intentionally accepts arbitrary SVG and returns the sealed type after finalization. Applications that accept untrusted SVG still need appropriate source, time, memory, and concurrency policy.

The sealed producer contract intentionally keeps `merman-render` in this crate's resolved dependency closure. `merman-export` is therefore not a standalone arbitrary-SVG converter, and its crate boundary alone is not evidence that parsing or rendering dependencies were removed. Exact PNG, JPEG, and PDF closure claims are verified from the repository's artifact profiles.

All formats keep explicit allocation, embedded-image, and structural conversion limits. See the main project's [SVG, PNG, JPEG, and PDF output guide](https://github.com/Latias94/merman/blob/main/docs/rendering/RASTER_OUTPUT.md) for sizing policy, resource controls, PDF behavior, and known parity gaps.

## License

Licensed under either of Apache License, Version 2.0 or MIT at your option.
