# ADR 0105: Ergonomic Feature Tiers and Safe High-Level Facade API

- Accepted for the v0.11 development line on 2026-11-26.
- Amends: convenience-function and aggregate-feature clauses of [ADR-0091](0091-selectable-diagram-families.md), [ADR-0098](0098-semantic-presentation-capabilities.md), and the `merman` facade overview in `crates/merman/src/lib.rs`
- Preserves: positive granular leaf features, explicit resource/cancellation safety guarantees, and the expert `Renderer::render(RenderRequest)` control plane

## Context

Merman supports 34 diagram families, 4 layout engines, 5 output formats (SVG, ASCII, PNG, JPEG, PDF), Theme v2, native fonts, math rendering, analysis/fixes, and LSP editor intelligence. While granular positive Cargo features (`diagram-*`, `layout-*`, `svg`, `ascii`, `analysis`, etc.) and explicit `RenderRequest` structs give expert integrators complete control, two real usability barriers have emerged for everyday Rust, editor, and AI/Agent GUI adopters:

1. **All-or-nothing Cargo feature ergonomics**:
   - The `merman` crate's default features (`all-diagrams`, `svg`, `layout-cytoscape`, `layout-dagre`, `layout-elk`) pull all 34 families and three legacy layout engines (including EPL-2.0 ELK), while omitting `layout-shoal`, `ascii`, and `analysis`.
   - Building a lean, modern, MIT/Apache-clean binary (for an editor, Agent GUI, or lightweight service) requires manually enumerating 10–12 individual `diagram-*` features alongside output and layout leaves.
2. **High ceremony for common safe calls**:
   - Because `merman` intentionally avoided free-standing convenience functions, even rendering a single Mermaid snippet to SVG or ASCII requires constructing `Renderer`, `OperationControl`, `SvgRequest`/`AsciiRequest`, `RenderRequest`, and pattern-matching a multi-variant `RenderOutput` enum (`let RenderOutput::Svg(Some(svg)) = ... else { ... }`).
   - Refusing ergonomic typed helpers on `Renderer` does not improve safety—ergonomic helpers can enforce the exact same default bounded `RenderResourcePolicy`, reuse cached engine state, and return strongly typed outputs.

## Decision

### 1. Minimal Orthogonal Cargo Feature Aggregates (Without a Preset Lattice)

Preserve every existing granular leaf feature (`diagram-*`, `svg`, `ascii`, `png`, `jpeg`, `pdf`, `layout-*`, `math`, `analysis`, `fixes`, `editor`, `native-fonts`, `system-fonts`, `system-*`) and continue to reject a combinatorial `preset-*` feature matrix (mixing workflows, runtime policies, and transports).

Add only **two** clean, orthogonal convenience aggregates on `merman` (and matching facade/binding manifests where appropriate):

1. **`core-diagrams`**: Curated set of 12 diagram families for engineering documentation, editors, and agent integrations:
   - `diagram-flowchart`
   - `diagram-sequence`
   - `diagram-class`
   - `diagram-state`
   - `diagram-er`
   - `diagram-gantt`
   - `diagram-mindmap`
   - `diagram-architecture`
   - `diagram-block`
   - `diagram-c4`
   - `diagram-pie`
   - `diagram-xychart`
   - (`all-diagrams` continues to select all 34 built-in families.)
2. **`layout-mermaid-legacy`**: Explicit aggregate for the three historical Mermaid-compatibility layout backends (`layout-dagre`, `layout-elk`, `layout-cytoscape`), cleanly contrasting with modern `layout-shoal`.

This makes the common lean modern integration self-documenting in one line:

```toml
merman = { version = "0.11", default-features = false, features = ["core-diagrams", "svg", "layout-shoal"] }
```

- Existing `default` features and `complete-svg` / `complete-svg-elk` remain backwards-compatible during the `0.11` line; switching `default` toward `layout-shoal` or publishing dedicated `lite` prebuilt packages (`@mermanjs/node-lite`, `@mermanjs/web-lite`) may follow in `0.12` once qualified.
- The aggregates select compilation inputs, not a runtime backend. Each family still needs an available backend compatible with the request. Do not propagate an aggregate into a transport unless it serves a supported source-build workflow.

