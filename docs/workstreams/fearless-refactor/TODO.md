# Fearless Refactor TODO

This backlog is intentionally architecture-focused. Each item should end with code deletion,
simpler ownership boundaries, stronger gates, or measurable performance improvement.

## P0: Safety and Verification Gates

- [x] Deduplicate typed render parse orchestration in `merman-core`.
  Evidence: commit `acceb66b`.
- [x] Deduplicate JSON layout dispatch in `merman-render`.
  Evidence: commit `acceb66b`.
- [x] Restore `cargo check --workspace --all-features`.
  Evidence: `flowchart_root_pack` compile failures fixed in commit `acceb66b`.
- [x] Make optional Node/KaTeX tests match optional backend semantics.
  Evidence: commit `acceb66b`.
- [x] Decide whether `cargo check --workspace --all-features` belongs in `xtask verify`.
  Decision: keep default `xtask verify` cost unchanged; add opt-in `--all-features` and include it
  in `--strict`.
- [x] Decide whether clippy belongs in `xtask verify`.
  Decision: add opt-in `--clippy`; `--strict` runs
  `cargo clippy --workspace --all-targets --all-features -- -D warnings`.
- [x] Make the workspace clippy-clean under the agreed release gate.
  Evidence: `cargo run -p xtask -- verify --strict` passed after cleanup. This includes
  `cargo clippy --workspace --all-targets --all-features -- -D warnings`.
  Cleanup included:
  - simple mechanical lints (`or_default`, `then_some`, `vec![]`, `with_capacity`, `?`).
  - render context structs replacing long parameter lists in flowchart/root SVG helpers.
  - all-target test-module ordering fixes and `xtask` helper lint cleanup.
- [x] Add a documented "fast local refactor gate" command set.
  Gap check: confirm which nextest packages and snapshot gates give the best signal per minute.
  Evidence: `README.md` now documents core, render, public API, feature-flag, and strict release
  gates.
- [x] Audit feature flags and remove or document stale experimental flags.
  Evidence: `flowchart_root_pack` was removed; remaining feature flags are documented in
  `README.md`.
- [x] Restore sequence DOM parity after the renderer split exposed note/block ordering drift.
  Evidence: `cargo run -p xtask -- compare-sequence-svgs --check-dom --dom-mode parity
  --dom-decimals 3` passes after rendering notes inline with the message-prelude interaction
  stream.

## P1: Typed Render Pipeline Cleanup

- [x] Inventory all diagrams by render model mode.
  Evidence: `RENDER_MODEL_INVENTORY.md`.
- [x] Remove duplicate error-diagram construction paths in `Engine`.
  Direction: centralize suppressed-error model construction for JSON and typed render models.
  Evidence: `error_diagram` now owns suppressed-error construction and
  `parse_lenient_failures_use_error_diagram_across_engine_entrypoints` covers all four Engine
  entrypoints.
- [x] Decide the future of `parse_diagram_for_render_sync`.
  Options:
  - Keep as compatibility-only and route all new render code through typed models.
  - Deprecate after public wrapper APIs no longer use it.
  Decision: remove it in M1. In-tree rendering already uses
  `parse_diagram_for_render_model_sync`, and the remaining special JSON-for-render handlers are
  obsolete for typed `mindmap` and `stateDiagram`.
- [x] Remove obsolete `parse_diagram_for_render_sync` compatibility API.
  Scope: remove the async alias and the `mindmap` / `stateDiagram` JSON-for-render helpers.
  Evidence: `parse_diagram_for_render_model_sync` is now the only render-optimized parse API.
- [x] Move sequence diagram render path toward a typed render model.
  Rationale: sequence has a large renderer and frequent layout/render coupling.
  Evidence: `parse_sequence_model_for_render` now returns `SequenceDiagramRenderModel`, and
  layout/SVG render-model dispatch consumes that typed model directly while `parse_diagram_sync`
  keeps the semantic JSON payload stable.
- [x] Move gantt or kanban render path toward a typed render model after sequence.
  Selection: kanban had the smaller parser/render surface and SVG rendering already reads only the
  layout model. Evidence: `parse_kanban_model_for_render` now returns `KanbanDiagramRenderModel`,
  and render-layout dispatch consumes it directly while `parse_diagram_sync` keeps the semantic JSON
  payload stable.
- [x] Move gantt render path toward a typed render model after kanban.
  Evidence: `parse_gantt_model_for_render` now returns `GanttDiagramRenderModel`, render-layout
  dispatch consumes it directly, SVG render-model dispatch consumes the same typed model, and
  `parse_diagram_sync` keeps the semantic JSON payload stable.
- [x] Move one small JSON-fallback diagram to a typed render model after gantt.
  Evidence: `parse_pie_model_for_render` now returns `PieDiagramRenderModel`, layout/SVG
  render-model dispatch consume it directly, and `parse_diagram_sync` keeps the semantic JSON
  payload stable.
- [x] Move one config-heavy small diagram to a typed render model.
  Evidence: `parse_packet_model_for_render` now returns `PacketDiagramRenderModel`, layout/SVG
  render-model dispatch consume it directly, and `parse_diagram_sync` keeps the semantic JSON
  payload stable.
- [x] Move one moderate small diagram to a typed render model and record drift honestly.
  Evidence: `parse_timeline_model_for_render` now returns `TimelineDiagramRenderModel`, layout/SVG
  render-model dispatch consume it directly, and `parse_diagram_sync` keeps the semantic JSON
  payload stable. The timing note records parse wins plus small layout/render midpoint drift.
- [x] Move one actor/task small diagram to a typed render model.
  Evidence: `parse_journey_model_for_render` now returns `JourneyDiagramRenderModel`, layout/SVG
  render-model dispatch consume it directly, and `parse_diagram_sync` keeps the semantic JSON
  payload stable, including omitted `scoreIsNaN` when false.
- [x] Move requirement diagrams to a typed render model.
  Evidence: `parse_requirement_model_for_render` now returns `RequirementDiagramRenderModel`,
  layout/SVG render-model dispatch consume it directly, and `parse_diagram_sync` keeps the semantic
  JSON payload stable, including `type`, `config`, accessibility fields, relationships, classes,
  and style data.
- [x] Move sankey diagrams to a typed render model.
  Evidence: `parse_sankey_model_for_render` now returns `SankeyDiagramRenderModel`, render-layout
  dispatch consumes it directly, SVG render-model dispatch uses the layout-only sankey SVG path,
  and `parse_diagram_sync` keeps the semantic JSON payload stable.
