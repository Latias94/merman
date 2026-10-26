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
    println!("{}", layout.layout());
    Ok(())
}
```

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
    println!("{}", layout.layout());
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
accepts `merman::presentation::PresentationConfig`. Installing a full engine does not change
semantic or ordinary ASCII requests.

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
3. `RenderTarget::LayoutJson` or `RenderTarget::Svg` enters the SVG adapter.
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
Only Kanban supports this mode; the SVG capability plan reports its supported modes in
`supported_layout_adaptation_ids`. Other families reject explicit adaptation, including empty
edge-geometry queries, without starting unused layout services.

Fit-width packs whole columns into rows in source order. It preserves configured section width,
labels, fonts, task occurrences, IDs and ticket links. Labels and column heights are prepared once;
placement uses the operation's existing cancellation and layout-work budget. A column wider than
its available container keeps its natural width and reports overflow. This is placement adaptation,
not a maximum SVG width or a font-scaling policy.

`layout_json` exposes `meta.layout_adaptation`, and completed SVG/native results expose the same
ordinary observation through `layout_adaptation()` and binding metadata `layout_adaptation`:
`mode`, `requested_width`, `actual_width`, and `overflowed`. Actual width includes the existing
viewBox padding, before viewport scaling; an empty board retains its 100px fallback. Native SDKs
preserve this additive metadata in their existing `raw_json` / `rawJson` field.

Kanban retains Mermaid's existing configuration precedence: effective `mindmap.padding` precedes
`kanban.padding`. With the selected defaults (200px columns, 5px gaps, 10px viewBox padding), one
column needs 220px and two need 425px. Explicit `mindmap.padding: 8` gives 216px and 421px.
Default rendering and fit-width requests wide enough for the natural strip produce identical SVG
bytes. Reuse a prepared source to switch widths without reparsing.

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
