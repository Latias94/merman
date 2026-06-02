# SVG Output Pipeline

`merman` has two SVG output contracts:

- `render_svg_sync` returns Mermaid-parity SVG and remains the default.
- `SvgPipeline` turns that parity SVG into consumer-oriented output for previews, raster export,
  or host-specific cleanup.

Default SVG output is not optimized or cleaned by default because parity output is the comparison
surface for upstream Mermaid fixtures. Consumers that need renderer compatibility should opt in to
a pipeline explicitly.

Typical choices:

- Use `render_svg_sync` when the caller wants the closest Mermaid-compatible SVG string.
- Use `render_svg_readable_sync` or `SvgPipeline::readable()` for browser previews that can keep `<foreignObject>` but should also expose SVG text fallbacks.
- Use `render_svg_resvg_safe_sync` or `SvgPipeline::resvg_safe()` before PNG/JPG/PDF export through `resvg` / `usvg`.
- Add `SvgPostprocessor` passes when a host application needs product-specific styling, metadata, or cleanup after a built-in preset.

## Presets

| Preset | Behavior |
| --- | --- |
| `SvgPipeline::parity()` | No post-processing. This preserves the exact SVG string produced by the parity renderer. |
| `SvgPipeline::readable()` | Adds best-effort SVG `<text>` overlays for labels emitted via `<foreignObject>`. |
| `SvgPipeline::resvg_safe()` | Adds readable fallbacks, strips the original `<foreignObject>` elements, and removes common `usvg` / `resvg` hazards such as unsupported CSS blocks, animation declarations, CSS `deg` units, empty visual attributes, empty rectangle placeholders, and non-finite values. |

## Rendering With A Pipeline

```rust
use merman::render::{HeadlessRenderer, SvgPipeline};

let renderer = HeadlessRenderer::new();
let svg = renderer
    .render_svg_with_pipeline_sync(
        "flowchart TD; A[Layer 7\\nHTTP]-->B;",
        &SvgPipeline::resvg_safe(),
    )?
    .unwrap();
# let _ = svg;
# Ok::<(), Box<dyn std::error::Error>>(())
```

Runnable example:

```bash
cargo run -p merman --features render --example svg_pipeline < fixtures/flowchart/basic.mmd > out.svg
```

The compatibility helpers are wrappers around the same pipeline:

- `render_svg_readable_sync(...)` uses `SvgPipeline::readable()`.
- `render_svg_resvg_safe_sync(...)` uses `SvgPipeline::resvg_safe()`.
- `svg_readable(svg)` and `svg_resvg_safe(svg)` apply the presets to an existing SVG string.

## Host Postprocessors

Applications can append product-specific passes after a built-in preset. The postprocess context
includes preset, pass ordering, diagram type, diagram title, and root SVG id:

```rust
use merman::render::{
    RenderResult, SvgPipeline, SvgPostprocessContext, SvgPostprocessor,
};
use std::borrow::Cow;

struct AddComment;

impl SvgPostprocessor for AddComment {
    fn name(&self) -> &'static str {
        "add-comment"
    }

    fn process<'a>(
        &self,
        svg: Cow<'a, str>,
        ctx: &SvgPostprocessContext<'_>,
    ) -> RenderResult<Cow<'a, str>> {
        Ok(Cow::Owned(format!(
            "{svg}<!-- type={} id={} -->",
            ctx.diagram_type().unwrap_or("unknown"),
            ctx.svg_id().unwrap_or("unknown"),
        )))
    }
}

let pipeline = SvgPipeline::resvg_safe().with_postprocessor(AddComment);
# let _ = pipeline;
```

Built-in passes always run before custom postprocessors, and custom postprocessors run in insertion
order. Custom pass errors are surfaced as render errors with the pass name attached.

## Built-In Host Styling Blocks

Host styling should use product-neutral postprocessors rather than modifying `resvg_safe` itself:

```rust
use merman::render::{
    CssOverridePolicy, HeadlessRenderer, ScopedCssPostprocessor, SvgPipeline,
};

let renderer = HeadlessRenderer::new().with_diagram_id("host-diagram");
let pipeline = SvgPipeline::resvg_safe().with_postprocessor(
    ScopedCssPostprocessor::new(
        r#"
.node rect {
  stroke: #2563eb;
  stroke-width: 2px;
}
.merman-foreignobject-fallback-text {
  fill: #111827;
}
"#,
    )
    .with_override_policy(CssOverridePolicy::StripExistingImportant),
);

let svg = renderer
    .render_svg_with_pipeline_sync("flowchart TD; A-->B;", &pipeline)?
    .unwrap();
# let _ = svg;
# Ok::<(), Box<dyn std::error::Error>>(())
```

`ScopedCssPostprocessor` injects a `<style>` element under the root `<svg>` tag and prefixes normal
selectors with the root SVG id. When the SVG already has style elements, the injected style is placed
after them so host rules follow Mermaid defaults in cascade order. `CssOverridePolicy::StripExistingImportant`
is opt-in because it changes cascade semantics. Generated `<foreignObject>` fallback text keeps useful
classes and inline font/fill hints so host CSS can target readable fallback output. When the same pipeline
feeds raster export, keep injected CSS in the `usvg` / `resvg` supported subset; browser-only features
such as CSS custom properties are better reserved for inline-only SVG pipelines or resolved by the host
before rasterizing.

Product-specific rules still belong in host code. For example, Zed-style accent token assignment,
theme color selection, and diagram-family-specific color semantics should be implemented as custom
`SvgPostprocessor` passes layered after these generic blocks.

Binding consumers can opt into the generic duplicate-fallback cleanup without writing a Rust
postprocessor:

```json
{
  "svg": {
    "pipeline": "resvg-safe",
    "drop_native_duplicate_fallbacks": true
  }
}
```

This removes fallback groups only when their text duplicates native SVG `<text>` output. It does not
apply host palette replacement or remove fallback-only labels.
