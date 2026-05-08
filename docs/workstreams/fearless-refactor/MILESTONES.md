# Fearless Refactor Milestones

This roadmap targets a cleaner next release without losing Mermaid parity. Milestones are ordered
so each stage reduces future risk before deeper changes begin.

## Goal Statement

The next version should be easier to maintain than the current one while being at least as complete,
at least as fast, and at least as parity-safe.

Success means:

- New contributors can understand the parse/layout/render pipeline without reading multiple
  duplicate dispatch blocks.
- High-impact diagrams no longer pay unnecessary JSON construction costs in render-only paths.
- Text measurement and markdown/HTML/SVG label logic have clear module boundaries.
- Large parity renderers can be changed locally without scanning thousands of unrelated lines.
- All feature-gated code compiles regularly.
- Clippy stays green for the workspace under the agreed release gate.
- Override growth is visible and justified.

## M0: Refactor Safety Baseline

Status: complete.

Evidence:

- `cargo run -p xtask -- verify --strict` passed. This covers fmt, all-features check,
  workspace all-target/all-features clippy, nextest, and SVG DOM parity.

Scope:

- Keep default tests green.
- Keep `--all-features` compilation green.
- Establish the standard gate list for refactor work.
- Make optional environment-dependent tests robust.

Exit criteria:

- `cargo fmt`
- `cargo check --workspace --all-features`
- `cargo clippy --workspace --all-targets --all-features -- -D warnings`
- `cargo run -p xtask -- verify --strict`
- `cargo nextest run -p merman-core -p merman-render`
- No known broken feature flags.

## M1: Pipeline Ownership Cleanup

Scope:

- Centralize parse/render dispatch.
- Centralize JSON layout dispatch.
- Centralize suppressed-error diagram construction.
- Remove the obsolete `parse_diagram_for_render_sync` compatibility API.

Exit criteria:

- One typed render parse dispatcher in `merman-core`.
- One JSON layout fallback dispatcher in `merman-render`.
- Error-diagram fallback code is not repeated across public parse methods.
- The render-optimized public parse API is `parse_diagram_for_render_model_sync`.
- Public wrappers use the typed render path where possible.

## M2: Typed Model Expansion

Status: in progress.

Scope:

- Move one large remaining JSON-render diagram to a typed render model.
- Use the first migration to define the repeatable pattern.
- Prefer sequence first unless profiling points elsewhere.

Progress:

- `sequence` now has a typed render model consumed by layout and SVG render-model dispatch.
- `kanban` now has a typed render model consumed by render-layout dispatch; semantic JSON parsing
  remains stable for the compatibility API.
- `gantt` now has a typed render model consumed by render-layout and SVG render-model dispatch;
  semantic JSON parsing remains stable for the compatibility API.
- `pie` now has a typed render model consumed by render-layout and SVG render-model dispatch;
  semantic JSON parsing remains stable for the compatibility API.
- `packet` now has a typed render model consumed by render-layout and SVG render-model dispatch;
  semantic JSON parsing remains stable for the compatibility API.
- `timeline` now has a typed render model consumed by render-layout and SVG render-model dispatch;
  semantic JSON parsing remains stable for the compatibility API.
- `journey` now has a typed render model consumed by render-layout and SVG render-model dispatch;
  semantic JSON parsing remains stable for the compatibility API.
- `requirement` now has a typed render model consumed by render-layout and SVG render-model
  dispatch; semantic JSON parsing remains stable for the compatibility API.
- `sankey` now has a typed render model consumed by render-layout dispatch; SVG render-model
  dispatch reuses the layout-only sankey SVG path.
- `radar` now has a typed render model consumed by render-layout and SVG render-model dispatch;
  semantic JSON parsing remains stable for the compatibility API.
- `info` now has a typed render model consumed by render-layout and SVG render-model dispatch;
  semantic JSON parsing remains stable for the compatibility API.