### 2. Dual-Layer `merman::Renderer` API (Safe Ergonomic Layer + Expert Control Plane)

Keep `Renderer::render(RenderRequest)` as the complete expert control plane (explicit `OperationControl`, custom deadlines, arbitrary targets, admission receipts, and evidence).

Add a **safe, strongly typed ergonomic layer directly on `Renderer`** (and a fluent `RendererBuilder`) so ordinary callers get cache reuse and bounded resource safety with minimal boilerplate:

- **Typed builder configuration** (`Renderer::builder()`):
  - Fluent setters for site configuration, Theme v2 preset and appearance or a compiled `NativeTheme`, default layout backend, `SvgOutputProfile` (see [ADR-0104](0104-paint-scene-ir-and-dual-output-profiles.md)), default resource policies, and optional `SvgEnvironment` or host text measurer. Reuse the existing `InputResourcePolicy` and graphical `RenderResourcePolicy` owners.
- **Strongly typed safe render helpers on `Renderer`**:
  - `#[cfg(feature = "svg")] pub fn render_svg(&self, source: &str) -> Result<SvgOutput, RenderError>` (and `render_svg_with(&self, source: &str, request: SvgRequest, control: OperationControl)` for per-call overrides without matching `RenderOutput`);
  - `#[cfg(feature = "ascii")] pub fn render_ascii(&self, source: &str) -> Result<AsciiOutput, RenderError>` (and `render_ascii_with(...)`);
  - `#[cfg(feature = "png")] pub fn render_png(&self, source: &str) -> Result<RasterOutput, RenderError>`, with the corresponding `RasterOutput` for JPEG and `PdfOutput` for PDF;
  - `default_svg_request()` and `default_ascii_request()` copy the configured target settings for callers supplying their own operation control or target overrides. All typed helpers apply the builder's configured input policy, and an explicit graphical target can impose lower input ceilings. Use `RenderRequest::with_resource_policy` for a per-operation input override through the expert API.
  - Every zero-boilerplate helper delegates directly to the same `Renderer::render` boundary with `OperationControl::new()` and the renderer's bounded input and graphical resource defaults. Passing an unbounded trusted-input policy remains an explicit expert operation.
  - Convenience helpers map an empty result for their target to the existing `RenderError::NoDiagram`. The expert operation keeps its optional-output and diagnostic behavior.
- **No free-standing hidden-global functions**: Helpers live on `Renderer` (not global `merman::render_svg(str)`) so callers naturally reuse compiled engine, config, and theme caches across repeated renders rather than silently rebuilding an engine per call.

### 3. Forward Compatibility for AI & Editor Workflows (v0.12 Seam)

Design `Renderer` and `RendererBuilder` so v0.12 AI/editor workflows attach cleanly without breaking `0.11` signatures:

- Streaming session construction (`renderer.streaming_session(...)`) composing tolerant parsing, `ShoalLayoutFrame` anti-jitter stability, and automatic cancellation of superseded frames;
- Host color bridging (`Theme::from_host_colors(canvas, ink, accent)`) projecting 2–3 host editor/GUI colors into a contrast-verified Theme v2 document;
- Optional LLM syntax auto-repair pre-pass built on `merman-analysis`.

## Alternatives considered

- **Add workflow-specific preset bundles.** This would conflate family selection with outputs, backends, and delivery profiles, especially when callers need to omit a dependency.
- **Add free-standing render functions or replace the expert API.** Free functions obscure per-host state and cache reuse. Replacing the expert API would remove explicit operation controls from existing users.

## Consequences

- New Rust integrators can select a small set of orthogonal features and render with typed methods. Verify the lean Dagre and Shoal compile recipes, bounded rejection, cancellation with explicit controls, and documented examples. Exercise `NoDiagram` separately from parse and resource errors.
- Existing expert callers (`Renderer::render`, `RenderRequest`, `capabilities/feature-surface-v1.json`, `capabilities/artifact-profiles-v2.json`, and all granular Cargo leaf features) remain fully supported with zero semantic weakening.
- Documentation ([`docs/FEATURES.md`](../FEATURES.md) and `crates/merman/src/lib.rs`) gains a clear "Recommended starting points" matrix (Lean Modern / Editor & Agent GUI / Terminal ASCII / Full Mermaid Parity) so users immediately know which combination to pick.
