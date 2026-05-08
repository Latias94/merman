# Fearless Refactor Changelog

This log records completed changes that materially advance the fearless-refactor workstream.
Detailed planning remains in `TODO.md` and `MILESTONES.md`.

## 2026-05-08

- Expanded `xtask report-overrides` to inventory hand-authored `maybe_override_*` raw SVG/path
  bridge functions under `svg/parity`, with stable `/` paths in report output.
- Fixed override helper-function counting in `xtask report-overrides` and added regression tests
  for helper and manual bridge detection.
- Documented the current flowchart degenerate path bridge with owner/removal criteria and refreshed
  `OVERRIDE_FOOTPRINT.md` for the generated-plus-manual report snapshot.
- Replaced sequence parity renderer long-argument helpers with focused render contexts and removed
  the sequence module-level `clippy::too_many_arguments` allow while keeping sequence DOM parity
  green.
- Structured SVG path-bounds cubic/arc inputs and removed the `path_bounds` module-level
  `clippy::too_many_arguments` allow.
- Structured shared SVG curve path emission around `PathPoint`/`PathCubic`, merged duplicate basis
  bounded/unbounded logic, and removed the `curve` module-level `clippy::too_many_arguments` allow.
- Grouped journey text candidate geometry/font inputs into small structs and removed the `journey`
  module-level `clippy::too_many_arguments` allow.
- Replaced treemap root viewBox's long-argument rectangle bounds helper with a small accumulator
  and removed the `treemap` module-level `clippy::too_many_arguments` allow.
- Replaced requirement label foreignObject emission with a small input struct and removed the
  `requirement` module-level `clippy::too_many_arguments` allow.
- Bundled sankey relaxation parameters into a small context struct and removed the `sankey`
  module-level `clippy::too_many_arguments` allow.
- Replaced timeline node layout's positional content/geometry/text arguments with
  `TimelineNodeRequest` and removed the `timeline` module-level `clippy::too_many_arguments`
  allow.
- Bundled sequence block frame width planning inputs into `BlockFrameWidthContext` and removed the
  `sequence` module-level `clippy::too_many_arguments` allow.
- Replaced C4 SVG tspan text emission's positional geometry/font arguments with `C4TspanText` and
  removed the `svg/parity/c4` module-level `clippy::too_many_arguments` allow.
- Bundled C4 layout recursion inputs and output state into `C4LayoutContext` /
  `C4LayoutState`, removing the `c4.rs` module-level `clippy::too_many_arguments` allow.
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