- `zenuml` now translates to sequence once and uses `SequenceDiagramRenderModel` in render-only
  flows; semantic JSON parsing remains stable for the compatibility API.
- `quadrantChart` now has a typed render model consumed by render-layout and SVG render-model
  dispatch; semantic JSON parsing remains stable for the compatibility API.
- `gitGraph` now has a typed render model consumed by render-layout and SVG render-model dispatch;
  semantic JSON parsing remains stable for the compatibility API, and layout borrows typed
  commit/branch indexes instead of cloning private transport structs.
- `treemap` now has a typed render model consumed by render-layout dispatch and layout-only SVG
  render-model dispatch; semantic JSON parsing remains stable for the compatibility API.
- `block` now has a typed render model consumed by render-layout and SVG render-model dispatch;
  semantic JSON parsing remains stable for the compatibility API.
- `er` now has a typed render model consumed by render-layout and SVG render-model dispatch;
  semantic JSON parsing remains stable for the compatibility API.
- `c4` now has a typed render model consumed by render-layout and SVG render-model dispatch;
  semantic JSON parsing remains stable for the compatibility API, and the JSON render bridge is
  now just a compatibility wrapper.
- `xychart` now has a typed render model consumed by render-layout dispatch; SVG emission stays
  layout-only, and the public render path routes through the typed model without changing the
  semantic JSON compatibility API.
- Post-migration sequence timing and benchmark evidence is recorded in
  `docs/performance/spotcheck_2026-05-07_sequence_typed_render_model.md`.
- Kanban parent-vs-typed timing evidence is recorded in
  `docs/performance/spotcheck_2026-05-08_kanban_typed_render_model.md`.
- Gantt pre-migration JSON-fallback baseline is recorded in
  `docs/performance/spotcheck_2026-05-08_gantt_json_baseline.md`.
- Gantt post-migration typed timing evidence is recorded in
  `docs/performance/spotcheck_2026-05-08_gantt_typed_render_model.md`.
- Pie parent-vs-typed timing evidence is recorded in
  `docs/performance/spotcheck_2026-05-08_pie_typed_render_model.md`.
- Packet parent-vs-typed timing evidence is recorded in
  `docs/performance/spotcheck_2026-05-08_packet_typed_render_model.md`.
- Timeline parent-vs-typed timing evidence is recorded in
  `docs/performance/spotcheck_2026-05-08_timeline_typed_render_model.md`.
- Journey parent-vs-typed timing evidence is recorded in
  `docs/performance/spotcheck_2026-05-08_journey_typed_render_model.md`.
- Requirement parent-vs-typed timing evidence is recorded in
  `docs/performance/spotcheck_2026-05-08_requirement_typed_render_model.md`.
- Sankey parent-vs-typed timing evidence is recorded in
  `docs/performance/spotcheck_2026-05-08_sankey_typed_render_model.md`.
- Radar parent-vs-typed timing evidence is recorded in
  `docs/performance/spotcheck_2026-05-08_radar_typed_render_model.md`.
- Info fixture-added JSON-fallback-vs-typed timing evidence is recorded in
  `docs/performance/spotcheck_2026-05-08_info_typed_render_model.md`.
- ZenUML parent-vs-typed timing evidence is recorded in
  `docs/performance/spotcheck_2026-05-08_zenuml_typed_render_model.md`.
- Quadrant chart parent-vs-typed timing evidence is recorded in
  `docs/performance/spotcheck_2026-05-08_quadrant_chart_typed_render_model.md`.
- GitGraph parent-vs-typed timing evidence is recorded in
  `docs/performance/spotcheck_2026-05-08_gitgraph_typed_render_model.md`.
- Treemap parent-vs-typed timing evidence is recorded in
  `docs/performance/spotcheck_2026-05-08_treemap_typed_render_model.md`.
- Block parent-vs-typed timing evidence is recorded in
  `docs/performance/spotcheck_2026-05-08_block_typed_render_model.md`.
