# Fearless Refactor Changelog

This log records completed changes that materially advance the fearless-refactor workstream.
Detailed planning remains in `TODO.md` and `MILESTONES.md`.

## 2026-05-09

- Pruned 4 obsolete `kanban` root viewport entries from the generated table after confirming the
  remaining 7 fixture-specific pins still gate `parity-root`.
- Removed the obsolete generated `journey` root viewport override table and its renderer call site
  after DOM parity passed without the 4 fixture-specific pins.
- Consolidated `merman-cli` render execution around internal `RenderRequest` and
  `RasterRequest` structs so parse/layout/render and SVG-raster handling share a smaller execution
  boundary without changing CLI behavior.

## 2026-05-08

- Corrected `xtask report-overrides` text lookup accounting so generated `*_OVERRIDES_*`
  binary-search tables are counted as text metric lookup entries instead of hand-curated helpers,
  with refreshed no-growth budgets and footprint docs.
- Collapsed redundant public Sankey padding component helpers into private constants, leaving only
  the `showValues`-aware public padding lookup in the override footprint.
- Removed unused requirement-layout `max-width` calculation state plus dead state/gantt helper
  functions that were kept only behind `dead_code` allows.
- Added a focused `text_measure_stress` Criterion bench for vendored font measurement and wrapped
  label paths before future cache work.
- Recorded the `text_measure_stress` same-machine Criterion spotcheck in
  `docs/performance/spotcheck_2026-05-08_text_measure_stress.md`.
- Removed a dead private font-metric quantizer and made the flowchart cluster-width probe
  test-only so production text-measure code stays slimmer.
- Added category-level owner/source/allowed-use/expected-removal metadata to `xtask
  report-overrides`, plus a regression test so generated override categories keep explicit removal
  criteria.
- Removed dead xtask debug/generator helpers, including unused state analyzer geometry, an obsolete
  font-metrics browser char-width helper, a stale flowchart width estimator, and an unused SVG
  override scratch struct.
- Added an override no-growth budget gate to `xtask report-overrides` and wired it into
  `xtask verify --strict` so new override growth must be explicit.
- Replaced `check-upstream-svgs`' long-argument helper with a request struct, removing the last
  `clippy::too_many_arguments` allow from `xtask` source.
- Removed 19 redundant architecture root viewport overrides after topology-driven calibration
  covered the matching profiles.
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
- Replaced architecture edge label geometry arguments, recursive group bounds arguments, and the
  render-model entry argument list with focused context structs, removing the
  `svg/parity/architecture.rs` module-level `clippy::too_many_arguments` allow.
- Replaced class marker defs helper argument lists with `MarkerContext` / `MarkerSpec`, removing
  the `svg/parity/class` module-level `clippy::too_many_arguments` allow.
- Replaced state RoughJS rectangle arguments with `StateRoughRectSpec`, removing the
  `svg/parity/state` module-level `clippy::too_many_arguments` allow and narrowing the requirement
  renderer call site to the same spec shape.
- Replaced vendored font-metric table argument lists with `FontMetricProfile`, removing the
  `text.rs` module-level `clippy::too_many_arguments` allow.
- Replaced flowchart label, node layout, recursive layout, place-graph, and cluster rect argument
  bundles with request/context structs, removing the `flowchart/mod.rs` module-level
  `clippy::too_many_arguments` allow.
- Replaced core flowchart semantic and state layout long-argument helpers with
  `FlowchartSemanticContext`, `TypedLayoutContext`, and `JsonLayoutContext`, and made
  `StateDb::add_state` merge `StateStmt` directly. Source code no longer carries
  `clippy::too_many_arguments` allows.
- Recorded an isolated Criterion spotcheck for the core flowchart/state context cleanup using
  `flowchart_medium` and `state_medium` in separate target directories.
- Removed the obsolete `render_layout_svg_parts_for_render_model` compat dispatcher and the
  no-config typed wrappers it exclusively served; typed render-model SVG dispatch now uses the
  `*_with_config` surface.
- Closed the render-only JSON clone cleanup batch after class, sequence, and render-model dispatch
  paths were reduced to intentional compatibility and lazy-sanitizer fallbacks.
- Removed the unused no-config class layout entrypoints so class note HTML measurement now keeps
  the parser's borrowed `MermaidConfig` through the typed path.
- Closed the flowchart/class/sequence hot-loop clone audit, leaving only compatibility, debug, and
  graphlib key ownership boundaries for future API-level work.
- Added `GATES.md` as the canonical refactor, parity, performance, and release gate reference for
  this workstream.
- Updated the root README architecture notes to describe the typed render-model path and the
  compatibility layout/render boundaries.
- Documented generated override parity as narrow Mermaid `@11.12.3` browser/export facts with
  explicit removal triggers.
- Added `TYPED_RENDERER_GUIDE.md` to document the standard checklist for new typed diagram renderer
  migrations.
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
