# Embedding Merman in a Rust application

This guide targets `0.11.0`. Match coupled packages to that release. The dependency examples use a sibling checkout, so adjust `path`
to your local source. Use the [0.8.0 to 0.9.0 upgrade guide](../release/V080_TO_V090_UPGRADE_GUIDE.md) for
published managed JSON and canonical record migrations, the
[versioned upgrade index](../release/README.md) for other releases, and
[package surfaces](../release/PACKAGE_SURFACES.md) to choose a binding or artifact.
For native preset selection, editable documents, and per-render theme outcomes, see the
[Theme v2 guide](custom-diagram-themes.md).

The [0.10 prepared-rendering migration](../migrations/render-reuse-host-integration.md) covers reusable parsed sources, borrowed low-level family preparation and independent geometry budgets when upgrading from `0.9.x`.

## Select languages, outputs, and layout backends

Choose these independently. `diagram-*` selects built-in languages and their implementations; `svg` selects SVG output; `layout-*` selects optional layout engines and implies `svg`. A layout feature does not select a language, and a language feature does not select an optional layout engine. This keeps parser-only users and applications with small allowlists from acquiring unrelated implementations.

For an SVG host that needs all built-in diagram families and the Dagre and Cytoscape adapters, use this dependency.

```toml
[dependencies]
merman = { path = "../merman/crates/merman", default-features = false, features = ["all-diagrams", "svg", "layout-cytoscape", "layout-dagre"] }
```

Default facade builds include all languages, SVG, Dagre, ELK, and Cytoscape. Math is opt-in through `math`. `complete-svg` selects SVG, Dagre, and Cytoscape, and `complete-svg-elk` adds ELK. Select math and diagram families independently of these aggregates. Add `math` explicitly when upgrading a previously math-enabled aggregate selection. Disable defaults and select the required families plus an aggregate or individual output/layout leaves when the default exceeds the host's requirements. See [Features](../FEATURES.md) for the complete matrix and distribution notices.

For a host accepting only Flowchart and Sequence, replace `all-diagrams` with `diagram-flowchart, diagram-sequence`. Forward selectors from a reusable library rather than assuming another dependency will enable them. Cargo features are additive: inspect the final application with `cargo tree -e features`, and verify a narrow selection in an independent consumer workspace.

## Set an application layout default

This development-build example enables Shoal as the only graph engine for Flowchart SVG.

```toml
[dependencies]
merman = { path = "../merman/crates/merman", default-features = false, features = ["svg", "diagram-flowchart", "layout-shoal"] }
```

```rust
use merman::{LayoutBackend, Renderer};

let renderer = Renderer::builder()
    .with_default_layout(LayoutBackend::Shoal)
    .build()?;
let svg = renderer.render_svg("flowchart TD\nA[Start] --> B[Done]")?;
println!("{}", svg.svg());
```

`with_default_layout` sets the renderer's top-level site default. Use
`with_site_config` with a `MermaidConfig` to set a family-specific default such as
`flowchart.layout`. Document frontmatter and directives still follow Mermaid's configuration
precedence. A document override can request a different engine and will need that engine's feature.
In a Shoal-only build, set `layout: tidy-tree` for Mindmap rather than relying on the global
Shoal default. Mindmap does not use Shoal.

The one-argument helpers use bounded defaults for SVG, ASCII, PNG, JPEG, and PDF. They return
`RenderError::NoDiagram` when detection finds no diagram, without hiding errors in a recognized
diagram. For per-call limits and cancellation, use the relevant `*_with` method with an explicit
target request and `OperationControl`, or use `Renderer::render(RenderRequest)` to retain the
optional-output contract.
Start from `renderer.default_svg_request()` or `renderer.default_ascii_request()` to keep the
builder's target settings when you supply a control or per-call override. A fresh `SvgRequest`
or `AsciiRequest` takes its own target defaults. Both ASCII helpers use the builder's configured
input policy. Use `Renderer::render` with `RenderRequest::with_resource_policy` to choose an
input policy for a specific operation.