- [x] Move radar diagrams to a typed render model.
  Evidence: `parse_radar_model_for_render` now returns `RadarDiagramRenderModel`, layout/SVG
  render-model dispatch consume it directly, and `parse_diagram_sync` keeps the semantic JSON
  payload stable, including title, accessibility fields, axes, curves, options, and config.
- [x] Move info diagrams to a typed render model.
  Evidence: `parse_info_model_for_render` now returns `InfoDiagramRenderModel`, layout/SVG
  render-model dispatch consume it directly, and `parse_diagram_sync` keeps the semantic JSON
  payload stable.
- [x] Move ZenUML render-only flows to a typed sequence render model.
  Evidence: `parse_zenuml_model_for_render` translates ZenUML to sequence syntax once, returns
  `SequenceDiagramRenderModel`, render-layout dispatch accepts it for `zenuml`, and
  `parse_diagram_sync` keeps the semantic JSON payload stable.
- [x] Move quadrant charts to a typed render model.
  Evidence: `parse_quadrant_chart_model_for_render` now returns `QuadrantChartRenderModel`,
  layout/SVG render-model dispatch consume it directly, and `parse_diagram_sync` keeps the
  semantic JSON payload stable, including title, accessibility fields, axes, quadrants, points,
  classes, and config.
- [x] Move gitGraph to a typed render model.
  Evidence: `parse_git_graph_model_for_render` now returns `GitGraphRenderModel`, layout/SVG
  render-model dispatch consume it directly, `parse_diagram_sync` keeps the semantic JSON payload
  stable, and layout borrows typed commit/branch indexes instead of cloning private JSON transport
  structs.
- [x] Move treemap to a typed render model.
  Evidence: `parse_treemap_model_for_render` now returns `TreemapDiagramRenderModel`, layout and
  SVG render-model dispatch consume it directly, `parse_diagram_sync` keeps the semantic JSON
  payload stable, and the benchmark fixture was repaired so the pipeline can actually measure the
  diagram.
- [x] Move block diagrams to a typed render model.
  Evidence: `parse_block_model_for_render` now returns `BlockDiagramRenderModel`, layout/SVG
  render-model dispatch consume it directly, `parse_diagram_sync` keeps the semantic JSON payload
  stable, and render-side block JSON transport structs were replaced by the shared core model.
- [x] Move ER diagrams to a typed render model.
  Evidence: `parse_er_model_for_render` now returns `ErDiagramRenderModel`, layout/SVG
  render-model dispatch consume it directly, `parse_diagram_sync` keeps the semantic JSON payload
  stable, and render-side ER JSON transport structs were replaced by the shared core model.
- [x] Move xychart to a typed render model.
  Evidence: `parse_xychart_model_for_render` now returns `XyChartDiagramRenderModel`, layout and
  render-model dispatch consume it directly, `render_svg_sync` routes typed xychart through the
  public render path, and `parse_diagram_sync` keeps the semantic JSON payload stable.
- [ ] Add parse/render timing samples before and after each typed migration.
  Gate: `MERMAN_PARSE_TIMING=1` plus targeted render benchmarks.
  Sequence status: post-migration baseline captured in
  `docs/performance/spotcheck_2026-05-07_sequence_typed_render_model.md`. Kanban status:
  same-machine parent-vs-typed Criterion spotcheck captured in
  `docs/performance/spotcheck_2026-05-08_kanban_typed_render_model.md`. Gantt status:
  pre-migration JSON-fallback baseline captured in
  `docs/performance/spotcheck_2026-05-08_gantt_json_baseline.md`, and post-migration typed
  Criterion spotcheck captured in
  `docs/performance/spotcheck_2026-05-08_gantt_typed_render_model.md`. Pie status:
  parent-vs-typed Criterion spotcheck captured in
  `docs/performance/spotcheck_2026-05-08_pie_typed_render_model.md`. Packet status:
  parent-vs-typed Criterion spotcheck captured in
  `docs/performance/spotcheck_2026-05-08_packet_typed_render_model.md`. Timeline status:
  parent-vs-typed Criterion spotcheck captured in
  `docs/performance/spotcheck_2026-05-08_timeline_typed_render_model.md`. Journey status:
  parent-vs-typed Criterion spotcheck captured in
  `docs/performance/spotcheck_2026-05-08_journey_typed_render_model.md`. Requirement status:
  parent-vs-typed Criterion spotcheck captured in
  `docs/performance/spotcheck_2026-05-08_requirement_typed_render_model.md`. Sankey status:
  parent-vs-typed Criterion spotcheck captured in
  `docs/performance/spotcheck_2026-05-08_sankey_typed_render_model.md`. Radar status:
  parent-vs-typed Criterion spotcheck captured in
  `docs/performance/spotcheck_2026-05-08_radar_typed_render_model.md`. Info status:
  fixture-added JSON-fallback-vs-typed Criterion spotcheck captured in
  `docs/performance/spotcheck_2026-05-08_info_typed_render_model.md`. ZenUML status:
  parent-vs-typed Criterion spotcheck captured in
  `docs/performance/spotcheck_2026-05-08_zenuml_typed_render_model.md`. Quadrant chart status:
  parent-vs-typed Criterion spotcheck captured in
  `docs/performance/spotcheck_2026-05-08_quadrant_chart_typed_render_model.md`. GitGraph status:
  parent-vs-typed Criterion spotcheck captured in
  `docs/performance/spotcheck_2026-05-08_gitgraph_typed_render_model.md`. Treemap status:
  parent-vs-typed Criterion spotcheck captured in
  `docs/performance/spotcheck_2026-05-08_treemap_typed_render_model.md`. Block status:
  parent-vs-typed Criterion spotcheck captured in
  `docs/performance/spotcheck_2026-05-08_block_typed_render_model.md`. ER status:
  parent-vs-typed Criterion spotcheck captured in
  `docs/performance/spotcheck_2026-05-08_er_typed_render_model.md`. Keep this item open for the
  next typed migration. C4 status: post-migration typed render-path spotcheck recorded in
  `docs/performance/spotcheck_2026-05-08_c4_typed_render_model.md`; the JSON compatibility
  parity compare still passes via `cargo run -p xtask -- compare-c4-svgs --check-dom
  --dom-mode parity --dom-decimals 3`, `c4_medium` has a current pipeline bench smoke in
  `docs/performance/spotcheck_2026-05-09_c4_xychart_pipeline_bench_smoke.md`, and the direct
  `C4Db`-to-`C4DiagramRenderModel` cleanup is recorded in
  `docs/performance/spotcheck_2026-05-09_c4_direct_render_model_parse.md`. Cross-repo end-to-end
  comparison evidence is in `docs/performance/spotcheck_2026-05-09_c4_xychart_mmdr_comparison.md`
  and stage attribution is in `docs/performance/spotcheck_2026-05-09_c4_xychart_stage_mmdr.md`;
  the refreshed reports put C4 at roughly `1.3-1.4x` end-to-end and `1.8-2.0x` in parse, while
  Architecture layout and XyChart layout/render remain the clearer cross-repo gaps. XyChart status:
  post-migration typed render-path spotcheck recorded in
  `docs/performance/spotcheck_2026-05-08_xychart_typed_render_model.md`; the JSON compatibility
  parity compare still passes via `cargo run -p xtask -- compare-xychart-svgs --check-dom
  --dom-mode parity --dom-decimals 3`, and the repaired `xychart_medium` fixture now has a current
  pipeline bench smoke in `docs/performance/spotcheck_2026-05-09_c4_xychart_pipeline_bench_smoke.md`,
  with cross-repo end-to-end comparison evidence in
  `docs/performance/spotcheck_2026-05-09_c4_xychart_mmdr_comparison.md` and stage attribution in
  `docs/performance/spotcheck_2026-05-09_c4_xychart_stage_mmdr.md`. The follow-up render
  allocation cleanup is recorded in
  `docs/performance/spotcheck_2026-05-09_xychart_render_allocation_cleanup.md`, and the follow-up
  layout tick-cache cleanup is recorded in
  `docs/performance/spotcheck_2026-05-09_xychart_layout_tick_cache.md`.
  Mindmap/Architecture canary status: current local Criterion pipeline evidence is recorded in
  `docs/performance/spotcheck_2026-05-10_mindmap_architecture_canary_pipeline_long.md`; both
  canaries show strong local layout-stage improvement, while `parse/mindmap_medium` stays noisy
  enough that parser work is still not the next priority.
  Standard canary stage attribution with an explicit mmdr toolchain is recorded in
  `docs/performance/spotcheck_2026-05-10_standard_canaries_stage_mmdr_toolchain.md`; it keeps
  Architecture layout as the clearest remaining gap and render fixed-cost as the broad secondary
  signal.
  `crates/merman/tests/pipeline_bench_fixtures.rs` now guards all pipeline fixtures against
  Criterion pre-check skips under the `render` feature.