- ER parent-vs-typed timing evidence is recorded in
  `docs/performance/spotcheck_2026-05-08_er_typed_render_model.md`.
- C4 post-migration typed render-path spotcheck is recorded in
  `docs/performance/spotcheck_2026-05-08_c4_typed_render_model.md`.
- XYChart typed render-path spotcheck is recorded in
  `docs/performance/spotcheck_2026-05-08_xychart_typed_render_model.md`.
- Same-machine baseline capture remains a process requirement for future typed migrations.

Exit criteria:

- At least one additional high-impact diagram has a typed render model.
- Render-only path avoids constructing the full semantic JSON model for that diagram.
- Existing semantic JSON API remains stable.
- Benchmarks or timing logs show the cost impact.

## M3: Text System Modularization

Status: complete for the text module split; keep follow-up cleanup under M5 override governance.

Scope:

- Split `merman-render` text handling by responsibility.
- Separate markdown, HTML label, SVG text, font metrics, wrapping, and override lookup code.
- Keep public text APIs stable through re-exports.

Progress:

- Extracted shared text types, SVG/font bbox helpers, and flowchart HTML parity helpers into
  `text/types.rs`, `text/svg_metrics.rs`, and `text/flowchart_parity.rs` while preserving
  `crate::text::*` re-exports.
- Extracted deterministic fallback width heuristics into `text/heuristic.rs`.
- Extracted Mermaid-like Markdown tokenization into `text/markdown.rs`.
- Extracted Mermaid HTML/XHTML label fragment rendering into `text/markdown_label.rs`.
- Moved markdown-only tests next to the split Markdown modules; cross-boundary measurement tests
  stay in the shared text test module.
- Extracted the `TextMeasurer` trait boundary into `text/measure.rs`.
- Extracted deterministic fallback text measurement and wrapping into `text/deterministic.rs`,
  leaving browser/font compatibility measurement behind the vendored measurer boundary.
- Extracted vendored browser/font measurement into `text/font_metrics.rs` while keeping
  `VendoredFontMetricsTextMeasurer` re-exported from `crate::text`.
- Extracted flowchart-aware HTML, Markdown, and precise SVG measurement helpers into
  `text/metrics.rs` while preserving the existing `crate::text::*` call surface.
- Introduced `text/overrides.rs` as the text override lookup boundary and moved flowchart text
  override lookup tests next to that owner.
- Moved timeline long-word override lookup and regression tests next to `timeline.rs`.
- Override lookup tests are now colocated with the text override boundary or their diagram owners.

Exit criteria:

- `text.rs` is no longer a large mixed-responsibility module.
- Tests live near the text subsystem they protect.
- Override lookup rules are documented.
- Flowchart/class/state text parity fixtures remain green.

## M4: Large Renderer Decomposition

Scope:

- Split class, sequence, and architecture parity renderers into smaller modules.
- Introduce render context structs where long parameter lists obscure ownership.
- Delete obsolete debug-only code after replacing it with `xtask` tooling or tests.

Progress:

- Extracted class SVG content-bounds helpers into `svg/parity/class/bounds.rs`.
- Extracted class node shell, basic-container emission, HTML row measurement, HTML label-group
  emission, SVG title emission, SVG label-run emission, and divider emission into
  `svg/parity/class/node.rs`.
- Started sequence renderer actor/participant split by moving actor label emission into
  `svg/parity/sequence/actors.rs`.
- Extracted sequence pre-actor box/rect frame emission into `svg/parity/sequence/frames.rs`.
- Extracted sequence actor popup menu emission into `svg/parity/sequence/actors.rs` and shared
  node geometry into `svg/parity/sequence/geometry.rs`.
- Extracted sequence actor-man top/bottom variant emission into
  `svg/parity/sequence/actors.rs`.
- Split sequence actor popup menu emission and actor-man variants into
  `svg/parity/sequence/actor_popup.rs` and `svg/parity/sequence/actor_man.rs`.
