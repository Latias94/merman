# Embedding Merman in a Rust application

This guide describes Rust embedding against the current development source. Build against the
same revision as the APIs below; the dependency examples use a sibling checkout, so adjust `path`
to your local source. Use the [0.8.0 upgrade guide](../release/V080_TO_V090_UPGRADE_GUIDE.md) for
published managed JSON and canonical record migrations, the
[versioned upgrade index](../release/README.md) for other releases, and
[package surfaces](../release/PACKAGE_SURFACES.md) to choose a binding or artifact.
Compiled-theme authoring remains experimental and requires a matching artifact.

For current development builds, the [prepared rendering migration](../migrations/render-reuse-host-integration.md)
adds reusable parsed sources, borrowed low-level family preparation and independent geometry
budgets. Those APIs are newer than the published versions described above.

## Select languages, outputs, and layout backends

Choose these independently. `diagram-*` selects built-in languages and their implementations; `svg` selects SVG output; `layout-*` selects optional layout engines and implies `svg`. A layout feature does not select a language, and a language feature does not select an optional layout engine. This keeps parser-only users and applications with small allowlists from acquiring unrelated implementations.

For an SVG host that needs all built-in diagram families and Cytoscape support:

```toml
[dependencies]
merman = { path = "../merman/crates/merman", default-features = false, features = ["all-diagrams", "svg", "layout-cytoscape"] }
```

Default facade builds include all languages, SVG, Cytoscape and ELK. Math is opt-in through `math`; `complete-svg` and `complete-svg-elk` are retired. Disable defaults and select the required family/output/layout leaves when the default exceeds the host's requirements. See [Features](../FEATURES.md) for the complete matrix and distribution notices.

For a host accepting only Flowchart and Sequence, replace `all-diagrams` with `diagram-flowchart, diagram-sequence`. Forward selectors from a reusable library rather than assuming another dependency will enable them. Cargo features are additive: inspect the final application with `cargo tree -e features`, and verify a narrow selection in an independent consumer workspace.

## Embed canonical semantics without presentation

For a host that only inspects Flowchart, Sequence and State, disable defaults and select exactly
those three families:

```toml
[dependencies]
merman = { path = "../merman/crates/merman", default-features = false, features = [
    "diagram-flowchart", "diagram-sequence", "diagram-state",
] }
```

```rust
use merman::{OperationControl, Renderer, SemanticDiagramModel, SemanticParser};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let renderer = Renderer::new().with_semantic_parser(SemanticParser::new());
    for source in [
        "flowchart TD\nA --> B",
        "sequenceDiagram\nAlice->>Bob: Hello",
        "stateDiagram-v2\n[*] --> Ready",
    ] {
        let artifact = renderer
            .parse_semantic(source, OperationControl::new())?
            .ok_or("no Mermaid diagram detected")?;
        match artifact.model() {
            SemanticDiagramModel::Flowchart(_) => println!("Flowchart"),
            SemanticDiagramModel::Sequence(_) => println!("Sequence"),
            SemanticDiagramModel::State(_) => println!("State"),
            _ => return Err("unexpected semantic family".into()),
        }
        println!("diagram type: {}", artifact.metadata().diagram_type);
    }
    Ok(())
}
```

`Renderer::parse_semantic()` and `RenderRequest::semantic()` yield a lean `SemanticArtifact`
with `SemanticMetadata` and a canonical `SemanticDiagramModel`. `SemanticParser` is also available
directly through `merman-core` with the same family selectors. Configure semantic and ordinary
ASCII requests with `with_semantic_parser(SemanticParser::new().with_site_config(config))`, where
`config` is a `MermaidConfig`.

Full Mermaid processing is a separate capability. `presentation` exposes `MermaidEngine`,
`PresentationMetadata` and `PresentationArtifact`; `svg` enables it automatically. Use
`Renderer::prepare_presentation()` or `RenderRequest::presentation()` when an operation needs
effective presentation configuration or a later graphical target. A lean semantic artifact has
no compatibility JSON or promotion to a graphical artifact; request a separate full parse from
the source for graphical output. `PresentationArtifact::into_prepared_source()` retains a full
parse for independently controlled redraws.

Select `compat-json` explicitly for `MermaidEngine`'s Mermaid-compatible JSON APIs or
`PresentationArtifact::compatibility_json()`. It implies `presentation`; `svg` alone does not
expose the facade's compatibility JSON projection. The old `Engine` and `ParseMetadata` aliases
are removed; use `MermaidEngine` and `PresentationMetadata` for their full counterparts.