For a defaults-off build that previously depended on Dagre, add `layout-dagre`. With a host allowlist,
include `RenderCapabilityPolicy::deny_all().with_allowed(RenderCapability::LayoutDagre)` in the
policy installed through `SvgEnvironment::with_capability_policy`, along with any other capabilities
your application needs. [Feature recipes](../FEATURES.md#svg-without-general-purpose-layout-engines)

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
use merman::model::SemanticDiagramModel;
use merman::{OperationControl, Renderer, SemanticParser};

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
use merman::SourcePresentationConfig;
use merman::svg::{CssOverridePostprocessor, SvgPipeline};
use merman::{
    MermaidEngine, OperationControl, RenderOutput, RenderRequest, Renderer, SvgRequest,
};

fn render_for_host(
    source: &str,
    config: SourcePresentationConfig,
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

`MermaidEngine::with_site_config` requires `merman::SourcePresentationConfig`. Wrap an
existing `MermaidConfig` with `SourcePresentationConfig::from_config(config)`, or construct the full
config with `SourcePresentationConfig::from_value(value)`. `with_presentation_engine` installs full
configuration, detectors and custom adapters independently of `with_semantic_parser`; it does
not replace the parser used by semantic or ordinary ASCII requests.

`MermaidConfig::as_value` and the config views available to custom detectors use the host's
`serde_json::Value` object order. Merman keeps authored order in the typed fields that use it for
rendering. For a programmatically built Usecase table, populate `UsecaseJsonNode::property_order`
to control the order of object rows.

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

## Consume Sequence drawing geometry in development builds

`RenderRequest::layout_json(...)` and a prepared source's `RenderTarget::LayoutJson` expose the
same owned layout projection. Current development builds extend `layout.SequenceDiagram` with
the geometry a native painter needs in addition to participant nodes, message edges, and lifelines:

| Field | Geometry |
| --- | --- |
| `frames` | Control-frame rectangles, control kinds and labels, and ordered section dividers for `else`, `and`, and `option`. |
| `activations` | Activation rectangles associated with participant IDs and nesting depth. |
| `boxes` | Participant-group rectangles and positioned group titles. |
| `title` | The positioned diagram title, when present. |

These positions come from the same layout preparation consumed by the SVG renderer. A host can
paint them directly without replaying the semantic message stream to reconstruct placement.
Coordinates use the existing participant/message layout space; retain negative coordinates and
apply the enclosing output viewport consistently. Text positions retain their documented anchor
conventions rather than representing glyph bounding boxes: box titles use a centered anchor,
while the diagram title uses a start anchor and alphabetic baseline. Rectangle coordinates are
top-left positions; section `y` values identify the divider strokes.

The added fields are omitted when empty or absent. Existing `nodes`, `edges`, `clusters`, and
`bounds` retain their meaning. Native hosts still own paint and must measure using the font stack
they will draw; geometric export does not embed fonts or reproduce a host's text shaping.
Rust consumers of the former `SequenceBlockLayout` and `block_layouts_by_id` should use
`SequenceDiagramLayout.frames` and each frame's `sections`. The cursor-replay intermediate type
is now private; the exported frame geometry includes its final keyword-width and critical-section
adjustments. Direct `SequenceDiagramLayout` literals must also initialize the new drawing fields.
See [`inspect_layout.rs`](../../crates/merman/examples/inspect_layout.rs) for the public request and
managed JSON writer, and the [prepared-source migration](../migrations/render-reuse-host-integration.md)
for source reuse and independent operation control.

## Example: an editor with a family allowlist

The reviewed Zed reference snapshot pins `0.8.0-alpha.5` and selects `svg, layout-cytoscape` with defaults disabled. Updating that declaration alone compiles on alpha.7, but strict parsing fails because no built-in family is selected. Add `all-diagrams` to preserve the former language surface, or select the families from the host's own allowlist:

```toml
merman = { path = "../merman/crates/merman", default-features = false, features = [
    "svg", "layout-cytoscape", "layout-dagre",
    "diagram-flowchart", "diagram-sequence", "diagram-class", "diagram-state",
    "diagram-er", "diagram-gantt", "diagram-pie", "diagram-git-graph",
    "diagram-mindmap", "diagram-timeline", "diagram-quadrant-chart",
    "diagram-xychart", "diagram-journey",
] }
```

These 13 logical families cover that reference snapshot's allowlist; `graph`, State header variants, and `xychart-beta` use the selector of their owning family. The all-family recipe retains parser availability beyond that allowlist; the subset recipe deliberately rejects other languages. Neither declaration adds ELK or math. Review newly accepted languages separately if the host later expands its allowlist.

Migrate the alpha.5 API as well: move `with_site_config` to `MermaidEngine` with a
`SourcePresentationConfig`, install it through `Renderer::with_presentation_engine`, move
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

For ordinary graph diagrams, a missing ELK loader falls back to Dagre when `layout-dagre` is compiled. If neither is available, an ELK request reports missing `layout-elk`. Mindmap uses its own registered fallback chain through Cytoscape and Dagre. Architecture requires Cytoscape for its default layout and `layout-shoal` when selected explicitly. The SVG and layout JSON operations report a typed missing-capability error for an unavailable or denied backend before measuring labels. Inspect the SVG render plan for the required and missing IDs.

With `layout-shoal`, a Rust host that repeatedly lays out flat or compound layered graphs can
clone an environment created with `SvgEnvironment::with_shoal_scratch_reuse`. This applies to
natural-width and `FitWidth` layouts. The environment and its clones share one idle slot for up
to 64 KiB of empty row storage. Output geometry, cancellation, and resource accounting belong to
each operation. Preview frames keep their own storage. Scratch reuse is off by default.

## Review presentation and host services

Core SVG leaves the background unset. Set `SvgOutputPolicy.root_background_color` for opaque artwork.

With `native-fonts`, install a reusable `NativeFontContext` through
`SvgEnvironment::with_native_font_context`. `Renderer` creates a font context for each layout.
In development builds, that context retains the system faces selected during the operation.
To paint with those selections, use `RenderRequest::document` and call `export_png`, `export_jpeg`,
or `export_pdf` on the resulting `RenderedDocument` with the corresponding codec feature enabled.

An external painter that calls `apply_to_usvg_options` on the original context can query system
fonts again, even with the same `usvg` version. For independent SVG parsing, give layout and
painting the same explicit font bytes and family stack, as in the example below.
[Font selection and fallback](../../crates/merman-export/README.md#selection-and-fallback-order)

For Mermaid-compatible semantic JSON, enable `compat-json` and retain the returned
`ManagedSemanticJson` owner. Retain managed layout results too, using their maintained JSON
writers rather than raw recursive serde. Canonical semantic inspection exposes typed models.

Mermaid 12 changes default layout, theme, and look. To request the earlier presentation choices, set top-level `layout: dagre`, `theme: default`, and `look: classic` through the host's approved site configuration. These choices do not promise byte-identical old SVG; correctness fixes and measurement changes also affect geometry and IDs. Refresh snapshots and selectors against the actual artifact. Runtime configuration does not remove compiled ELK dependencies or their distribution notices.

Default SVG measurement is deterministic and font-agnostic. For applications that measure in their display toolkit, `DeterministicTextMeasurer::with_width_callback(...)` accepts widths of complete candidate strings and lets Merman own wrapping. Use a host provider when measurement can fail. Test titles, multiline labels, Unicode and zoom in the final display stack. See the [monospace callback example](../../crates/merman/examples/render_svg_monospace.rs).

The default runtime is deterministic. Enable and select native clock, time-zone, or random adapters only when the host needs them. In particular, date-dependent Gantt output should be tested with an explicit deterministic or native policy. Apply resource limits at the operation boundary and keep host scheduling, document versions, and retained caches in the host. See the [resource-policy host guide](../integration/RESOURCE_POLICY.md), [host integration recipes](../../crates/merman/examples/README.md#host-integration-recipes) and [the symbol migration reference](../release/UNRELEASED_UPGRADE_GUIDE.md) for detailed replacements.

### Match standalone SVG labels to resvg fonts

`SvgPipeline::resvg_safe()` preserves the source label viewport when converting HTML to SVG text.
The default estimate for the 14 px bold Class title `Animal` in issue #331 was 39.2 px, while
native measurement with Arial Bold gave 46.68 px. A downstream painter that chooses the wider
face can clip the last letter. State transition labels and ER entity, attribute and relationship
labels use the same viewport mechanism. [Issue #331](https://github.com/Latias94/merman/issues/331)

When your application controls both stages, pass the font files to Merman **before layout** and
register the same faces in the external painter. Set Mermaid's `fontFamily` using the family
names stored in those files. If your theme sets `themeVariables.fontFamily`, use the same stack
there. Class and ER use the top-level setting for some width probes and the theme setting for
displayed labels. The example sets both explicitly.
For example, a Fontsource stylesheet can use `Recursive Variable`, while different Recursive files
register as `Recursive` or `Recursive Sans Linear Light`. A resvg process with system fonts can then
continue down the stack to Arial. Check the family in `fontdb::FaceInfo::families`
and include the weights, styles and glyph coverage the diagram uses. For WOFF or WOFF2 web
assets, obtain the corresponding native TTF or OTF files.

Run the example from the `0.11.0` checkout with the included Cousine regular and bold faces.
[Standalone resvg 0.50.0 example](../../crates/merman/examples/standalone_resvg/src/main.rs)

```sh
cargo run --manifest-path crates/merman/examples/standalone_resvg/Cargo.toml --target-dir target -- target/native-labels crates/merman-export/tests/fonts/Cousine-Regular.ttf crates/merman-export/tests/fonts/Cousine-Bold.ttf
```

The example writes `class.svg` and `class.png`. It reads each file once, passes identical bytes to
`NativeFontContext::from_font_bytes` and resvg, and takes the family names from resvg's font database.
Put a text family that covers the labels first. The example reports missing coverage or a glyph
that would require independent fallback. When a subset and a full face share the same family,
style and weight, supply the full face once. A TTC can already contain a face supplied as a TTF.
For path-based loading, `NativeFontOptions::with_font_file` also accepts font collections.
An independent resvg consumer parses the standalone SVG and registers the font bytes in its own
database. SVG references fonts by name.

In development builds after `0.11.0`, hosts that use Merman's parser can import `usvg` from
`merman_export::usvg` or `merman::svg::export::usvg`. Call
`fonts.apply_to_usvg_options(&mut options)` with that parser's `Options` before
`usvg::Tree::from_str`. The re-export shares the `merman-usvg` package identity with the context
and exporters. With published `0.11.0`, the equivalent in-process API uses the upstream `usvg`
`0.47` dependency.

When `svg.evidence().native_font_diagnostics()` returns `Some(diagnostics)`, compare the PostScript
names in `diagnostics.selected_faces()` with the faces of the positioned glyphs in the consumer's
`usvg::Tree`. In development builds, `PostScriptName#N` gives the face index `N` in the font bytes
used for measurement. For example, a standalone snapshot can report `PingFangSC-Regular#0` for a
face at index 3 in the installed TTC. Compare indices only when both stages use identical font bytes.

For the bold Class title, the `0.11.0` example and resvg 0.50 request `wght=700`. For CSS normal,
the `0.11.0` example's upstream usvg 0.47 uses the font's default axis position, while resvg 0.50
and Merman's development renderer request `wght=400` when no native instance supplies a weight.
Separate consumers can select different fonts, fallbacks, or OpenType coordinates. Supply the same
explicit static Regular and Bold faces to keep those choices fixed across applications. With the
weight-only Recursive file, FontTools can produce both instances from the same source.
[Merman font instance handling](../../vendor/usvg/UPSTREAM.md)

```sh
fonttools varLib.instancer Recursive.ttf wght=400 -o Regular.ttf
fonttools varLib.instancer Recursive.ttf wght=700 -o Bold.ttf
```

Even with a matching face, italic or overhanging glyphs can extend past their text advance.
The source viewport still clips ink that crosses its boundary.