## P1: Text and Measurement Module Split

- [x] Split `crates/merman-render/src/text.rs` by responsibility.
  Evidence: `text.rs` is now a thin module/re-export boundary; text responsibilities live under
  dedicated `text/*` modules.
  Proposed modules:
  - `text/types.rs` (done)
  - `text/heuristic.rs` (done)
  - `text/markdown.rs` (tokenization done)
  - `text/markdown_label.rs` (HTML/XHTML fragment rendering done)
  - `text/measure.rs` (trait boundary done)
  - `text/deterministic.rs` (deterministic fallback measurer done)
  - `text/svg_metrics.rs` (done)
  - `text/flowchart_parity.rs` (done)
  - `text/metrics.rs` (flowchart-aware HTML/Markdown/SVG measurement done)
  - `text/html_label.rs` (defer until HTML label measurement needs another split)
  - `text/svg_text.rs` (defer until SVG text emission/measurement needs another split)
  - `text/overrides.rs` (shared flowchart/sequence lookup boundary done; remaining
    diagram-specific generated HTML width tables stay in diagram owners)
  - `text/font_metrics.rs` (vendored browser/font measurer done)
- [x] Keep public re-exports stable from `text.rs` or `text/mod.rs`.
  Evidence: first text split keeps existing `crate::text::*` callers unchanged.
- [x] Move markdown-only tests next to markdown code.
  Evidence: tokenization and HTML/XHTML fragment tests now live in `text/markdown.rs` and
  `text/markdown_label.rs`; measurement/layout-spanning Markdown tests remain in `text/tests.rs`.
- [x] Move override lookup tests next to override code.
  Evidence: flowchart text override lookup tests now live in `text/overrides.rs`; timeline override
  tests live in `timeline.rs`; existing diagram-specific generated override tests are colocated
  with their diagram owners.
- [x] Separate "browser compatibility measurement" from "deterministic fallback measurement".
  Evidence: `DeterministicTextMeasurer` now lives in `text/deterministic.rs`; browser/font
  compatibility measurement lives behind `VendoredFontMetricsTextMeasurer` in
  `text/font_metrics.rs`.
- [x] Keep diagram-specific HTML width overrides out of generic text measurement.
  Evidence: `VendoredFontMetricsTextMeasurer` now only applies shared/Flowchart HTML width
  overrides. ER and Block apply their generated HTML width tables in their owning renderer modules,
  while the stale Mindmap HTML width table and generator were deleted after layout snapshots proved
  the stable Mindmap path does not need it. Regression tests cover both no-leak generic text
  behavior and owner-local ER/Block parity lookup behavior.
- [x] Document when a text width override is allowed.
  Evidence: `OVERRIDE_POLICY.md` records allowed sources, disallowed shortcuts, placement rules,
  evidence checklist, and review questions.

## P1: Renderer Boundary Cleanup

- [x] Split `svg/parity/class/render.rs`.
  Proposed boundaries:
  - render context and ids (render lookup maps, small config helpers, and timing detail emission
    now live in `class/context.rs`; htmlLabels/font/padding/viewport/theme setting derivation now
    lives in `class/settings.rs`)
  - class box geometry (bounds accumulation helpers, class node shell/basic-container emission,
    HTML row measurement, HTML label-group emission, HTML class node body emission, SVG class node
    body emission, SVG title emission, SVG label-run emission, and divider emission now live in
    `class/bounds.rs` and `class/node.rs`; class node traversal, note/interface dispatch, and node
    body orchestration now live in `class/nodes.rs`; interface node emission now lives in
    `class/interface.rs`)
  - relation paths and labels (edge ids/classes, geometry/order, and edge label/terminal emission
    now live in `class/edge.rs`; shared HTML label metrics/styles now live in `class/label.rs`;
    edge paths, labels, terminals, data-point encoding, and timing accumulation now live in
    `class/edge.rs`; shared cluster/edge group orchestration now lives in `class/groups.rs`; the
    namespace-subgraph branch now reuses the shared optimized edge group path instead of a
    duplicate edge emitter)
  - SVG text labels (wrapping, label bbox, and bold-width compensation helpers now live in
    `class/label.rs`)
  - note rendering (note node emission now lives in `class/note.rs`)
  - namespace/subgraph rendering (ordering and subgraph open/close emission now live in
    `class/namespace.rs`; class node render-order/index construction and namespace
    wrapper/subgraph mode selection now also live there; namespace cluster group emission now
    lives there)
  - root/viewBox handling (SVG root opening, accessibility title/description emission, root
    viewBox/max-width placeholders, and graph-margin constant now live in `class/root.rs`; root
    viewBox/max-width calibration and class diagram title positioning now live in
    `class/viewbox.rs`)
  - debug SVG helpers
  Evidence: `class/render.rs` is now a thin orchestration layer; node, edge, namespace, root,
  viewBox, note, interface, debug SVG, settings, and context concerns live in dedicated
  `class/*` modules.