- Extracted sequence top/bottom actor box and lifeline emission into
  `svg/parity/sequence/actors.rs`.
- Extracted sequence SVG render settings/config parsing into
  `svg/parity/sequence/settings.rs`.
- Extracted sequence root SVG opening, accessibility title/description, and viewport override
  handling into `svg/parity/sequence/root.rs`.
- Extracted sequence activation precomputation and group emission into
  `svg/parity/sequence/activation.rs`.
- Extracted sequence note emission into `svg/parity/sequence/notes.rs`.
- Extracted sequence message-prelude interaction overlay orchestration for notes, activations, and
  block frames into `svg/parity/sequence/interactions.rs`.
- Adjusted sequence interaction overlay ordering so note groups render inline with the message
  stream, keeping them in Mermaid DOM order relative to completed block frames.
- Extracted sequence loop/alt/par/critical block model collection into
  `svg/parity/sequence/block_collection.rs`.
- Extracted sequence block label wrapping, loop text emission, and block frame range helpers into
  `svg/parity/sequence/blocks.rs`.
- Split sequence block label wrapping and loop text emission into
  `svg/parity/sequence/block_text.rs`.
- Split sequence block frame/message range geometry into
  `svg/parity/sequence/block_geometry.rs`.
- Split sequence actor label, lifeline wrapper, and non-actor-man shape emission into
  `svg/parity/sequence/actor_shapes.rs`.
- Split sequence actor-man glyph geometry and SVG emission into
  `svg/parity/sequence/actor_man_glyphs.rs`.
- Extracted sequence message label/line emission and autonumber handling into
  `svg/parity/sequence/messages.rs`.
- Shared sequence block frame and label-box emission helpers across loop/alt/par/critical
  variants.
- Shared sequence block message y-range and separator y-position helpers across
  loop/alt/par/critical variants.
- Shared single-section sequence block emission for loop/opt/break variants.
- Shared multi-section sequence block emission for alt/par variants.
- Extracted critical sequence block emission into `svg/parity/sequence/blocks.rs`, keeping its
  multi-section frame widening and single-section header wrap behavior localized.
- Added `SequenceBlockRenderContext` so sequence block frame helpers share one explicit parameter
  bundle instead of repeated long argument lists.
- Added focused sequence render contexts for actors, actor-man glyphs, interaction overlays,
  message rendering, and loop text emission; `svg/parity/sequence` no longer carries a module-level
  `clippy::too_many_arguments` allow.
- Structured SVG path-bounds cubic and arc inputs so `svg/parity/path_bounds.rs` no longer carries
  a module-level `clippy::too_many_arguments` allow.
- Structured shared SVG curve emission around `PathPoint` and `PathCubic`, merged the duplicate
  bounded/unbounded basis curve paths, and removed the `svg/parity/curve.rs` module-level
  `clippy::too_many_arguments` allow.
- Grouped journey text candidate geometry/font inputs into small structs and removed the
  `svg/parity/journey.rs` module-level `clippy::too_many_arguments` allow.
- Replaced treemap root viewBox's long-argument rectangle bounds helper with a small accumulator
  and removed the `svg/parity/treemap.rs` module-level `clippy::too_many_arguments` allow.
- Replaced requirement label foreignObject emission with a small input struct and removed the
  `svg/parity/requirement.rs` module-level `clippy::too_many_arguments` allow.
- Bundled sankey relaxation parameters into a small context struct and removed the
  `crates/merman-render/src/sankey.rs` module-level `clippy::too_many_arguments` allow.
- Replaced timeline node layout's positional content/geometry/text arguments with
  `TimelineNodeRequest` and removed the `crates/merman-render/src/timeline.rs` module-level
  `clippy::too_many_arguments` allow.
- Bundled sequence block frame width planning inputs into `BlockFrameWidthContext` and removed the
  `crates/merman-render/src/sequence.rs` module-level `clippy::too_many_arguments` allow.
