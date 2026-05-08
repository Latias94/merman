# Fearless Refactor Changelog

This log records completed changes that materially advance the fearless-refactor workstream.
Detailed planning remains in `TODO.md` and `MILESTONES.md`.

## 2026-05-08

- Simplified class layout namespace lookup by precomputing namespace parent/child pairs once per
  render pass and reusing the namespace declaration order vector across graph setup and cluster
  emission.
- Added `class_namespace_dense` to the pipeline benchmark fixture set and recorded the baseline in
  `docs/performance/spotcheck_2026-05-08_class_namespace_dense_layout.md`.
- Moved `c4` from JSON-fallback rendering to `C4DiagramRenderModel`.
- Removed the render-side C4 JSON transport structs; JSON layout compatibility now deserializes
  into the shared core render model before using the typed layout and SVG paths.
- Routed public `merman::render::render_svg_sync` C4 rendering through the typed render model and
  layout-only SVG emission.
- Added typed-model and public-render regression coverage for C4.
- Recorded the C4 typed render-path spotcheck in
  `docs/performance/spotcheck_2026-05-08_c4_typed_render_model.md`.
- Moved `xychart` from JSON-fallback rendering to `XyChartDiagramRenderModel`.
- Removed the render-side xychart JSON transport structs; JSON layout compatibility now deserializes
  into the shared core render model before using the typed layout path.
- Routed public `merman::render::render_svg_sync` xychart rendering through the typed render model
  and layout-only SVG emission.
- Added typed-model and public-render regression coverage for xychart.
- Recorded the xychart typed render-path spotcheck in
  `docs/performance/spotcheck_2026-05-08_xychart_typed_render_model.md`.