- [x] Split `svg/parity/sequence/render.rs`.
  Proposed boundaries:
  - render settings (sequence SVG config parsing now lives in `sequence/settings.rs`)
  - actors and participants (actor traversal and top/bottom ordering live in
    `sequence/actors.rs`; actor label, lifeline wrapper, and non-actor-man shape emission now live
    in `sequence/actor_shapes.rs`; actor-man top/bottom ordering lives in
    `sequence/actor_man.rs`; actor-man glyph geometry and SVG emission now live in
    `sequence/actor_man_glyphs.rs`; popup menu emission lives in `sequence/actor_popup.rs`;
    pre-actor box/rect frame emission now lives in `sequence/frames.rs`; shared node geometry now
    lives in `sequence/geometry.rs`)
  - messages (message label/line emission and autonumber handling now live in
    `sequence/messages.rs`)
  - notes (note emission now lives in `sequence/notes.rs`)
  - loops/alt/par/rect blocks (loop/alt/par/critical block collection now lives in
    `sequence/block_collection.rs`; label wrapping and loop text emission now live in
    `sequence/block_text.rs`; frame and message range geometry now lives in
    `sequence/block_geometry.rs`; shared frame/label-box emission now lives in
    `sequence/blocks.rs`; loop/opt/break share single-section block emission; alt/par share
    multi-section block emission; critical block emission owns a dedicated helper for its
    Mermaid-specific layout quirks)
  - activation rendering (precomputation and group emission now live in
    `sequence/activation.rs`)
  - interaction overlay orchestration (message-prelude notes, activations, and block frames now
    live in `sequence/interactions.rs`; notes now render inline with the message stream instead of
    being emitted as a pre-pass, preserving Mermaid DOM order around completed block frames)
  - viewport/bounds (root SVG opening, accessibility title/description, and sequence viewport
    override handling now live in `sequence/root.rs`)
  Evidence: `sequence/render.rs` is now a thin orchestration layer; actors, messages, notes,
  blocks, activations, interactions, root, settings, CSS, and model helpers live in dedicated
  `sequence/*` modules.
- [x] Split `svg/parity/architecture.rs`.
  Proposed boundaries:
  - typed model extraction (JSON and typed render-model access now live in
    `architecture/model.rs`)
  - CSS/theme/settings derivation (CSS construction, icon/padding/font/useMaxWidth settings, and
    text style derivation now live in `architecture/settings.rs`; root `<style>` emission remains
    in `architecture/root.rs`)
  - service/group layout emission (group rectangle recursion and shared bounds helpers now live in
    `architecture/geometry.rs`; service, junction, and group SVG emission now lives in
    `architecture/nodes.rs`)
  - edge rendering (direction, arrow, and edge-id helpers now live in `architecture/geometry.rs`;
    edge bounds accumulation and edge DOM emission now live in `architecture/edges.rs`)
  - icon/text XHTML normalization (foreignObject XHTML normalization and entity-safe ampersand
    escaping now live in `architecture/foreign_object.rs`; built-in icon SVG bodies now live in
    `architecture/icons.rs`; SVG label wrapping/text emission now lives in
    `architecture/labels.rs`)
  - root/viewBox handling (SVG root opening, accessibility title/description emission, empty
    diagram fallback sizing, and root viewBox/max-width placeholders now live in
    `architecture/root.rs`; root viewport finalization, profile calibration, f32 snapping, and
    generated root override replacement now live in `architecture/viewport.rs`)
  Evidence: `architecture.rs` is now an orchestration layer over `architecture/model.rs`,
  `architecture/settings.rs`, `architecture/nodes.rs`, `architecture/edges.rs`,
  `architecture/geometry.rs`, `architecture/labels.rs`, `architecture/icons.rs`,
  `architecture/foreign_object.rs`, `architecture/root.rs`, and `architecture/viewport.rs`.