- Replaced C4 SVG tspan text emission's positional geometry/font arguments with `C4TspanText` and
  removed the `svg/parity/c4.rs` module-level `clippy::too_many_arguments` allow.
- Bundled C4 layout recursion inputs and output state into `C4LayoutContext` /
  `C4LayoutState`, removing the `crates/merman-render/src/c4.rs` module-level
  `clippy::too_many_arguments` allow.
- Extracted class edge geometry/order helpers into `svg/parity/class/edge.rs`.
- Extracted class edge label/terminal emission into `svg/parity/class/edge.rs`.
- Moved class edge DOM id and edge class pattern helpers into `svg/parity/class/edge.rs`.
- Moved class edge paths, edge labels, terminals, data-point encoding, and timing accumulation into
  `svg/parity/class/edge.rs`.
- Extracted shared class cluster/edge group orchestration for `clusters`, `edgePaths`, and
  `edgeLabels` into `svg/parity/class/groups.rs`.
- Moved class HTML label metrics/styles into `svg/parity/class/label.rs`.
- Moved class SVG text wrapping, label bbox, and bold-width compensation helpers into
  `svg/parity/class/label.rs`.
- Moved class render lookup maps, small config helpers, and timing detail emission into
  `svg/parity/class/context.rs`.
- Extracted class SVG render setting derivation for htmlLabels, font sizing, padding, viewport
  padding, and theme defaults into `svg/parity/class/settings.rs`.
- Moved HTML class node body emission into `svg/parity/class/node.rs`.
- Moved SVG class node body emission into `svg/parity/class/node.rs`.
- Extracted class node traversal, namespace-subgraph transitions, note/interface dispatch, and
  class node body orchestration into `svg/parity/class/nodes.rs`.
- Removed the duplicate class namespace-subgraph edge path/label emitter and routed it through
  the shared optimized edge group path.
- Extracted class interface node emission into `svg/parity/class/interface.rs`, sharing node
  position data through `svg/parity/class/node.rs`.
- Extracted class note node emission into `svg/parity/class/note.rs`.
- Extracted class namespace ordering and nested subgraph emission into
  `svg/parity/class/namespace.rs`.
- Extracted class node render-order/index construction into
  `svg/parity/class/namespace.rs`.
- Extracted class namespace wrapper/subgraph mode selection into
  `svg/parity/class/namespace.rs`.
- Extracted class namespace cluster group emission into
  `svg/parity/class/namespace.rs`.
- Extracted class SVG root opening, accessibility title/description emission, root
  viewBox/max-width placeholders, and graph-margin constant into `svg/parity/class/root.rs`.
- Extracted class root viewBox/max-width calibration and class diagram title positioning into
  `svg/parity/class/viewbox.rs`.
- Extracted architecture JSON/typed render-model access into
  `svg/parity/architecture/model.rs`.
- Extracted architecture render settings, CSS construction, and theme/config-derived text style
  setup into `svg/parity/architecture/settings.rs`.
- Extracted architecture service, junction, and group SVG emission into
  `svg/parity/architecture/nodes.rs`.
- Extracted architecture foreignObject XHTML normalization and entity-safe ampersand escaping into
  `svg/parity/architecture/foreign_object.rs`, with its normalization tests colocated there.
- Extracted architecture built-in icon SVG bodies into `svg/parity/architecture/icons.rs`.
- Extracted architecture SVG label wrapping and text/tspan emission into
  `svg/parity/architecture/labels.rs`.
- Extracted architecture edge direction/arrow helpers, shared bounds helpers, and recursive group
  rectangle computation into `svg/parity/architecture/geometry.rs`.
- Extracted architecture edge bounds accumulation and DOM emission into
  `svg/parity/architecture/edges.rs`.
- Refactored architecture edge label wrapping, bbox projection, and transform derivation into one
  local render plan so bounds accumulation and DOM emission share the same computation.
