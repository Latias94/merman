# Headless Layout

Merman lays out Mermaid `@11.16.1` diagrams in Rust without a browser or JavaScript runtime. Layout
is part of the canonical typed headless operation, not a JSON-first stage that can be paired with an
unrelated semantic model.

## Public API

Most callers should use `merman::svg::HeadlessRenderer`:

```rust
use merman::svg::{HeadlessRenderer, RenderEnvironment};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let renderer = HeadlessRenderer::new()
        .with_environment(RenderEnvironment::parity())
        .with_strict_parsing();

    let layout = renderer
        .layout_json_sync("flowchart TD\nA --> B")?
        .expect("diagram detected");
    println!("{layout}");
    Ok(())
}
```

When one caller needs layout JSON and SVG from the same operation, use the consuming prepared stage:

```rust
use merman::svg::HeadlessRenderer;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let renderer = HeadlessRenderer::new().with_diagram_id("layout-example");
    let prepared = renderer
        .prepare_render_sync("flowchart TD\nA --> B")?
        .expect("diagram detected");

    let layout = prepared.layout_json()?;
    let svg = prepared.render_svg(renderer.svg_options())?;
    assert_eq!(layout["meta"]["diagram_type"], "flowchart-v2");
    println!("{svg}");
    Ok(())
}
```

The free `layout_json_sync` helper delegates to the same operation. The removed
`layout_parsed*`/`LayoutedDiagram` APIs are not compatibility entry points.

## Typed Ownership

1. `merman-core` detects the diagram and constructs family semantics once.
2. The family projects its typed render model.
3. `merman-render::family::prepare` computes the matching typed layout and returns an opaque
   `FamilyRenderArtifact`.
4. `FamilyRenderArtifact::layout_json` projects compatibility layout JSON.
5. `FamilyRenderArtifact::render_svg` consumes the same semantic/layout pair.

The opaque artifact makes cross-family combinations unrepresentable. Compatibility semantic or
layout JSON remains an output format; it is not the master built-in render input.

Custom semantic parsers may return a named JSON model, but that model is explicitly non-renderable
unless a renderer capability is designed and registered separately.

## Render Environment

`RenderEnvironment` selects text measurement, math and icon services, time, randomness, and
resource limits before the operation begins. `begin_session()` freezes those choices once. Layout
uses named measurement phases rather than constructing a family-local production measurer.

Use `RenderEnvironment::parity()` for deterministic vendored measurement and pinned operation
policy. Host builds can supply host services explicitly; a successful host measurement bypasses
vendored fallback facts. `LayoutOptions` contains layout request values; it does not own environment
services.

## Low-Level Use

Direct `merman-render` callers can use this sequence:

1. `Engine::parse_diagram_for_render_model_sync`
2. `RenderEnvironment::begin_session`
3. `merman_render::family::prepare`
4. `FamilyRenderArtifact::layout_json` and optionally consuming `render_svg`

The session must travel with the artifact so layout, SVG, postprocessing, and operation reporting
observe the same policy snapshot.

## Compatibility and Verification

- Diagram-family layout algorithms remain family-owned and source-backed.
- Layout JSON snapshots verify the serialized projection.
- SVG parity commands exercise the canonical typed operation rather than rebuilding a legacy
  parse-layout-render chain.
- Browser-dependent text and root residuals are governed by ADR-0057 and ADR-0062; they must not be
  hidden by semantic model distortion or broad comparator normalization.

See ADR-0010 and ADR-0073 for the semantic and render ownership contract.