- [x] Prefer small render context structs over long parameter lists.
  Result: sequence block frame helpers now share `SequenceBlockRenderContext`. Sequence message,
  interaction, actor, actor-man glyph, and loop-text helpers now use focused render contexts, and
  `svg/parity/sequence` no longer needs a module-level `clippy::too_many_arguments` allow.
  SVG path-bounds cubic/arc helpers now use explicit input structs, so `svg/parity/path_bounds.rs`
  also no longer needs a module-level allow.
  Shared SVG curve emit helpers now use `PathPoint` and `PathCubic`, the basis curve no longer
  duplicates bounded and unbounded emission paths, and `svg/parity/curve.rs` no longer needs a
  module-level `clippy::too_many_arguments` allow. Evidence: `cargo clippy -p merman-render
  --all-targets --all-features -- -D warnings`, `cargo nextest run -p merman-render`, and
  flowchart/class/ER/state DOM parity compares passed.
  Journey text candidate emission now groups geometry/font inputs into small structs, so
  `svg/parity/journey.rs` no longer needs a module-level allow. Evidence: `cargo clippy -p
  merman-render --all-targets --all-features -- -D warnings`, `cargo nextest run -p merman-render
  journey`, and `cargo run -p xtask -- compare-journey-svgs --check-dom --dom-decimals 3`.
  Treemap root viewBox accumulation now uses a small bounds accumulator instead of an eight-arg
  helper, so `svg/parity/treemap.rs` no longer needs a module-level allow. Evidence: `cargo
  clippy -p merman-render --all-targets --all-features -- -D warnings`, `cargo nextest run -p
  merman-render treemap`, and `cargo run -p xtask -- compare-treemap-svgs --check-dom
  --dom-decimals 3`.
  Requirement label foreignObject emission now uses a `LabelForeignObject` input struct, so
  `svg/parity/requirement.rs` no longer needs a module-level allow. Evidence: `cargo clippy -p
  merman-render --all-targets --all-features -- -D warnings`, `cargo nextest run -p merman-render
  requirement`, and `cargo run -p xtask -- compare-requirement-svgs --check-dom --dom-decimals 3`.
  Sankey relaxation passes now share a `RelaxParams` bundle instead of repeating layout tuning
  arguments, so `sankey.rs` no longer needs a module-level allow. Evidence: `cargo clippy -p
  merman-render --all-targets --all-features -- -D warnings`, `cargo nextest run -p merman-render
  sankey`, and `cargo run -p xtask -- compare-sankey-svgs --check-dom --dom-decimals 3`.
  Timeline node layout now uses a `TimelineNodeRequest` input struct instead of passing content,
  section, geometry, and text settings positionally, so `timeline.rs` no longer needs a module-level
  allow. Evidence: `cargo clippy -p merman-render --all-targets --all-features -- -D warnings`,
  `cargo nextest run -p merman-render timeline`, and `cargo run -p xtask -- compare-timeline-svgs
  --check-dom --dom-decimals 3`.
  Sequence block frame width planning now uses a `BlockFrameWidthContext` instead of repeating
  actor/message/text-measurement inputs, so `sequence.rs` no longer needs a module-level allow.
  Evidence: `cargo clippy -p merman-render --all-targets --all-features -- -D warnings`, `cargo
  nextest run -p merman-render sequence`, and `cargo run -p xtask -- compare-sequence-svgs
  --check-dom --dom-decimals 3`.
  C4 SVG tspan text emission now uses a `C4TspanText` input struct instead of positional geometry
  and font arguments, so `svg/parity/c4.rs` no longer needs a module-level allow. Evidence:
  `cargo clippy -p merman-render --all-targets --all-features -- -D warnings`, `cargo nextest run
  -p merman-render c4`, and `cargo run -p xtask -- compare-c4-svgs --check-dom --dom-decimals 3`.
  C4 layout recursion now shares `C4LayoutContext` and `C4LayoutState` instead of passing
  model/config/child-map/output state positionally, so `c4.rs` no longer needs a module-level
  allow. Evidence: `cargo clippy -p merman-render --all-targets --all-features -- -D warnings`,
  `cargo nextest run -p merman-render c4`, and `cargo run -p xtask -- compare-c4-svgs --check-dom
  --dom-decimals 3`.
  Architecture edge label planning, recursive group bounds computation, and the render-model entry
  point now use focused context structs instead of positional bundles, so
  `svg/parity/architecture.rs` no longer needs a module-level allow. Evidence: `cargo clippy -p
  merman-render --all-targets --all-features -- -D warnings`, `cargo nextest run -p merman-render
  architecture`, and `cargo run -p xtask -- compare-architecture-svgs --check-dom --dom-decimals
  3`.
  The legacy Architecture JSON compatibility model in `architecture.rs` was also trimmed of unused
  node/edge fields, and the unused top-level group separation helper was deleted while keeping the
  same clippy/nextest/DOM gates green.
  Class marker defs now use `MarkerContext` / `MarkerSpec` instead of long helper argument lists,
  so `svg/parity/class` no longer needs a module-level allow. Evidence: `cargo clippy -p
  merman-render --all-targets --all-features -- -D warnings`, `cargo nextest run -p merman-render
  class`, and `cargo run -p xtask -- compare-class-svgs --check-dom --dom-decimals 3`.
  State and requirement RoughJS rectangle helpers now use a spec struct instead of positional
  geometry/paint arguments, so `svg/parity/state` no longer needs a module-level allow and the
  requirement renderer follows the same narrowed call shape. Evidence: `cargo clippy -p
  merman-render --all-targets --all-features -- -D warnings`, `cargo nextest run -p merman-render
  state requirement`, `cargo run -p xtask -- compare-state-svgs --check-dom --dom-decimals 3`,
  and `cargo run -p xtask -- compare-requirement-svgs --check-dom --dom-decimals 3`.
  Vendored font metric wrapping now uses `FontMetricProfile` instead of repeating generated table
  references across line width and wrapping helpers, so `text.rs` no longer needs a module-level
  allow. Evidence: `cargo clippy -p merman-render --all-targets --all-features -- -D warnings`,
  `cargo nextest run -p merman-render text`, and `cargo run -p xtask -- compare-flowchart-svgs
  --check-dom --dom-decimals 3`.
  Flowchart layout/label helpers now use request and context structs (`FlowchartLabelMetricsRequest`,
  `NodeLayoutDimensionsRequest`, recursive layout context/state, place-graph inputs/outputs, and
  cluster rect context/state), so `flowchart/mod.rs` no longer needs a module-level allow.
  Evidence: `cargo clippy -p merman-render --all-targets --all-features -- -D warnings`,
  `cargo nextest run -p merman-render flowchart`, and `cargo run -p xtask -- compare-flowchart-svgs
  --check-dom --dom-decimals 3`.
  Core flowchart semantic application now uses `FlowchartSemanticContext`, state layout extraction
  uses `TypedLayoutContext` / `JsonLayoutContext`, and `StateDb::add_state` merges `StateStmt`
  directly. This removes the remaining source `clippy::too_many_arguments` allows. Evidence:
  `cargo clippy -p merman-core --all-targets --all-features -- -D warnings`, `cargo nextest run -p
  merman-core`, `cargo run -p xtask -- compare-flowchart-svgs --check-dom --dom-decimals 3`, and
  `cargo run -p xtask -- compare-state-svgs --check-dom --dom-decimals 3`, plus
  `cargo run -p xtask -- verify --strict`.
- [x] Remove dead debug helpers once equivalent `xtask` commands exist.
  Evidence: `crates/xtask/src/state_svgdump.rs` no longer carries dead `center()` or unused scope
  index fields; `crates/xtask/src/cmd/overrides/font_metrics.rs` removed unused browser/debug
  helpers and the old flowchart width estimator; `crates/xtask/src/cmd/overrides/svg.rs` dropped
  an unused `SampleKey`. `cargo nextest run -p xtask` and `cargo clippy -p xtask --all-targets
  --all-features -- -D warnings` passed afterward.
