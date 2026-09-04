# Headless Layout

Merman lays out Mermaid `@11.17.2` diagrams in Rust without a browser or JavaScript runtime.
Layout is a target-local phase of one operation-scoped render, not a JSON-first stage that can be
paired with an unrelated semantic model.

## Public API

Most callers should request layout JSON through `Renderer`:

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

When a caller must inspect metadata before choosing an output, prepare a format-neutral semantic
artifact, then consume it into exactly one target:

```rust
use merman::{OperationControl, RenderOutput, RenderTarget, Renderer, SvgRequest};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let artifact = Renderer::new()
        .prepare_semantic("flowchart TD\nA --> B", OperationControl::new())?
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

Opening separate layout and SVG requests intentionally creates separate operations. This keeps
target ownership and resource accounting explicit instead of exposing a reusable SVG-only layout
artifact.

## Typed Ownership

1. `merman-core` detects the diagram and constructs family semantics under one
   `OperationControl` and runtime context.
2. The public facade owns the format-neutral `SemanticArtifact`.
3. `RenderTarget::LayoutJson` or `RenderTarget::Svg` enters the SVG adapter.
4. `merman-render::family::prepare` computes the matching typed layout behind the facade.
5. Layout JSON or SVG is projected from that same target-local artifact.

The opaque artifact makes cross-family combinations unrepresentable. Compatibility semantic or
layout JSON remains an output format; it is not the master built-in render input.

Custom semantic parsers may return a named JSON model, but that model is explicitly non-renderable
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
