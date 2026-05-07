# JSON Clone Audit

Date: 2026-05-08

This note classifies `serde_json::Value` cloning in the layout/render pipeline. The goal is not
"zero clone" everywhere; it is to keep owned JSON only where an API contract requires ownership and
to keep render-only paths typed/config-borrowed.

## Current Shape

- `merman::render::render_svg_sync` uses the typed render path:
  - `Engine::parse_diagram_for_render_model_sync`
  - `layout_parsed_render_layout_only`
  - `render_layout_svg_parts_for_render_model_with_config`
- `layout_parsed` still clones `parsed.model` into `LayoutedDiagram.semantic`.
  This is required by the current compatibility API because `LayoutedDiagram` owns both layout and
  semantic JSON for later `render_layouted_svg` calls.
- `layout_parsed_render_layout_only` does not construct an owned semantic JSON payload. It only
  borrows `parsed.meta.effective_config.as_value()` except for diagram-specific typed layout APIs
  that already take `&MermaidConfig`.

## Clone Taxonomy

| Category | Owner | Status | Decision |
| --- | --- | --- | --- |
| Owned semantic compatibility payload | `layout_parsed` | Intentional | Keep until a future borrowed/typed `LayoutedRenderModel` API replaces this surface. |
| Value-to-`MermaidConfig` bridge in legacy JSON render entrypoints | `render_layout_svg_parts`, `render_layout_svg_parts_for_render_model` | Compatibility cost | Keep for callers that only provide `&Value`; prefer `*_with_config` for public wrappers. |
| Rebuilding `MermaidConfig` inside class typed layout/render paths | class note HTML layout metrics and rendering | Removed for class typed/config path | Pass the existing `&MermaidConfig` through class layout/render state and only allocate on legacy `&Value` entrypoints. |
| Cloning typed render models for local title fallback | sequence SVG rendering | Removed | Keep model borrowed and compute an effective title override at emission time. |

## Changes Made

- Class SVG render-with-config now has a dedicated model entrypoint that keeps
  `&merman_core::MermaidConfig` available through note HTML measurement and sanitization.
- Class layout now has JSON and typed `*_with_config` entrypoints so note HTML layout metrics can
  reuse the parser's existing `MermaidConfig`.
- `render_layout_svg_parts_with_config` and typed render-model dispatch use the class with-config
  path instead of degrading to `effective_config.as_value()`.
- Sequence SVG rendering no longer clones `SequenceDiagramRenderModel` just to fill a fallback
  diagram title; it borrows the model and computes the fallback title lazily.

## Remaining Work

1. Keep `layout_parsed` semantic cloning documented as a compatibility cost until the public API is
   reviewed.
2. Prefer adding `*_with_config` variants when a renderer needs sanitize/url behavior and the caller
   already has `&MermaidConfig`.
3. Audit string cloning in flowchart/class/sequence hot loops after the Value clone cleanup is
   stable. Candidate areas:
   - flowchart self-loop helper edge expansion.
   - sequence message/block label assembly.
   - class namespace and relation lookup keys.
4. Do not replace meaningful typed ownership with borrowed data unless it simplifies the render
   boundary; DOM parity and maintainability remain the first gates.

## Verification

- `cargo fmt --check`
- `cargo check -p merman-render --all-targets --all-features`