- Replaced architecture edge label geometry arguments, recursive group bounds arguments, and the
  render-model entry argument list with focused context structs, removing the
  `svg/parity/architecture.rs` module-level `clippy::too_many_arguments` allow.
- Replaced class marker defs helper argument lists with `MarkerContext` / `MarkerSpec`, removing
  the `svg/parity/class` module-level `clippy::too_many_arguments` allow.
- Replaced state RoughJS rectangle arguments with `StateRoughRectSpec`, removing the
  `svg/parity/state` module-level `clippy::too_many_arguments` allow and narrowing the
  requirement renderer call site to the same spec shape.
- Replaced vendored font-metric table argument lists with `FontMetricProfile`, removing the
  `text.rs` module-level `clippy::too_many_arguments` allow.
- Replaced flowchart label, node layout, recursive layout, place-graph, and cluster rect argument
  bundles with request/context structs, removing the `flowchart/mod.rs` module-level
  `clippy::too_many_arguments` allow.
- Replaced core flowchart semantic and state layout long-argument helpers with context structs, and
  made `StateDb::add_state` merge `StateStmt` directly; source code no longer carries
  `clippy::too_many_arguments` allows.
- Extracted architecture SVG root opening, accessibility title/description emission, empty diagram
  fallback sizing, and root viewBox/max-width placeholders into
  `svg/parity/architecture/root.rs`.
- Extracted architecture root viewport finalization, profile calibration, `f32` snapping, and
  generated root override replacement into `svg/parity/architecture/viewport.rs`.
- Class and sequence renderer splits are complete for the current scope: `class/render.rs` and
  `sequence/render.rs` are thin orchestration boundaries over dedicated owner modules.
- Architecture renderer split is now complete for the current scope; keep any follow-up cleanup
  under M5 if future profiling or navigation reveals new dead code.
- Removed the remaining dead xtask debug helpers and stale scratch structs after equivalent
  commands existed; the state SVG analyzer, font-metrics generator, and SVG override generator no
  longer keep dead helper code around.

Exit criteria:

- No single parity renderer file remains difficult to navigate because of unrelated concerns.
- Diagram-specific tests still cover layout, SVG, and entity-sanitization behavior.
- No DOM parity regressions for the touched diagram families.

## M5: Override Governance and Debt Reduction

Status: in progress.

Scope:

- Inventory generated and manual overrides.
- Remove stale or redundant overrides.
- Add removal criteria for temporary parity bridges.

Progress:

- `OVERRIDE_FOOTPRINT.md` records the generated and manual `xtask report-overrides` snapshot plus
  the remaining naming/metadata limits.
- `OVERRIDE_POLICY.md` documents when text/render overrides are allowed, where they belong, and
  what evidence reviewers should require.
- `xtask report-overrides` now inventories hand-authored `maybe_override_*` raw SVG/path bridge
  functions under `svg/parity` in addition to generated override modules.
- `xtask report-overrides` now counts rows in generated `*_OVERRIDES_*` binary-search tables as
  text metric lookup entries, so generated table debt is tracked separately from hand-curated
  helper constants.
- Redundant public Sankey padding component helpers were collapsed into private constants, leaving
  only the actual `showValues`-aware public padding lookup in the helper footprint.
- Kanban removed a redundant label line-height helper by reusing the existing foreignObject
  height constant, and XYChart collapsed its bar data-label scale helpers into one public helper,
  reducing the hand-curated helper footprint to 81.
- Treemap removed a derived section header center-y helper and now computes it directly from the
  header height, reducing the hand-curated helper footprint to 80.
- Pie collapsed its center point into one public helper for both axes, reducing the hand-curated
  helper footprint to 79.
- Radar removed a redundant legend baseline-y helper and now uses the literal `0.0` directly,
  reducing the hand-curated helper footprint to 78.