- [x] Remove the obsolete flowchart straight-except-one-endpoint helper after flowchart parity
  stayed green.
  Evidence: `crates/merman-render/src/svg/parity/flowchart/edge_geom/basis.rs` no longer keeps the
  `maybe_collapse_straight_except_one_endpoint` special case or its support helpers, and
  `cargo run -p xtask -- compare-flowchart-svgs --check-dom --dom-mode parity --dom-decimals 3
  --filter flowchart` stayed green without it.

## P2: Override Hygiene

- [x] Run and record override footprint.
  Command: `cargo run -p xtask -- report-overrides`.
  Evidence: `OVERRIDE_FOOTPRINT.md`.
- [x] Classify overrides by category:
  - generated text metrics
  - root viewport
  - raw SVG/path precision
  - temporary parity bridge
  Evidence: `xtask report-overrides` now scans generated override modules by category and
  hand-authored `maybe_override_*` raw SVG/path bridge functions under `svg/parity`.
- [x] Count generated text lookup tables as text lookup debt.
  Evidence: `xtask report-overrides` now counts rows in generated `*_OVERRIDES_*` binary-search
  tables as text metric lookup entries, so `block`, `er`, `gantt`, and `mindmap` table data no
  longer appear as hand-curated helper functions in `OVERRIDE_FOOTPRINT.md`.
- [x] Remove redundant public helper decomposition in hand-curated overrides.
  Evidence: Sankey first collapsed redundant padding component helpers, then moved its remaining
  node geometry values into `sankey` owner constants and deleted the now-empty generated module,
  reducing helper footprint without changing layout behavior.
- [x] Recheck the obsolete flowchart degenerate path helper before attempting removal again.
  Evidence: removing `crates/merman-render/src/svg/parity/flowchart/edge_geom/degenerate_path.rs`
  caused `cargo run -p xtask -- verify --strict` to fail with flowchart DOM mismatches on
  `stress_flowchart_subgraph_title_margins_extreme_nested_030`,
  `upstream_cypress_flowchart_v2_spec_5064_should_render_when_subgraph_child_has_links_to_outside_node_044`,
  and `upstream_flowchart_v2_subgraph_child_links_outside_spec`, so the helper stays in place.
- [x] Add generated metadata for generated overrides with expected removal criteria.
  Evidence: `xtask report-overrides` now prints owner, source, allowed-use, and expected-removal
  metadata for every generated override category and manual raw SVG/path bridge category, with a
  regression test guarding generated category removal metadata. The last manual raw bridge was
  removed and `xtask report-overrides` now reports zero manual bridge files. Zero-count categories
  now remain visible in the report with `no entries`, so eliminated helper/bridge categories are
  still auditable in strict-gate logs.
- [x] Count restricted-visibility helper functions in helper footprint.
  Evidence: `xtask report-overrides` now counts `pub(...) fn` helpers as hand-curated helper
  functions, so visibility-only changes cannot hide override footprint from the no-growth gate.
- [x] Recheck the redundant flowchart cluster-run helper before attempting removal again.
  Evidence: removing `maybe_remove_redundant_cluster_run_point` caused
  `cargo run -p xtask -- verify --strict` to fail with flowchart DOM mismatches on the same
  subgraph/cluster edge families, so the special case stays in place.
- [x] Delete the flowchart cyclic-special basis helper after proving the strict gate stays green.
  Evidence: removing `maybe_pad_cyclic_special_basis_route` from
  `crates/merman-render/src/svg/parity/flowchart/edge_geom/basis.rs` kept
  `cargo run -p xtask -- compare-flowchart-svgs --check-dom --dom-mode parity --dom-decimals 3
  --filter flowchart` green and `cargo run -p xtask -- verify --strict` passed.
