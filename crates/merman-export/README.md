# merman-export

`merman-export` is the bounded binary-export layer behind Merman's PNG, JPEG, and PDF output. It encodes SVG that has already passed Merman's terminal resvg-compatible finalizer; it does not parse Mermaid source or choose a layout engine.

Most applications should depend on [`merman`](https://crates.io/crates/merman) and select its
typed PNG, JPEG, or PDF target. Use this crate directly only when the application needs to retain a
validated SVG artifact, inspect complete export evidence and allocation plans, or schedule encoding
separately from Mermaid rendering.

The examples target the published `merman-export 0.11.0`. Select font discovery explicitly or provide a font context. See the changes for the next breaking release under [platform font discovery in development](#platform-font-discovery-in-development).

For `0.9.x` imports and output handoffs, see the [model/source migration](../../docs/migrations/model-source-boundaries.md). For available versions and installation channels, see the [host guide](../../docs/release/PREBUILT_PRODUCTS.md).

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
Implicit family and missing-glyph selection exclude symbolic/pictorial faces and fonts marked
as Last Resort by OpenType `head.flags` bit 14. Last Resort glyphs represent code-point ranges,
not readable character coverage; choosing one for a missing emoji can replace an otherwise
readable mixed-script label with placeholders. Explicitly named font requests remain available.
Direct exporters set
the same context with `RasterOptions::with_font_context` or
`PdfOptions::with_font_context`. PNG, JPEG, and PDF do not enable `system-fonts`.
Without a context, exports require explicitly compiled native system discovery and a usable
font-source policy; otherwise they return `ExportError::FontSourceUnavailable`, including for
an empty discovered database. With `system-fonts` compiled, the context supports
bounded system discovery through `NativeFontOptions::with_system_fonts(true)`;
library callers opt in explicitly. Selected bytes are snapshotted and verified
before measurement and painting. Explicit and system directory roots resolve symlinks
and ancestor aliases before scanning; traversal within each resolved root still skips
discovered symlinks. Metadata discovery reads one font file temporarily
under the cumulative scan-byte budget; the independent resident-byte budget applies
when a selected file is snapshotted, so discovery can inspect larger system collections.
The resident-byte budget does not cap temporary scan buffers or process peak memory.
See [CLI font coverage and limitations](../../docs/FEATURES.md#optional-cli-fonts).

`png` and `jpeg` share private bitmap preparation. `pdf` is a separate vector export capability. Features are additive, but one output does not implicitly expose another output's API. The published `merman-export`, `merman`, and `merman-render` versions must match because the sealed SVG type crosses their crate boundaries.

Themes can select font-family names and typography values, but caller-supplied theme font files
remain rejected. For explicit native shaping across layout and typed binary outputs, use the
separate `native-fonts` context rather than a theme recipe.

### Platform font discovery in development

The development source targets the next breaking release. When upgrading from
`0.11.0`, add `max_selection_entries` and `max_selection_bytes` to complete
`NativeFontLimits` struct literals. Literals using `..NativeFontLimits::default()`
continue to compile. Exhaustive `NativeFontError` matches must also handle
`PlatformDiscoveryUnavailable { .. }`.

With system fonts authorized, `load_platform` uses Fontique for family matching
and DirectWrite for fallback on Windows. On macOS it queries Core Text descriptors
one family at a time. Linux and FreeBSD submit the authored family order to
Fontconfig, which applies strong and weak aliases. Merman accepts scan-time aliases
after verifying the physical face and keeps sorted alternatives available while
checking text suitability and glyph coverage. Generic lookup stops at the first
accepted face.

Layout and painting share the same bounded snapshots. A system-only context starts
with zero resident font bytes and acquires each face on demand, including the
generic default for text without a font family. Platform metadata access and file
mappings run outside Merman's scan budgets and preemptive cancellation.
Before acquiring a new fallback source, mapped coverage tables can reject an
unrelated font without consuming the selected-file budget. Coverage and identity
are checked again on admitted bytes. Existing snapshots take precedence over
later file changes. When explicit files contain identical bytes, each supplied
path refers to the same retained snapshot. Each input still counts toward the
acquisition limits. If a repeated path changes during acquisition, registry
construction reports the path and asks the caller to retry.

On macOS, the retained Core Text font identifies the selected face. Merman usually
rebuilds its tables as a standalone SFNT. It can retain one immutable TTC or OTC
for faces such as Heiti SC and Heiti TC when the whole collection fits the remaining
byte budget and its member count fits the face limit. Merman checks that every
member points to the same byte range for an outline table larger than half the
collection, then matches the selected Core Text face to an on-disk member by its
font tables. Both faces can then use the same bytes, charged once. If the
collection exceeds the remaining byte budget, Merman admits the standalone face
when it fits. The first use charges the full size of a retained collection. With
a tight byte budget, selecting a collection before an unrelated font can therefore
reach the limit even when the reverse order fits. A face in an explicitly admitted
file can also be reused after its tables match. Measurement and export use index
zero in a standalone SFNT and the verified member index in a retained collection.

`for_export()` retains the operation's successful platform selections.
`for_operation()` starts a new scope and missing matches are retried. Cluster
requests include the authored stack, cluster text, language, direction, and faces
already tried. Selection metadata has independent limits of 16,384 entries
(`max_selection_entries`) and 4 MiB (`max_selection_bytes`) by default. Already
selected faces stay pinned even when the measurement cache is disabled.

With system fallback authorized on macOS, the resolver looks up an explicit face
by its PostScript name and verifies its admitted OpenType tables at the selected index
against the registered Core Text face before using that face's native cascade.
If no registered face matches, the resolver can use the Helvetica system cascade.
The resolver declines non-default native variation coordinates before data admission.

The default for `NativeFontLimits::max_bytes` increases from 64 MiB to 256 MiB.
Each native context counts explicit inputs and selected system data toward this total. Explicit files
and bounded system discovery use whole-file snapshots. Temporary buffers for
native tables, SFNT assembly and collection reads can contribute to peak process
memory. SFNT assembly preserves the selected face's glyph data, so a large color
or bitmap table can still exceed the budget.

Ordinary `load` still uses bounded system discovery. The platform context identity
covers its policy, initially admitted bytes, and the default family name when
derived from the system catalog. Later system snapshots are operation-local.
On Linux/FreeBSD, `load_platform` reports `PlatformDiscoveryUnavailable` when the
Fontconfig runtime cannot load, before acquiring explicit sources. A valid empty
Fontconfig configuration remains empty; library callers decide whether an unavailable
service permits another provider. The CLI retries bounded discovery only for automatic
provider selection, never for an explicit `--font-system platform` request.
Fontconfig named instances that require variation coordinates are skipped. CSS
weight can still vary the admitted physical face along its supported weight axis.

### Selection and fallback order

Workspace builds shape each missing cluster with one physical face that covers it
and preserve the other clusters in the run. Platform discovery tries the authored
stack, then caller fonts in source order, then the platform's candidates for the
missing cluster. Windows applies the relative font size returned by DirectWrite
to both the cluster metrics and the painted glyphs.

SVG text inherits `xml:lang`, and each `<tspan>` can override it. Library users can set
the default for Mermaid measurement and untagged SVG with
`NativeFontContext::with_language`. Registry builds using the published usvg 0.47
retain its scalar fallback interface. That interface can use the context language for
platform lookup and has the original whole-run shaping and replacement behavior.

For explicit font files or memory buffers without system discovery, fallback visits
admitted faces in source order, skipping previously tried, unsuitable, and incompatible
candidates. Supply individual files or buffers in the intended order when that priority
matters. Directory and system discovery have their own catalogue order.

Use a closed set for repeatable native output, and share the resulting context between layout
and painting:

```rust,no_run
use merman_export::{NativeFontOptions, NativeFontError};
use merman_model::OperationControl;

fn fonts(control: &OperationControl) -> Result<merman_export::NativeFontContext, NativeFontError> {
    NativeFontOptions::default()
        .with_font_file("fonts/Body-Regular.ttf")
        .with_font_file("fonts/PreferredFallback-Regular.ttf")
        .with_font_file("fonts/OtherFallback-Regular.ttf")
        .with_system_fonts(false)
        .load(control)
        .map(|context| context.with_strict_glyph_coverage(true))
}
```

This order expresses a context-wide preference. Per-script or per-label fallback policies are
not configurable through this interface. Exported SVG still needs matching fonts in its viewer;
native font bytes and resolver policy are not embedded in the SVG.

### Memory-Only And Bundled Fonts

`NativeFontContext::from_font_bytes(fonts, limits, &control)` accepts
`impl IntoIterator<Item = Vec<u8>>` and returns `Result<NativeFontContext, NativeFontError>`.
It owns admitted data and performs no file acquisition or ambient discovery, even when
`system-fonts` is compiled. File acquisition and system discovery are excluded from this
crate's WebAssembly code paths; explicit file/directory requests return a source error there.

```toml
[dependencies]
merman-export = { version = "0.11.0", default-features = false, features = ["png", "bundled-fonts"] }
merman-model = { version = "0.11.0", default-features = false }
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
merman = { version = "0.11.0", default-features = false, features = ["diagram-flowchart", "png", "system-fonts"] }
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
merman-model = { version = "0.11.0", default-features = false }
merman-render = { version = "0.11.0", default-features = false }
merman-export = { version = "0.11.0", default-features = false, features = ["png", "system-fonts"] }
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