Semantic and ordinary ASCII operations do not evaluate visual theme programs, even when
presentation is compiled. Source-config admission, semantic configuration and sanitization still
apply. An admitted color that fails visual theme evaluation can therefore succeed for semantic
inspection or ASCII while full presentation returns the theme error.

## Replace the previous renderer

The former `HeadlessRenderer` and its source-to-SVG methods are replaced by `Renderer::render`. Keep reusable engine configuration on the renderer and per-output settings on `SvgRequest`:

```rust
use merman::presentation::PresentationConfig;
use merman::svg::{CssOverridePostprocessor, SvgPipeline};
use merman::{
    MermaidEngine, OperationControl, RenderOutput, RenderRequest, Renderer, SvgRequest,
};

fn render_for_host(
    source: &str,
    config: PresentationConfig,
    diagram_id: String,
) -> Result<String, Box<dyn std::error::Error>> {
    let renderer = Renderer::new()
        .with_presentation_engine(MermaidEngine::new().with_site_config(config));
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

`MermaidEngine::with_site_config` requires `merman::presentation::PresentationConfig`. Wrap an
existing `MermaidConfig` with `PresentationConfig::from_config(config)`, or construct the full
config with `PresentationConfig::from_value(value)`. `with_presentation_engine` installs full
configuration, detectors and custom adapters independently of `with_semantic_parser`; it does
not replace the parser used by semantic or ordinary ASCII requests.

Reuse the configured `Renderer` across calls when site configuration and defaults are shared; construct a fresh request and operation control for each operation. Clone the operation control for the task that cancels it. Return or classify parse errors, cancellation, resource exhaustion, and missing capabilities separately. A successful `None` means no diagram was detected, not that a failed operation was ignored.

`SvgPipeline::resvg_safe()` prepares SVG for the supported resvg/usvg path, including conversion of HTML labels. The CSS override postprocessor is useful when the host deliberately supplies final styles; omit it when those overrides are not part of the integration. Give co-located SVGs IDs that remain unique after `sanitize_svg_id()` normalization. A resvg-compatible artifact does not authorize arbitrary browser navigation or host DOM insertion; browser hosts should use the owning browser admission APIs.

## Redraw unchanged source in development builds

Use `Renderer::prepare_source(PrepareSourceRequest::new(source, control))` once and
`Renderer::render_prepared(&prepared, target, fresh_control)` for each output. The
[complete example](../../crates/merman/examples/redraw_prepared.rs) changes layout width without
reparsing. Preparation freezes semantics, effective configuration and parse-time dates/IDs; prepare
again when those inputs must change. Each output has fresh cancellation, input admission, layout
work and output runtime state. A failed redraw does not poison another redraw.

`prepare_source` uses the full presentation path and requires `presentation`. To retain an
already prepared full artifact, call `PresentationArtifact::into_prepared_source()`.

Changing `SvgRequest.layout` or measurement services starts another layout; a prepared source is
not a layout cache. Completed results own their public data after the snapshot is dropped.
Use `RenderedDocument` instead when reusing one completed SVG across graphical exports.
The [migration reference](../migrations/render-reuse-host-integration.md#reuse-parsed-source-in-rust)
also covers `prepare(&parsed, ...)`, artifact lifetimes and the removed arbitrary-JSON layout handoff.

## Example: an editor with a family allowlist

The reviewed Zed reference snapshot pins `0.8.0-alpha.5` and selects `svg, layout-cytoscape` with defaults disabled. Updating that declaration alone compiles on alpha.7, but strict parsing fails because no built-in family is selected. Add `all-diagrams` to preserve the former language surface, or select the families from the host's own allowlist:

```toml
merman = { path = "../merman/crates/merman", default-features = false, features = [
    "svg", "layout-cytoscape",
    "diagram-flowchart", "diagram-sequence", "diagram-class", "diagram-state",
    "diagram-er", "diagram-gantt", "diagram-pie", "diagram-git-graph",
    "diagram-mindmap", "diagram-timeline", "diagram-quadrant-chart",
    "diagram-xychart", "diagram-journey",
] }
```

These 13 logical families cover that reference snapshot's allowlist; `graph`, State header variants, and `xychart-beta` use the selector of their owning family. The all-family recipe retains parser availability beyond that allowlist; the subset recipe deliberately rejects other languages. Neither declaration adds ELK or math. Review newly accepted languages separately if the host later expands its allowlist.

Migrate the alpha.5 API as well: move `with_site_config` to `MermaidEngine` with a
`PresentationConfig`, install it through `Renderer::with_presentation_engine`, move
`with_diagram_id` to the SVG request, remove `with_vendored_text_measurer`, and replace
`render_svg_with_pipeline_sync` with the operation above. Keep the host's theme configuration,
readability policy, and output checks. The independent feature consumer exercises all 13 families
through the resvg-safe pipeline; it does not replace Zed's GPUI, final font, CSS, or raster
integration tests.

Parser-backed editor and analysis defaults use `SemanticParser` and do not evaluate visual
themes. They no longer report visual theme color or theme-evaluation-limit diagnostics by
default. Enabling presentation or compatibility JSON does not change that default. Hosts that
need visual-theme validation must request full presentation explicitly; a successful editor
snapshot does not establish that the graphical preview will succeed.

## Handle unavailable languages and backends

Known identities remain in `diagram_family_capabilities()` even when their parsers are not compiled. Use its implementation flags and `supported_diagrams()` for actual parser availability, not detection or header recognition alone. Strict parsing rejects an unavailable language; lenient parsing can produce an Error model and must not be used as proof that the language is supported. The facade diagnostic suggests `diagram-*` or `all-diagrams` only when its catalog confirms a known built-in parser is absent; the structured code remains `merman.parse.unsupported_diagram`.

A widened core dependency can have more parsers than a locally selected SVG or ASCII renderer. Check output planning and output-local capabilities too; enabling a parser elsewhere is not proof that this output has its family handler. Unknown or custom registry failures retain their original unsupported diagnostic.

Optional graph backends preserve registered fallback behavior. Without compiled ELK, an ELK layout request resolves to the family's fallback, usually Dagre, or the available Mindmap fallback. Without Cytoscape, Mindmap can use Dagre; Architecture's Cytoscape-dependent render path returns a typed missing-capability error. When a compiled requested backend is denied by runtime policy, the result is a typed policy error, not a silent fallback. A generic feature flag is therefore neither a universal layout promise nor a substitute for inspecting the host's required family/backend combination.

## Review presentation and host services

Core SVG leaves the background unset; set `SvgOutputPolicy.root_background_color` when opaque artwork is required. Optional `native-fonts` supports a reusable `NativeFontContext` through `RenderEnvironment::with_native_font_context`; the facade forks operation-local measurement state for each render while retaining the selected context for measurement/painting identity checks. Ordinary Rust defaults remain deterministic.

For Mermaid-compatible semantic JSON, enable `compat-json` and retain the returned
`ManagedSemanticJson` owner. Retain managed layout results too, using their maintained JSON
writers rather than raw recursive serde. Canonical semantic inspection exposes typed models.

Mermaid 12 changes default layout, theme, and look. To request the earlier presentation choices, set top-level `layout: dagre`, `theme: default`, and `look: classic` through the host's approved site configuration. These choices do not promise byte-identical old SVG; correctness fixes and measurement changes also affect geometry and IDs. Refresh snapshots and selectors against the actual artifact. Runtime configuration does not remove compiled ELK dependencies or their distribution notices.

Text measurement is deterministic and font-agnostic by default. The removed vendored metric tables are not replaced by a guarantee for the host's display font. When the final font stack is known, supply `DeterministicTextMeasurer::with_width_callback(...)` through the text-measurement policy; measure complete candidate strings and let Merman own wrapping. Use a host provider when measurement can fail. The maintained [font integration example](../../crates/merman/examples/render_svg_monospace.rs) demonstrates the current request/environment wiring. Test titles, multiline labels, Unicode, zoom, and the final raster path using the installed font.

The default runtime is deterministic. Enable and select native clock, time-zone, or random adapters only when the host needs them. In particular, date-dependent Gantt output should be tested with an explicit deterministic or native policy. Apply resource limits at the operation boundary and keep host scheduling, document versions, and retained caches in the host. See the [resource-policy host guide](../integration/RESOURCE_POLICY.md), [host integration recipes](../../crates/merman/examples/README.md#host-integration-recipes) and [the symbol migration reference](../release/UNRELEASED_UPGRADE_GUIDE.md) for detailed replacements.
