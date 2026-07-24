# merman-render

[![Crates.io](https://img.shields.io/crates/v/merman-render.svg)](https://crates.io/crates/merman-render)
[![Documentation](https://docs.rs/merman-render/badge.svg)](https://docs.rs/merman-render)
[![Crates.io Downloads](https://img.shields.io/crates/d/merman-render.svg)](https://crates.io/crates/merman-render)
[![Made with Rust](https://img.shields.io/badge/made%20with-Rust-orange.svg)](https://www.rust-lang.org)

[![License: MIT](https://img.shields.io/badge/License-MIT-yellow.svg)](https://opensource.org/licenses/MIT)
[![License: Apache 2.0](https://img.shields.io/badge/License-Apache%202.0-blue.svg)](https://opensource.org/licenses/Apache-2.0)

`merman-render` is the low-level layout and SVG crate behind
[merman](https://crates.io/crates/merman). It consumes typed `merman-core` family semantics and
produces compatibility layout JSON or Mermaid-like SVG through one family artifact.

Mermaid-compatible language configuration and sanitizer behavior are always available. This crate
has empty defaults. `layout-cytoscape` opts into the shared Architecture FCoSE and Mindmap
COSE-Bilkent implementation backed by `manatee`. When that backend is enabled, Architecture and
non-`tidy-tree` Mindmap diagrams use those source-backed layouts. Builds without it report those
families as unavailable for rendering while retaining their parsing and semantic capabilities. Add
the individual `system-clock`, `system-timezone`, `system-random`, or `system-timing` adapter only
when the embedding product needs it.

ELK integration is kept behind the explicit `layout-elk` feature in this low-level crate and in
the public `merman` facade. A facade `svg` build does not pull it in; callers that need ELK must
enable `layout-elk` explicitly. When enabled, Flowchart ELK, Class, and ER layout use the sole
source-backed Rust implementation of Mermaid's ELK adapter and Eclipse ELK layered pipeline; no
compatibility backend or runtime selector is retained.

Most applications should start with the `merman` crate and `merman::svg::HeadlessRenderer`. Use
`merman-render` directly when you need lower-level control over layout, text measurement, SVG
options, or SVG postprocessing.

## What It Provides

- Headless layout for parsed Mermaid diagrams.
- Mermaid-parity SVG emission.
- `FamilyRenderArtifact`, which keeps one matching built-in semantic/layout pair opaque and
  projects layout JSON or consuming SVG output.
- `LayoutOptions::headless_svg_defaults()` for editor/export use cases.
- Text measurement hooks through `TextMeasurer`.
- Math rendering hooks through `MathRenderer`.
- Shared Root Viewport policy for computed sizing, accessibility chrome, and root SVG emission.
- `SvgPipeline` presets and postprocessors for readable or rasterizer-friendly SVG.

## Render Environment

`RenderEnvironment` owns adapters and policy for one operation: named text-measurement routes,
math rendering, icons, time, randomness, and resource limits.
Call `begin_session()` once before layout and retain that `RenderSession` through SVG and any raster
postprocessing so those phases observe the same snapshot and provenance. The higher-level
`HeadlessRenderer` also applies the frozen session date and timezone to date-sensitive parsing;
direct low-level callers are responsible for configuring the core `Engine` consistently.

`TextMeasurer` keeps browser DOM primitives distinct. In particular,
`measure_svg_create_text_bbox_y_offset_px` measures ordinary Mermaid createText, while
`measure_svg_create_text_middle_bbox_y_offset_px` measures Architecture's formatted text under an
inherited middle baseline. The latter is font- and x-height-dependent and cannot reuse the former.
The vendored profile's pinned middle-baseline shift is a deterministic fallback, not a general
system-font formula; an authoritative host measurement bypasses it.

This is a breaking replacement for independently configured layout and SVG services. Text and math
adapters no longer live in `LayoutOptions`, and render code does not read process-global policy.
Production request values stay in `SvgRenderOptions`; diagnostics, including timing output, live in
`SvgDebugOptions` and are accepted only by the explicit `*_with_debug` entry points.

## Direct Rendering Example

```rust
use merman_core::{Engine, ParseOptions};
use merman_render::{
    environment::RenderEnvironment, family, LayoutOptions,
};
use merman_render::svg::{
    SvgDebugOptions, SvgPipeline, SvgPostprocessMetadata, SvgRenderOptions,
};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let engine = Engine::new();
    let parsed = engine
        .parse_diagram_for_render_model_sync(
            "flowchart TD\nA[API] --> B[DB]",
            ParseOptions::strict(),
        )?
        .expect("diagram detected");

    let layout_options = LayoutOptions::headless_svg_defaults();
    let session = RenderEnvironment::parity().begin_session()?;
    let artifact = family::prepare(parsed, &layout_options, session)?;

    // Compatibility layout JSON projects from this exact typed family artifact.
    let layout_json = artifact.layout_json()?;
    eprintln!("layout family: {}", layout_json["meta"]["diagram_type"]);

    let svg_options = SvgRenderOptions {
        diagram_id: Some("example-diagram".to_string()),
        ..SvgRenderOptions::default()
    };

    // SVG consumes the artifact, so its semantic model and layout cannot be recombined.
    let rendered = artifact.render_svg(&svg_options, &SvgDebugOptions::default())?;
    let (svg, family_kind, metadata, session) = rendered.into_parts();
    assert_eq!(family_kind, family::RenderFamilyKind::Flowchart);
    let pipeline_metadata = SvgPostprocessMetadata::from_svg(&svg)
        .with_family_kind(family_kind)
        .with_diagram_type(metadata.diagram_type)
        .with_optional_diagram_title(metadata.title);
    let svg = SvgPipeline::resvg_safe()
        .process_to_string_with_metadata(&svg, &pipeline_metadata, &session)?;
    println!("{svg}");

    Ok(())
}
```

## SVG Output Pipelines

The default SVG renderer aims for Mermaid DOM parity. Host applications can opt into an output
pipeline after rendering:

- `SvgPipeline::parity()` leaves the SVG unchanged.
- `SvgPipeline::readable()` keeps fallback text for `<foreignObject>` labels.
- `SvgPipeline::resvg_safe()` prepares SVG for common `usvg` / `resvg` rasterization paths.
- `ScopedCssPostprocessor`, `CssOverridePostprocessor`, and custom `SvgPostprocessor`
  implementations let applications inject host-specific styling without forking the renderer.

See [`docs/rendering/SVG_OUTPUT_PIPELINE.md`](https://github.com/Latias94/merman/blob/main/docs/rendering/SVG_OUTPUT_PIPELINE.md) for the higher-level integration guide.

## Relationship To merman

`merman` re-exports the common render APIs behind its `svg` feature and adds
`HeadlessRenderer`, consuming `prepare_render_sync` stages, SVG id sanitization helpers, and
optional raster helpers. Direct `merman-render` users call `family::prepare` and retain its
`RenderSession`; the old public `layout_parsed*`, `render_layouted_svg`, raw model/layout SVG
helpers, and per-family pass-through wrappers are not retained as compatibility paths.
