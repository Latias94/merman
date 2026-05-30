# Public Render API and CLI Review

Date: 2026-05-08

This note records the P3 review after the sequence, kanban, and gantt typed render migrations.

## Current Public Surface

- `parse_diagram_sync`: stable semantic JSON API. Keep it as the compatibility boundary for callers
  that inspect Mermaid DB-shaped output.
- `parse_diagram_for_render_model_sync`: render-optimized API. This is now the internal default for
  headless SVG rendering and should receive new typed render models.
- `layout_diagram_sync`: stable layout JSON helper. It intentionally keeps the semantic JSON
  payload because `LayoutedDiagram` exposes both semantic and layout data.
- `render_svg_sync`: default public SVG helper. It already uses `parse_diagram_for_render_model_sync`
  and avoids semantic JSON transport for typed-first diagrams.
- `HeadlessRenderer`: ergonomic wrapper around the same sync helpers. Keep it as the integration
  surface for users who want reusable config.
- `raster::*`: thin SVG-to-PNG/JPG/PDF helpers plus render-and-raster helpers. Keep rasterization
  separate from SVG rendering so non-raster users do not pay those dependencies.

## Decisions

- Keep synchronous executor-free helpers as the primary API. Rendering is CPU-bound and does not
  need an async runtime.
- Keep async wrappers as simple aliases for now. They do not introduce runtime dependencies, and
  removing them before a public migration guide would create churn without deleting real code.
- Keep `layout_diagram_sync` on semantic JSON for now. Its return type exposes semantic JSON, so
  moving it to typed render models would either clone JSON later or change the public contract.
- Do not introduce a larger public `RenderRequest` yet. `HeadlessRenderer` already covers repeated
  options for library callers.
- For the CLI, prefer small internal helpers before a request object. The immediate duplication was
  layout-option construction and SVG raster output handling, and both are now shared.
- Keep math rendering opt-in. `--math-renderer ratex` is feature-gated behind `ratex-math`, while
  the default CLI path remains dependency-light and declines math rendering unless explicitly
  requested.

## Cleanup Completed

- CLI layout commands and render commands now share `layout_options`.
- CLI Mermaid-input raster output and direct SVG-input raster output now share `write_rasterized_svg`.
- CLI raster options and default output extension selection now have one owner.
- CLI math renderer selection now has one feature-gated owner and feeds both `LayoutOptions` and
  `SvgRenderOptions`, avoiding measurement/render drift.
- The obsolete `merman_render::svg::render_layout_svg_parts_for_render_model` compat dispatcher was
  removed; render-model SVG dispatch now uses the `*_with_config` surface so callers do not rebuild
  `MermaidConfig` from JSON.

## Follow-up

- Revisit async wrappers near the next public release boundary.
- Consider an internal CLI request object only if more command modes start sharing validation and
  output routing.
- Keep new diagram render work on `parse_diagram_for_render_model_sync`; avoid adding new semantic
  JSON render-only shims.