- [ ] Delete overrides made obsolete by typed model or measurement fixes.
  Evidence: root viewport footprint is down 795 entries net so far: 19 `architecture` entries after
  topology-driven viewport calibration, 4 `journey` entries after the deterministic viewport path
  proved stable, and 11 `kanban` entries after profile-based root height calibration replaced the
  remaining fixture-specific pins, plus 4 `sankey` entries now covered by deterministic emitted
  bounds. The remaining 3 `sankey` root pins were rechecked by disabling the Sankey root lookup and
  running `compare-sankey-svgs --check-dom --dom-mode parity-root --dom-decimals 3`; they still
  guard root height drift in `upstream_docs_sankey_example_002`,
  `upstream_examples_sankey_energy_flow_001`, and `upstream_html_demos_sankey_energy_flow_002`.
  Timeline then dropped 9 entries now covered by deterministic root output, and 12 `pie` entries now
  covered by deterministic root output, plus 12 `er` entries now covered by deterministic root
  output, 35 `requirement` entries now covered by deterministic root output, and 16 `c4` entries
  now covered by deterministic root output. The entire 119-entry `block` root override module was
  deleted after Block `parity-root` stayed green without any root pins, followed by 68 `state`
  root pins now covered by deterministic root output. The Class root table then dropped 166
  obsolete pins and gained one missing docs root pin, making Class `parity-root` green with a
  165-entry net reduction. Gitgraph then dropped 6 obsolete pins while staying green under
  `parity-root`, and Sequence dropped 32 obsolete pins while staying green under `parity-root`.
  Flowchart then dropped 131 obsolete pins without adding new `parity-root` failures. The later
  `upstream_docs_math_flowcharts_001` math baseline normalization and sanitized KaTeX probe fix
  cleared the remaining Flowchart `parity-root` mismatch without growing the root table. Pie then
  replaced its 23 remaining root pins with a typed empty-pie root viewport rule plus shared
  1/64px-quantized legend SVG bbox measurement, deleting the Pie root override module while keeping
  `parity-root` green. Mindmap then refreshed typed root viewport profile calibration, added two
  small model-derived root profiles, and pruned 28 obsolete root pins while keeping `parity-root`
  green; disabling the remaining Mindmap lookup still leaves 52 root mismatches, so those pins stay
  until their geometry/text profiles move into typed renderer logic. A small-bucket audit also
  confirmed the remaining Timeline, Requirement, and ER root pins still fail when their lookups are
  disabled. Class then moved its remaining 31 root viewport pins into typed profile calibration and
  namespace render-mode rules, deleting the Class root override module while keeping `parity-root`
  green. Architecture then added default root viewport calibration for nested-groups and
  reasonable-height profiles and pruned 70 obsolete fixture-scoped pins, leaving 31 Architecture
  root pins that still fail when the lookup is disabled. The stale
  Mindmap HTML width lookup table and generator were also deleted after the shared text measurer
  leak was removed and layout snapshots proved the stable Mindmap path did not need those 291
  entries. One
  additional hand-curated
  `kanban` helper was removed by reusing the existing foreignObject height constant, and the
  XYChart bar data-label helpers were
  collapsed into one public scale helper. Treemap also dropped a derived section header
  center-y helper, Pie collapsed its center point into one helper for both axes, and Radar dropped
  a redundant legend baseline-y helper. Pie also dropped two derived legend-position helpers,
  now derives its label radius in layout, and Treemap reuses section inner padding for two header
  spacing values. Journey also reuses existing legend radius and top padding values for two
  formerly separate helpers. Sequence derives separator self-message y expansion from the existing
  frame envelope expansion. Kanban now reuses the section padding for the item label inset,
  and Architecture now reuses the service label bottom extension for singleton offsets. XYChart now
  uses one shared bar data-label inset helper for both bar orientations, reducing the helper
  footprint to 67. Pie later dropped a redundant outer-radius helper, Sequence now derives
  its note padding total from the existing note gap, Journey inlines its single-use legend
  placement and mouth offset values, and Radar inlines its remaining legend box size and label
  x-offset literals. Pie now inlines its fixed margin, center, radius, legend label font size,
  title y, and legend text y literals, leaving only the shared legend rectangle size and spacing
  helpers. XYChart now inlines its bar data-label scale and inset literals, deleting the empty
  generated override module. Treemap now inlines its section header label/value sizing literals,
  leaving only the shared section spacing helpers and leaf-fit tolerance. Sequence now inlines its
  self-only frame min pad literals in block geometry. Sankey now inlines its SVG-only label
  font/gap/dy literals, leaving only node geometry and padding helpers and reducing the helper
  footprint to 38. Architecture also deleted a dead icon text bbox helper, reducing the helper
  footprint to 37. Radar inlines its final legend row spacing value in layout and deletes the
  now-empty generated module, reducing the helper footprint to 36. Pie moved its remaining legend
  rectangle/spacing values into `pie` owner constants and deleted the now-empty generated module,
  reducing the helper footprint to 34. Sankey moved its remaining node width/padding values into
  `sankey` owner constants and deleted the now-empty generated module, reducing the helper
  footprint to 32. Journey moved its fixed viewBox/title/legend/face geometry into `journey`
  owner constants and deleted the now-empty generated module, reducing the helper footprint to 26.
  Kanban moved its section padding, label foreignObject height, and item row heights into `kanban`
  owner constants and deleted the now-empty generated module, reducing the helper footprint to 21.
  Treemap moved its section spacing geometry into `treemap` owner constants and kept the remaining
  `Item A1` leaf-fit browser tolerance beside the SVG parity loop, deleting the now-empty
  generated module and reducing the helper footprint to 18.
  Sequence moved its note wrap slack, text line-height math, and frame padding geometry into
  `sequence` owner constants/functions and deleted the now-empty generated module, reducing the
  helper footprint to 12.
  Architecture moved its text bbox formulas, canvas-label width scale, service label extension,
  and default wrap width into `architecture` owner constants/functions and deleted the now-empty
  generated module, reducing the helper footprint to 6.
  Gitgraph moved branch-label correction control flow into the `gitgraph` owner module and
  reclassified the remaining bbox correction data as text metric lookup entries, reducing the
  helper footprint to 0 while keeping measured correction data visible.
  C4 moved its three stable SVG bbox line-height rules into the C4 owner module and deleted the
  generated `c4_text_overrides_11_12_2.rs` module, leaving only the remaining text lookup tables.
  C4 also moved its 17 type-line `textLength` pins into the owner module and deleted the generated
  `c4_type_textlength_11_12_2.rs` module, so that logic now lives only in owner code.
  The single Timeline text lookup was rechecked by disabling it and running
  `compare-timeline-svgs --check-dom --dom-mode parity-root --dom-decimals 3`; it still guards the
  `upstream_long_word_wrap` root `max-width`, which drifts from `961px` to `961.5px` without the
  lookup.
  `compare-architecture-svgs --check-dom --dom-decimals 3`,
  `compare-block-svgs --check-dom --dom-mode parity-root --dom-decimals 3`,
  `compare-c4-svgs --check-dom --dom-mode parity-root --dom-decimals 3`,
  `compare-class-svgs --check-dom --dom-mode parity-root --dom-decimals 3`,
  `compare-er-svgs --check-dom --dom-mode parity-root --dom-decimals 3`,
  `compare-flowchart-svgs --check-dom --dom-decimals 3`,
  `compare-gitgraph-svgs --check-dom --dom-mode parity-root --dom-decimals 3`,
  `compare-pie-svgs --check-dom --dom-mode parity-root --dom-decimals 3`,
  `compare-requirement-svgs --check-dom --dom-mode parity-root --dom-decimals 3`,
  `compare-sankey-svgs --check-dom --dom-decimals 3`,
  `compare-sequence-svgs --check-dom --dom-mode parity-root --dom-decimals 3`,
  `compare-state-svgs --check-dom --dom-mode parity-root --dom-decimals 3`,
  `compare-timeline-svgs --check-dom --dom-mode parity-root --dom-decimals 3`,
  `compare-journey-svgs --check-dom --dom-mode parity --dom-decimals 3`,
  `compare-kanban-svgs --check-dom --dom-mode parity-root --dom-decimals 3`, and
  `compare-treemap-svgs --check-dom --dom-decimals 3` still pass. Flowchart `parity-root`
  now also passes after the math baseline normalization and sanitized KaTeX probe fix.
- [x] Prevent override tables from becoming the default fix for model bugs.
  Evidence: `xtask report-overrides --check-no-growth` now fails when any generated/manual override
  category grows beyond the explicit budget, and `xtask verify --strict` includes that gate.
  `OVERRIDE_POLICY.md` requires model/layout/sanitizer/DOM-order analysis before raising a budget.

## P2: Performance and Allocation

- [x] Establish baseline benchmark numbers for current `main`.
  Evidence: `docs/performance/spotcheck_2026-05-08_current_main_baseline.md`.
  Commands:
  - `cargo bench -p merman --features render --bench pipeline -- --noplot --sample-size 20
    --warm-up-time 1 --measurement-time 1`
  - targeted `flowchart_stress`, `architecture_stress`, `architecture_layout_stress`, and
    `mindmap_layout_stress` benches with the same Criterion options.
  Note: package-wide Criterion options were rejected by the lib bench harness, so the recorded
  baseline uses explicit `--bench` commands.
