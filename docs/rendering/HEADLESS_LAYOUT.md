# Headless Layout

Merman lays out diagrams from the [selected Mermaid baseline](../alignment/README.md#machine-authorities)
in Rust without a browser or JavaScript runtime.
Layout is a target-local phase of one operation-scoped render, not a JSON-first stage that can be
paired with an unrelated semantic model.

## Public API

Most callers should request layout JSON through `Renderer`. Enable `svg` and the required
`diagram-*` families; `svg` also enables the full `presentation` capability:

```rust
use merman::{OperationControl, RenderOutput, RenderRequest, Renderer, SvgRequest};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let output = Renderer::new().render(RenderRequest::layout_json(
        "flowchart TD\nA --> B",
        OperationControl::new(),
        SvgRequest::default(),
    ))?;
    let RenderOutput::LayoutJson(Some(layout)) = output else {
        return Err("no Mermaid diagram detected".into());
    };
    layout.layout().write_json(std::io::stdout().lock())?;
    Ok(())
}
```

The default response has `schema_version: 1`, a `family_id`, the detected `diagram_type`, and
the family-specific `layout` payload. The payload uses the same coordinates, routes, group
origins, and label positions as the `layout` field of the full snapshot. An adapted layout also
reports `layout_adaptation` at the top level.

Use `Renderer::write_layout_json` with a layout JSON request to stream compact or pretty JSON
to an `std::io::Write`. It writes one value when the source contains a diagram. The caller
supplies any trailing newline and flushes the destination. The CLI uses this path for geometry.

Use `RenderRequest::layout_snapshot`, `RenderTarget::LayoutSnapshot`, or `merman-cli layout
--snapshot` to retrieve `meta`, `semantic`, and `layout`. CLI integrations can check
`cli_contract_version` in `merman-cli capabilities --json`. Contract 7 uses `layout` for the
full snapshot. Contract 8 uses `layout --snapshot`. Request the snapshot for `meta.config`,
`meta.effective_config`, or the parse diagnostic when using `--suppress-errors`.
Platform binding layout operations return the full snapshot.

When a caller must inspect full metadata before choosing a graphical output, prepare a
`PresentationArtifact`, then consume it into exactly one target:

```rust
use merman::{OperationControl, RenderOutput, RenderTarget, Renderer, SvgRequest};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let artifact = Renderer::new()
        .prepare_presentation("flowchart TD\nA --> B", OperationControl::new())?
        .ok_or("no Mermaid diagram detected")?;
    assert_eq!(artifact.metadata().diagram_type, "flowchart-v2");

    let output = artifact.render(RenderTarget::LayoutJson(SvgRequest::default()))?;
    let RenderOutput::LayoutJson(Some(layout)) = output else {
        return Err("layout target returned no diagram".into());
    };
    layout.layout().write_json(std::io::stdout().lock())?;
    Ok(())
}
```

`Renderer::parse_semantic()` instead returns a lean `SemanticArtifact` containing
`SemanticMetadata` and a canonical `SemanticDiagramModel`. It does not evaluate visual themes,
retain full presentation configuration, or support promotion to layout or SVG. Ordinary ASCII
requests use this semantic path too. Request a separate full parse from the source when graphical
output is needed. Full metadata is `PresentationMetadata`; the former `Engine` and `ParseMetadata`
aliases are removed in favor of `MermaidEngine` and `PresentationMetadata`.

Configure canonical parsing with `Renderer::with_semantic_parser`. Full configuration and custom
detectors/adapters belong to `with_presentation_engine`; its `MermaidEngine::with_site_config`
accepts `merman::SourcePresentationConfig`. Installing a full engine does not change
semantic or ordinary ASCII requests.

Register a detector with `DetectorRegistry::add_config_fn` to read the effective configuration by
key path and record changes with `DetectorConfig::set`. Later detectors see those assignments,
including assignments equal to the current value. Existing detectors registered through `add_fn`
can still request or replace the full JSON tree. Use the field-based API for plugins that need
only a handful of settings.

Opening separate layout and SVG requests intentionally creates separate operations. This keeps
target ownership and resource accounting explicit instead of exposing a reusable SVG-only layout
artifact.

Current development builds can share immutable parsing between those operations with
`Renderer::prepare_source()` and `render_prepared()`, or transfer a full artifact through
`PresentationArtifact::into_prepared_source()`. Each target still computes its own layout
under fresh control and resource admission. See the
[prepared rendering migration](../migrations/render-reuse-host-integration.md#reuse-parsed-source-in-rust).

## Typed Ownership

1. For a graphical request, `merman-core` detects the diagram and constructs family semantics
   with full presentation metadata under one `OperationControl` and runtime context.
2. The public facade owns the full `PresentationArtifact`.
3. A layout, layout snapshot, or SVG target enters the SVG adapter.
4. `merman-render::family::prepare` borrows the canonical model/context and computes the matching
   owned typed layout behind the facade.
5. Layout JSON or SVG is projected from that same target-local artifact.

The opaque artifact makes cross-family combinations unrepresentable. Compatibility semantic or
layout JSON remains an output format; it is not the master built-in render input. Select
`compat-json` explicitly to call `PresentationArtifact::compatibility_json()`; `svg` alone does
not expose that facade projection. Layout JSON remains an SVG target output.

Full custom parser overlays may return a named JSON model, but that model is explicitly non-renderable
unless a renderer capability is designed and registered separately.

## Render Environment

`SvgRequest.environment` selects text measurement, math and icon services, and SVG resource limits.
`Renderer` owns the operation runtime policy, parser defaults, and input admission. The facade
captures one operation context and passes it into the SVG session; the target adapter does not
create a replacement operation.

Use `RenderEnvironment::deterministic()` for built-in font-agnostic measurement. Host builds can
supply host services explicitly; a successful host measurement bypasses the deterministic fallback
for that request.
`LayoutOptions` contains layout request values and does not own environment services.

## Explicit Kanban Width Adaptation

Set `LayoutOptions.adaptation` to `LayoutAdaptation::FitWidth`, or use binding JSON
`{"layout":{"adaptation":"fit-width","container_width":425}}`. The default is `None` / `"none"`.
The SVG capability plan reports placement modes for the selected family and backend in
`supported_layout_adaptation_ids`. Unsupported requests return a typed error before layout.

Fit-width packs whole columns into rows in source order. It preserves configured section width,
labels, fonts, task occurrences, IDs and ticket links. Labels and column heights are prepared once;
placement uses the operation's existing cancellation and layout-work budget. A column wider than
its available container keeps its natural width and reports overflow. This is placement adaptation,
not a maximum SVG width or a font-scaling policy.

`layout_json` exposes `layout_adaptation` at the top level. Full snapshots expose it under
`meta.layout_adaptation`. Completed SVG/native results expose the same
ordinary observation through `layout_adaptation()` and binding metadata `layout_adaptation`:
`mode`, `requested_width`, `actual_width`, and `overflowed`. Actual width includes the existing
viewBox padding, before viewport scaling; an empty board retains its 100px fallback. Native SDKs
preserve this additive metadata in their existing `raw_json` / `rawJson` field.

Kanban retains Mermaid's existing configuration precedence: effective `mindmap.padding` precedes
`kanban.padding`. With the selected defaults (200px columns, 5px gaps, 10px viewBox padding), one
column needs 220px and two need 425px. Explicit `mindmap.padding: 8` gives 216px and 421px.
Default rendering and fit-width requests wide enough for the natural strip produce identical SVG
bytes. Reuse a prepared source to switch widths without reparsing.

## Shoal Width Adaptation and Preview Frames

Flowchart, Agentflow, Architecture, C4, Class, State, ER, Requirement, Usecase, and Swimlane support `FitWidth`
when their effective configuration selects Shoal. Layered diagrams wrap disconnected components
and try tighter spacing. Architecture wraps disconnected root components while preserving their
four-sided port constraints. C4 wraps rows of cards within its boundaries and reserves the SVG
margin before placing relationship captions. Captions that cannot fit near their shaft without
covering other ink retain an explicit endpoint reference below the drawing. Flowchart and Agentflow
can also turn horizontal flows downward for containers narrower than 640 CSS pixels. Opt into
folding eligible wide leaf fans with
`LayoutOptions::with_fold_wide_ranks(true)` or the binding options below.

```json
{"layout":{"adaptation":"fit-width","container_width":1200,"fold_wide_ranks":true}}
```

The `layout_adaptation` observation reports normalized geometry width before SVG viewBox padding
and scaling. An empty diagram has a geometry width of zero. If the diagram cannot fit, the
observation reports overflow and describes the narrowest applicable result.

Swimlane keeps its common rank grid and lane order while tightening the gaps. Horizontal
lanes can turn downward within the same breakpoint when there is no expanded Pool.
Expanded Pools keep their authored axis and include their outer headings in the reported
width. Block uses the dimensions of its authored grid and selects Shoal for edge routing.

For an interactive preview of Swimlane or a width-fitting Shoal family, call `LayoutOptions::with_shoal_layout_frame`
with the previous successful `ShoalLayoutFrame`, or with `None` to start a session. Read the
next frame from `SvgOutput::shoal_layout_frame()` after successful rendering. Full binding SVG
results expose it as `shoal_layout_frame` metadata. Keep the frame with its document and pass
it back on the next edit. In layered diagrams and Swimlane, Shoal matches surviving node IDs within
their parent and local flow direction, then uses the previous rank and cross-axis order as placement
hints where supported. Architecture and C4 record placements and solve each frame from the current
port constraints or card inventory. Flat layered diagrams
also use component-relative coordinates to reduce movement. In-memory Rust frames can pass reusable
layout storage to the next render when measurements and topology still agree. Constrained renders
and renders with an equally tight layout work limit release their scratch after solving. Serialized
frames carry placement hints only. The frame is separate from layout JSON and SVG.

## Low-Level Use

Direct `merman-render` callers can still use its typed model-level adapter, but they must explicitly
own the controlled parse, `OperationContext`, `OperationControl`, `RenderSession`, and resource
projection. This is a low-level integration boundary, not a second source-to-output facade.

## Compatibility and Verification

- Diagram-family layout algorithms remain family-owned and source-backed.
- Layout JSON snapshots verify the serialized projection.
- SVG parity commands exercise the canonical typed operation rather than rebuilding a legacy
  parse-layout-render chain.
- Browser-dependent text and root residuals are governed by ADR-0057 and ADR-0062; they must not be
  hidden by semantic model distortion or broad comparator normalization.

See ADR-0010 and ADR-0073 for the semantic and render ownership contract.
