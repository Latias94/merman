# Embedding Merman in a Rust application

This guide covers the `0.8.0` Rust feature and request interface, whose publication is recorded in the [October 6 snapshot](../release/PUBLISH_ORDER.md#080-publication-snapshot). Current source-branch compiled-theme work remains experimental; use a matching source checkout and artifact for those interfaces. Merman selects Mermaid 12.1; review the [alpha.7-to-0.8.0 migration](../release/ALPHA7_TO_0_8_0_UPGRADE_GUIDE.md) for rendering and low-level ELK changes. Use [the stable upgrade guide](../release/V070_TO_V080_UPGRADE_GUIDE.md) when starting from `0.7.0`, and the [versioned index](../release/README.md) for earlier prereleases.

## Select languages, outputs, and layout backends

Choose these independently. `diagram-*` selects built-in languages and their implementations; `svg` selects SVG output; `layout-*` selects optional layout engines and implies `svg`. A layout feature does not select a language, and a language feature does not select an optional layout engine. This keeps parser-only users and applications with small allowlists from acquiring unrelated implementations.

For the currently published package, an SVG host preserving the pre-selector parser surface and Cytoscape support uses:

```toml
[dependencies]
merman = { version = "=0.8.0", default-features = false, features = ["all-diagrams", "svg", "layout-cytoscape"] }
```

For current source-branch development, point the same feature selection at the reviewed checkout. Default facade builds include all languages, SVG, Cytoscape, ELK, and math. Disable defaults when that exceeds the host's requirements; `complete-svg` adds SVG, Cytoscape, and math without ELK, and still needs a family selection. See [Features](../FEATURES.md) for the complete matrix and distribution notices.

For a host accepting only Flowchart and Sequence, replace `all-diagrams` with `diagram-flowchart, diagram-sequence`. Forward selectors from a reusable library rather than assuming another dependency will enable them. Cargo features are additive: inspect the final application with `cargo tree -e features`, and verify a narrow selection in an independent consumer workspace.

## Replace the previous renderer

The former `HeadlessRenderer` and its source-to-SVG methods are replaced by `Renderer::render`. Keep reusable engine configuration on the renderer and per-output settings on `SvgRequest`:

```rust
use merman::svg::{CssOverridePostprocessor, SvgPipeline};
use merman::{Engine, MermaidConfig, OperationControl, RenderOutput, RenderRequest, Renderer, SvgRequest};

fn render_for_host(
    source: &str,
    config: MermaidConfig,
    diagram_id: String,
) -> Result<String, Box<dyn std::error::Error>> {
    let renderer = Renderer::new().with_engine(Engine::new().with_site_config(config));
    let mut request = SvgRequest::default();
    request.options.diagram_id = Some(diagram_id);
    request.pipeline = Some(
        SvgPipeline::resvg_safe()
            .with_postprocessor(CssOverridePostprocessor::strip_existing_important()),
    );
    let output = renderer.render(RenderRequest::svg(source, OperationControl::new(), request))?;
    let RenderOutput::Svg(Some(svg)) = output else {
        return Err("no Mermaid diagram detected".into());
    };
    Ok(svg.svg().to_owned())
}
```

Reuse the configured `Renderer` across calls when site configuration and defaults are shared; construct a fresh request and operation control for each operation. Clone the operation control for the task that cancels it. Return or classify parse errors, cancellation, resource exhaustion, and missing capabilities separately. A successful `None` means no diagram was detected, not that a failed operation was ignored.

`SvgPipeline::resvg_safe()` prepares SVG for the supported resvg/usvg path, including conversion of HTML labels. The CSS override postprocessor is useful when the host deliberately supplies final styles; omit it when those overrides are not part of the integration. Give co-located SVGs IDs that remain unique after `sanitize_svg_id()` normalization. A resvg-compatible artifact does not authorize arbitrary browser navigation or host DOM insertion; browser hosts should use the owning browser admission APIs.

## Example: an editor with a family allowlist

The reviewed Zed reference snapshot pins `0.8.0-alpha.5` and selects `svg, layout-cytoscape` with defaults disabled. Updating that declaration alone compiles on alpha.7, but strict parsing fails because no built-in family is selected. Add `all-diagrams` to preserve the former language surface, or select the families from the host's own allowlist:

```toml
merman = { version = "=0.8.0", default-features = false, features = [
    "svg", "layout-cytoscape",
    "diagram-flowchart", "diagram-sequence", "diagram-class", "diagram-state",
    "diagram-er", "diagram-gantt", "diagram-pie", "diagram-git-graph",
    "diagram-mindmap", "diagram-timeline", "diagram-quadrant-chart",
    "diagram-xychart", "diagram-journey",
] }
```

These 13 logical families cover that reference snapshot's allowlist; `graph`, State header variants, and `xychart-beta` use the selector of their owning family. The all-family recipe retains parser availability beyond that allowlist; the subset recipe deliberately rejects other languages. Neither declaration adds ELK or math. Review newly accepted languages separately if the host later expands its allowlist.

Migrate the alpha.5 API as well: move `with_site_config` to `Engine`, move `with_diagram_id` to the SVG request, remove `with_vendored_text_measurer`, and replace `render_svg_with_pipeline_sync` with the operation above. Keep the host's theme configuration, readability policy, and output checks. The independent feature consumer exercises all 13 families through the resvg-safe pipeline; it does not replace Zed's GPUI, final font, CSS, or raster integration tests.

## Handle unavailable languages and backends

Known identities remain in `diagram_family_capabilities()` even when their parsers are not compiled. Use its implementation flags and `supported_diagrams()` for actual parser availability, not detection or header recognition alone. Strict parsing rejects an unavailable language; lenient parsing can produce an Error model and must not be used as proof that the language is supported. The facade diagnostic suggests `diagram-*` or `all-diagrams` only when its catalog confirms a known built-in parser is absent; the structured code remains `merman.parse.unsupported_diagram`.

A widened core dependency can have more parsers than a locally selected SVG or ASCII renderer. Check output planning and output-local capabilities too; enabling a parser elsewhere is not proof that this output has its family handler. Unknown or custom registry failures retain their original unsupported diagnostic.

Optional graph backends preserve registered fallback behavior. Without compiled ELK, an ELK layout request resolves to the family's fallback, usually Dagre, or the available Mindmap fallback. Without Cytoscape, Mindmap can use Dagre; Architecture's Cytoscape-dependent render path returns a typed missing-capability error. When a compiled requested backend is denied by runtime policy, the result is a typed policy error, not a silent fallback. A generic feature flag is therefore neither a universal layout promise nor a substitute for inspecting the host's required family/backend combination.

## Review presentation and host services

Mermaid 12 changes default layout, theme, and look. To request the earlier presentation choices, set top-level `layout: dagre`, `theme: default`, and `look: classic` through the host's approved site configuration. These choices do not promise byte-identical old SVG; correctness fixes and measurement changes also affect geometry and IDs. Refresh snapshots and selectors against the actual artifact. Runtime configuration does not remove compiled ELK dependencies or their distribution notices.

Text measurement is deterministic and font-agnostic by default. The removed vendored metric tables are not replaced by a guarantee for the host's display font. When the final font stack is known, supply `DeterministicTextMeasurer::with_width_callback(...)` through the text-measurement policy; measure complete candidate strings and let Merman own wrapping. Use a host provider when measurement can fail. The maintained [font integration example](../../crates/merman/examples/render_svg_monospace.rs) demonstrates the current request/environment wiring. Test titles, multiline labels, Unicode, zoom, and the final raster path using the installed font.

The default runtime is deterministic. Enable and select native clock, time-zone, or random adapters only when the host needs them. In particular, date-dependent Gantt output should be tested with an explicit deterministic or native policy. Apply resource limits at the operation boundary and keep host scheduling, document versions, and retained caches in the host. See the [resource-policy host guide](../integration/RESOURCE_POLICY.md), [host integration recipes](../../crates/merman/examples/README.md#host-integration-recipes) and [the symbol migration reference](../release/UNRELEASED_UPGRADE_GUIDE.md) for detailed replacements.