- [x] Record a same-machine benchmark spotcheck for the core flowchart/state context cleanup.
  Evidence: `docs/performance/spotcheck_2026-05-08_core_context_cleanup.md` compares parent
  `cefe26b3` with current `e7e761db` using isolated Criterion target directories.
- [x] Profile JSON clone cost in `layout_parsed` and public wrapper APIs.
  Evidence: `JSON_CLONE_AUDIT.md`. Decision: `layout_parsed` keeps the owned semantic clone for the
  compatibility API; public `render_svg_sync` already uses typed layout/render dispatch and avoids
  owned semantic JSON.
- [x] Reduce repeated `serde_json::Value` cloning in layout/render-only paths.
  Evidence: class typed/config layout and render keep `&MermaidConfig` through note HTML
  measurement/sanitization, sequence SVG rendering borrows the typed render model for title
  fallback, and the obsolete `render_layout_svg_parts_for_render_model` compat shim plus its
  no-config typed wrappers were removed in commit `2c491ace`; unused class layout no-config
  entrypoints were removed afterward. `JSON_CLONE_AUDIT.md` classifies the remaining
  `from_value(effective_config.clone())` sites as intentional legacy `&Value`
  compatibility bridges or lazy sanitizer fallbacks; future removal belongs to a public compatibility
  API redesign, not the render-only typed path.
- [x] Audit hot loops for avoidable string cloning in flowchart/class/sequence renderers.
  Evidence: `HOT_LOOP_CLONE_AUDIT.md` records the completed pass. Flowchart layout now borrows normal
  edges in the self-loop expansion stage, and layout/SVG render share an explicit helper-edge
  constructor that clones only retained source fields. Sequence block collection now borrows block
  labels and message ids from the typed render model; sequence activation planning now borrows
  active message and actor ids; non-wrapped sequence actor/message/note labels now render borrowed
  `<br>` split lines; block label hyphenation probes no longer clone the current head string. Class
  edge rendering now reuses sorted edge order, reuses marker-adjusted point buffers, borrows edge
  ids for edge-label center lookup, and class layout precomputes namespace facade lookup plus
  namespace declaration order once per render pass. The `class_namespace_dense` pipeline fixture now
  tracks this namespace-heavy path. Remaining clone sites are public compatibility, debug helper, or
  graphlib key ownership boundaries.
- [x] Add focused benchmarks before optimizing text measurement caches.
  Evidence: `text_measure_stress` now covers vendored font computed-length and SVG-like wrapped
  label measurement with default and bold Mermaid flowchart font styles. Same-machine spotcheck:
  `docs/performance/spotcheck_2026-05-08_text_measure_stress.md`.
- [x] Reduce XYChart SVG render allocation overhead.
  Evidence: `svg/parity/xychart.rs` now uses static node tags, insertion-order attribute vectors,
  one group-path helper, and direct CSS writes into the output buffer. The follow-up smoke is
  recorded in `docs/performance/spotcheck_2026-05-09_xychart_render_allocation_cleanup.md`, with
  `render/xychart_medium` at `113.74-122.92 us` and a `-14.810%` midpoint change in the local
  Criterion analysis.
- [x] Cache XYChart axis tick labels across layout passes.
  Evidence: `crates/merman-render/src/xychart.rs` now refreshes tick labels when an axis position
  changes and reuses borrowed tick slices in `calculate_space`, `tick_distance`, and axis drawable
  generation. The follow-up smoke is recorded in
  `docs/performance/spotcheck_2026-05-09_xychart_layout_tick_cache.md`, with
  `layout/xychart_medium` at `55.129-60.551 us` and a `-13.698%` midpoint change in the local
  Criterion analysis.
- [x] Refresh Mindmap/Architecture canary pipeline timing after the cleanup pass.
  Evidence: `docs/performance/spotcheck_2026-05-10_mindmap_architecture_canary_pipeline_long.md`
  records a current same-machine Criterion run for `mindmap_medium` and `architecture_medium`;
  both canaries show strong local layout-stage improvement, and the longer sample now serves as the
  default local checkpoint.
- [x] Move C4 typed textLength pins into the owner module.
  Evidence: `crates/merman-render/src/svg/parity/c4.rs` now owns the `textLength` mapping logic
  directly, and the generated `c4_type_textlength_11_12_2.rs` module was deleted.

## P3: Public API and CLI Cleanup

- [x] Review public `merman::render` API after typed render migration.
  Evidence: `PUBLIC_API_CLI_REVIEW.md`.
- [x] Keep synchronous executor-free API as the default path.
  Decision: `render_svg_sync`, `layout_diagram_sync`, and `HeadlessRenderer` remain the primary
  render integration surface.
- [x] Decide whether async wrappers should remain simple aliases or be feature-gated later.
  Decision: keep async wrappers as runtime-agnostic aliases for now; revisit near a public release
  boundary instead of creating preemptive churn.
- [x] Audit CLI option parsing for duplicated raster branches.
  Evidence: CLI layout options and SVG rasterization output handling now share small helpers across
  Mermaid-input render and direct SVG-input rasterization flows.
- [x] Consider a small internal `RenderRequest`/`RasterRequest` struct for CLI command execution.
  Evidence: `crates/merman-cli/src/main.rs` now routes parse/layout/render through internal
  `RenderRequest` and `RasterRequest` structs, which centralize layout options, SVG options,
  raster options, and default raster output-path resolution without changing CLI behavior.

## P3: Documentation Cleanup

- [x] Update README architecture notes after pipeline cleanup.
  Evidence: root `README.md` documents the typed render-model path, compatibility
  `layout_diagram_sync` / `render_layouted_svg` paths, and parity renderer ownership boundaries.
- [x] Add a short contributor guide for adding a new typed diagram renderer.
  Evidence: `TYPED_RENDERER_GUIDE.md` documents the typed model, layout, SVG dispatch,
  compatibility, and gate checklist for new migrations.
- [x] Document standard gates for parity, refactor, and release work.
  Evidence: `README.md` now points to `GATES.md`, which records the refactor, parity, performance,
  and release command sets.
- [x] Document what "Mermaid parity" means for generated override data.
  Evidence: `OVERRIDE_POLICY.md` defines generated override parity as narrow, reproducible
  Mermaid `@11.12.3` browser/export facts with explicit removal triggers.