- Pie removed two derived legend-position helpers and now computes legend x-offsets from the
  existing rectangle size and spacing constants, reducing the hand-curated helper footprint to 76.
- Pie now derives its label radius from the slice radius in layout, and Treemap reuses the existing
  section inner padding for value inset and label/value gap, reducing the helper footprint to 73.
- Journey now reuses its legend circle radius for text baseline alignment and its viewBox top
  padding for title y-position, reducing the helper footprint to 71.
- Sequence now derives separator self-message y expansion from the existing frame envelope
  expansion, reducing the helper footprint to 70.
- Kanban now reuses its section padding for the item label inset, reducing the helper footprint to
  69.
- Architecture now reuses its service label bottom extension for singleton service text offsets,
  reducing the helper footprint to 68.
- XYChart now uses one shared bar data-label inset helper for both bar orientations, reducing the
  helper footprint to 67.
- CLI render execution now uses internal `RenderRequest` and `RasterRequest` structs so command
  execution keeps its layout, SVG, and raster concerns in one place.
- `xtask report-overrides` now prints category-level owner/source/allowed-use/expected-removal
  metadata for generated override categories and manual raw SVG/path bridge categories.
- `xtask report-overrides --check-no-growth` now enforces explicit category budgets, and
  `xtask verify --strict` includes that override-growth gate.
- Root viewport overrides dropped 34 fixture-specific entries so far: 19 architecture pins after
  topology-driven calibration covered the matching profiles, 4 journey pins after the deterministic
  viewport path covered the matching fixtures, and 11 kanban pins after profile-based root height
  calibration covered the remaining Kanban `parity-root` profiles.
- The current flowchart degenerate path bridge documents its owner and removal criteria near the
  implementation.

Exit criteria:

- Override footprint is reported and tracked.
- Temporary and generated override categories have owners and removal conditions.
- Override table growth fails the strict gate unless the no-growth budget is intentionally updated.
- Architecture root viewport tables keep shrinking as profile-driven viewport code replaces
  fixture-specific pins.
- New model fixes are preferred over new overrides unless browser/font probing proves otherwise.

## M6: Release Readiness

Scope:

- Run broad parity gates.
- Run benchmarks.
- Update README and contributor guidance.
- Decide whether any pre-1.0 public API cleanup should happen before release.

Progress:

- Current-main Criterion baseline captured in
  `docs/performance/spotcheck_2026-05-08_current_main_baseline.md`, covering the pipeline bench
  plus targeted flowchart, architecture, and mindmap stress benches.
- Class namespace-heavy layout cleanup baseline captured in
  `docs/performance/spotcheck_2026-05-08_class_namespace_dense_layout.md`; the pipeline bench now
  includes `class_namespace_dense`.
- Core flowchart/state context cleanup spotcheck captured in
  `docs/performance/spotcheck_2026-05-08_core_context_cleanup.md`, covering the touched
  `flowchart_medium` and `state_medium` pipeline fixtures.
- Focused text measurement cache-readiness spotcheck captured in
  `docs/performance/spotcheck_2026-05-08_text_measure_stress.md`.
- Removed the obsolete `render_layout_svg_parts_for_render_model` compat dispatcher and the
  no-config typed wrappers it served, so typed render-model dispatch now stays on the
  `*_with_config` path.
- Removed the unused no-config class layout entrypoints so class note HTML measurement keeps the
  parser's borrowed `MermaidConfig` on the typed path.
- Completed the flowchart/class/sequence hot-loop clone audit; the remaining clone sites are now
  documented as compatibility, debug, or graphlib key ownership boundaries.

Exit criteria:

- `cargo run -p xtask -- verify --strict`
- `cargo check --workspace --all-features`
- `cargo clippy --workspace --all-targets --all-features -- -D warnings`
- `cargo bench -p merman --features render`
- Workstream TODO has no unresolved P0 items.
- Release notes call out internal cleanup and any public API changes.
