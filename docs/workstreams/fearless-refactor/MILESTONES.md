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
- All feature-gated public code compiles regularly, including no-default, render, and raster
  combinations.
- Clippy stays green for the workspace under the agreed release gate.
- Override growth is visible and justified.

## M0: Refactor Safety Baseline

Status: complete.

Evidence:

- `cargo run -p xtask -- verify --strict` passed. This covers fmt, all-features check, public
  feature matrix, workspace all-target/all-features clippy, nextest, override no-growth, and SVG
  DOM/root parity.

Scope:

- Keep default tests green.
- Keep `--all-features` compilation green.
- Keep the public no-default/render/raster feature matrix green.
- Establish the standard gate list for refactor work.
- Make optional environment-dependent tests robust.

Exit criteria:

- `cargo fmt`
- `cargo check --workspace --all-features`
- `cargo run -p xtask -- verify --feature-matrix`
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

Status: complete for in-tree Mermaid 11.12.3 diagrams; keep the JSON fallback only for `error`
and custom registry parsers.

Scope:

- Move render-critical diagrams from JSON-for-render transport to typed render models.
- Use the first migration to define the repeatable pattern.
- Preserve semantic JSON compatibility APIs while removing render-path JSON construction where the
  renderer has an in-tree typed model.

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
- C4 render-model parsing now bypasses the semantic JSON bridge and converts `C4Db` directly into
  `C4DiagramRenderModel`, which materially reduced the `c4_medium` parse and end-to-end pipeline
  smoke cost.
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
- C4 direct render-model parse cleanup is recorded in
  `docs/performance/spotcheck_2026-05-09_c4_direct_render_model_parse.md`.
- XYChart typed render-path spotcheck is recorded in
  `docs/performance/spotcheck_2026-05-08_xychart_typed_render_model.md`.
- C4/XYChart cross-repo end-to-end comparison evidence is recorded in
  `docs/performance/spotcheck_2026-05-09_c4_xychart_mmdr_comparison.md`, and stage attribution is
  recorded in `docs/performance/spotcheck_2026-05-09_c4_xychart_stage_mmdr.md`.
- Mindmap/Architecture/C4 stage attribution is recorded in
  `docs/performance/spotcheck_2026-05-09_mindmap_architecture_c4_stage_mmdr.md`; Architecture
  layout remains the largest observed stage gap after the C4 parse cleanup.
- Current Mindmap/Architecture same-machine canary pipeline timing is recorded in
  `docs/performance/spotcheck_2026-05-10_mindmap_architecture_canary_pipeline_long.md`; both
  canaries show strong local layout-stage improvement, with `parse/mindmap_medium` still noisy but
  not yet a parser optimization target.
- Architecture layout's legacy JSON compatibility model has been trimmed of unused fields, and the
  dead top-level group separation helper was removed without disturbing DOM parity.
- The final manual raw SVG/path bridge was removed; `xtask report-overrides` now reports zero
  manual bridge files.
- `RENDER_MODEL_INVENTORY.md` now records every non-error in-tree diagram as `typed-first`.
  `RenderSemanticModel::Json` remains intentionally available for the suppressed `error` diagram
  payload and for custom registry parsers that are outside the in-tree Mermaid compatibility set.
- Same-machine baseline capture remains a process requirement for future typed migrations.
- The consolidated typed migration timing index lives in
  `TYPED_MIGRATION_TIMING.md`; append the next migration's baseline pair there before merge.

Exit criteria:

- Every non-error in-tree Mermaid compatibility diagram has a typed render model on the render-only
  path.
- Render-only paths avoid constructing the full semantic JSON model for typed-first diagrams.
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
- Kept shared vendored text measurement free of ER, Mindmap, and Block fixture-specific HTML width
  tables; ER and Block now run their generated lookups only inside their owning diagram modules,
  and the stale Mindmap HTML width table was deleted.
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
- Rebased sequence message cursor startup on the base actor layout height so special participant
  types no longer push the first message down after their post-render bbox adjustments; this
  removed 8 obsolete Sequence root viewport pins and refreshed the related layout goldens.
- Added a shared xtask root viewport delta reporter and wired Sequence compare to
  `--report-root`, giving future Sequence root-pin audits the same upstream/local evidence table
  already used by Flowchart.
- A filtered 2026-05-11 recheck confirmed that `title_and_accdescr_multiline` still needs its
  480px root pin; removing it shifts the root `style` max-width from 480px to 493px and moves
  Bob from `x=305` to `x=318`.
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
- Split Sequence layout block directive cursor-step planning into
  `crates/merman-render/src/sequence/block_steps.rs`, leaving
  `layout_sequence_diagram_typed` to pass an explicit planning context and consume directive
  steps.
- Split the remaining Sequence layout orchestration and directive/rect/note/message handling out
  of `crates/merman-render/src/sequence.rs` into
  `crates/merman-render/src/sequence/orchestration.rs`, leaving
  `layout_sequence_diagram_typed` as the top-level coordinator for actor planning, graph
  assembly, and bounds finalization.
- Centralized the remaining Sequence geometry literals into named owner constants across
  `sequence/block_bounds.rs`, `sequence/block_steps.rs`, `sequence/notes.rs`, and
  `sequence/rect.rs`, so the frame and padding offsets read as policy instead of scattered magic
  numbers.
- Split Sequence layout block frame bounds accumulation into
  `crates/merman-render/src/sequence/block_bounds.rs`, so the final root-bounds expansion pass has
  its own block-stack owner instead of a large inline local state machine.
- Split Sequence layout rect open-stack geometry and final rect horizontal bounds into
  `crates/merman-render/src/sequence/rect.rs`, keeping rect-specific quirks local to one owner.
- Split Sequence layout activation stack state into
  `crates/merman-render/src/sequence/activation.rs`, leaving the main message loop to ask for
  active actor endpoint bounds instead of manipulating activation stacks directly.
- Split Sequence note layout into `crates/merman-render/src/sequence/notes.rs`, so note placement,
  wrapped-note measurement quirks, rect-bounds contribution, and cursor increment live behind one
  focused helper.
- Split regular Sequence message edge layout into
  `crates/merman-render/src/sequence/messages.rs`, so endpoint adjustment, wrapped-message text,
  label measurement, and cursor-step geometry live behind one focused helper while the main loop
  applies actor lifecycle state.
- Split created/destroyed Sequence actor vertical lifecycle state into
  `crates/merman-render/src/sequence/actors.rs`, so Mermaid `starty`/`stopy` anchors, visual-height
  lookup, and cursor bump handling have one owner.
- Moved bottom actor box and lifeline construction into
  `crates/merman-render/src/sequence/actors.rs`, keeping participant footer geometry with the same
  actor lifecycle owner.
- Split Sequence root content/viewBox bounds derivation into
  `crates/merman-render/src/sequence/root_bounds.rs`, so mirror actor, popup panel, boxed
  participant, and self-message sizing quirks have a local owner before further root-pin cleanup.
- Moved top actor box construction into `crates/merman-render/src/sequence/actors.rs`, so top and
  footer actor geometry now share the same module and the obsolete `max_actor_visual_height`
  accumulator is gone.
- Bundled Sequence actor measurement, per-message spacing, box margin calculation, actor box
  membership, and x-coordinate planning into `SequenceActorLayoutPlan`, leaving
  `layout_sequence_diagram_typed` to consume an explicit actor plan instead of owning the spacing
  state machine.
- Re-ran the Sequence disabled-root audit after the actor/root-bounds decomposition; the 80 DOM
  mismatches matched the then-current Sequence root table size, so the remaining Sequence root pins
  stayed classified as active typed-bounds/text-measurement targets.
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
- Moved the remaining Class root viewport pins into typed profile calibration and namespace
  render-mode rules, then deleted the generated Class root override module.
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
- Narrowed the split Flowchart parity context and helper API by deleting unused
  style/class/cluster wrappers and context fields that were only initialized.
- Replaced core flowchart semantic and state layout long-argument helpers with context structs, and
  made `StateDb::add_state` merge `StateStmt` directly; source code no longer carries
  `clippy::too_many_arguments` allows.
- Collapsed Flowchart callback AST actions to the semantic callback flag used by rendering,
  removing the last non-generated `dead_code` allow from the core/render source tree.
- Removed stale core parser helpers left behind by typed pipeline cleanup: `BlockDb` no longer has
  an unused id generator, and Flowchart no longer keeps obsolete collect/merge helpers.
- Extracted architecture SVG root opening, accessibility title/description emission, empty diagram
  fallback sizing, and root viewBox/max-width placeholders into
  `svg/parity/architecture/root.rs`.
- Extracted architecture root viewport finalization, profile calibration, `f32` snapping, and
  generated root override replacement into `svg/parity/architecture/viewport.rs`.
- Class and sequence renderer splits are complete for the current scope: `class/render.rs` and
  `sequence/render.rs` are thin orchestration boundaries over dedicated owner modules.
- Architecture renderer split is now complete for the current scope; keep any follow-up cleanup
  under M5 if future profiling or navigation reveals new dead code.
- Removed local dead ER, GitGraph, and State parity helpers that were no longer called after the
  renderer split and viewport cleanup work.
- Inlined the State viewport mode helper into its two call sites, deleting
  `prefer_fast_state_viewport_bounds` while keeping the strict gate green.
- Collapsed State v2 Dagre input graph construction into one shared builder consumed by the
  production layout path and the debug/xtask comparison helper, deleting the debug-only copy.
- Collapsed duplicated State raw/non-raw context resolution helpers behind shared implementations,
  removed unused wrappers, and narrowed `state_strip_note_group` to file-private visibility.
- Collapsed duplicated State label HTML wrapping and entity-preservation helpers behind shared
  private helpers, leaving the label entry points thin and easier to audit.
- Narrowed the State link sanitizer's internal URL parsing helpers to file-private visibility, so
  only the public allowlist entry point remains exported.
- Moved RoughJS hex parsing and `opsToPath` formatting into a shared parity helper layer consumed
  by both State and Flowchart renderers.
- Moved RoughJS rectangle and circle generation into the shared parity helper layer so the seeded
  shape emission logic no longer forks between State and Flowchart.
- Collapsed repeated Flowchart RoughJS op-set-to-SVG-path serializers into one private helper while
  preserving RoughJS `opsToPath` formatting and call ordering.
- Collapsed repeated Flowchart RoughJS stroke dash parsing into one private helper and narrowed the
  node helper visibility for same-file internals.
- Removed the remaining generated `dead_code` allowances after clippy proved the generated
  override modules no longer need that blanket exception.
- Removed the generated module's remaining `clippy::all` umbrella allowance after the generated
  font-metrics lookup and its `xtask gen-font-metrics` template moved to a normal iterator search,
  so fixture-derived parity data is no longer hidden from `merman-render` clippy coverage.
- Deleted the unused Flowchart `edge_bbox` parity helper module after the active edge path pipeline
  fully moved into `edge_geom` and root SVG emission.
- Deleted the obsolete Flowchart straight-except-one-endpoint basis helper after full flowchart DOM
  parity stayed green without the special case.
- Inlined the single-use flowchart basis midpoint helper into the edge path builder, deleting
  `maybe_insert_midpoint_for_basis` after flowchart DOM parity and the strict gate stayed green.
- Removed unused no-bounds D3 curve wrappers, leaving `curve.rs` with only active path-and-bounds
  entrypoints plus the still-used basis/linear path helpers.
- Removed the remaining dead xtask debug helpers and stale scratch structs after equivalent
  commands existed; the state SVG analyzer, font-metrics generator, and SVG override generator no
  longer keep dead helper code around.

Exit criteria:

- No single parity renderer file remains difficult to navigate because of unrelated concerns.
- Diagram-specific tests still cover layout, SVG, and entity-sanitization behavior.
- No DOM parity regressions for the touched diagram families.

## M5: Override Governance and Debt Reduction

Status: complete for the current release; remaining override entries are documented derivation or
measurement targets, not known-obsolete cleanup candidates.

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
- The stale Mindmap HTML width override table and `gen-mindmap-text-overrides` command were
  deleted after removing shared text-measurer leakage proved the stable Mindmap layout path did
  not need those 291 generated lookup entries.
- The obsolete `gen-er-text-overrides` command and generator were deleted after ER text lookup debt
  shrank to a three-entry hand-curated guard file; the empty ER `calcTextWidth` lookup path was
  removed from the renderer.
- C4 moved its three stable SVG bbox line-height rules into the C4 owner module and deleted the
  generated `c4_text_overrides_11_12_2.rs` module. C4 also moved its 17 type-line `textLength`
  pins into the owner module and deleted the generated `c4_type_textlength_11_12_2.rs` module,
  so the type-line `textLength` logic now lives only in owner code.
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
- Pie now derives its outer radius from the slice radius, Sequence now derives its note padding
  total from the existing note gap, Journey inlines its single-use legend placement and mouth
  offset values, and Radar inlines its remaining legend box size and label x-offset literals.
  Pie now inlines its fixed margin, center, radius, legend label font size, title y, and legend
  text y literals while keeping only the shared legend rectangle size and spacing helpers.
  XYChart now inlines its bar data-label scale and inset literals, deleting the empty generated
  override module. Treemap now inlines its section header label/value sizing literals, leaving only
  the shared section spacing helpers and leaf-fit tolerance. Sequence now inlines its self-only
  frame min pad literals in block geometry. Sankey now inlines its SVG-only label font/gap/dy
  literals, leaving only node geometry and padding helpers. Architecture also deleted a dead icon
  text bbox helper. Radar now inlines its final legend row spacing value and deletes the now-empty
  generated module. Pie moved its remaining legend rectangle/spacing values into `pie` owner
  constants and deleted the now-empty generated module. Sankey moved its remaining node
  width/padding values into `sankey` owner constants and deleted the now-empty generated module,
  reducing the helper footprint to 32. Journey moved its fixed viewBox/title/legend/face geometry
  into `journey` owner constants and deleted the now-empty generated module, reducing the helper
  footprint to 26. Kanban moved its section padding, label foreignObject height, and item row
  heights into `kanban` owner constants and deleted the now-empty generated module, reducing the
  helper footprint to 21. Treemap moved its section spacing geometry into `treemap` owner constants
  and kept the remaining `Item A1` leaf-fit browser tolerance beside the SVG parity loop, deleting
  the now-empty generated module and reducing the helper footprint to 18. Sequence moved its note
  wrap slack, text line-height math, and frame padding geometry into `sequence` owner
  constants/functions and deleted the now-empty generated module, reducing the helper footprint to
  12. Architecture moved its text bbox formulas, canvas-label width scale, service label extension,
  and default wrap width into `architecture` owner constants/functions and deleted the now-empty
  generated module, reducing the helper footprint to 6. Gitgraph moved branch-label correction
  control flow into the `gitgraph` owner module and reclassified the remaining bbox correction data
  as text metric lookup entries, reducing the helper footprint to 0 while preserving measured-data
  visibility in override reporting. A later GitGraph branch-label pass deleted the 7-entry
  branch-label bbox correction table after raw measured widths rounded to 1/64px preserved GitGraph
  DOM parity. A later GitGraph commit-label pass deleted the 3-entry literal extra table after the
  rounded measured widths and existing edge-character corrections still preserved GitGraph DOM
  parity. A later GitGraph glyph pass removed the left-side `2`, `6`, `5`, `C`, and `B`
  corrections after the smaller correction table stayed green under DOM parity.
  A subsequent GitGraph glyph pass removed the right-side `C`, `D`, `B`, `0`, `6`, `4`, `a`, and
  `d` corrections after the even smaller correction table stayed green under DOM parity.
- CLI render execution now uses internal `RenderRequest` and `RasterRequest` structs so command
  execution keeps its layout, SVG, and raster concerns in one place.
- `xtask report-overrides` now prints category-level owner/source/allowed-use/expected-removal
  metadata for generated override categories and manual raw SVG/path bridge categories, including
  zero-count categories.
- `xtask report-overrides` now counts restricted-visibility helper functions, so changing helper
  visibility cannot hide hand-curated helper footprint.
- C4 and XYChart now have current exact pipeline bench smoke coverage in
  `docs/performance/spotcheck_2026-05-09_c4_xychart_pipeline_bench_smoke.md`.
- `crates/merman/tests/pipeline_bench_fixtures.rs` now guards all pipeline bench fixtures against
  parse/layout/render pre-check skips under the `render` feature.
- `xtask report-overrides --check-no-growth` now enforces explicit category budgets, and
  `xtask verify --strict` includes that override-growth gate.
- Class text lookup debt dropped by 69 entries after the exact deterministic fallback pass, the
  `uses` plain-label cleanup, the `OK` pair cleanup, the `ApiClient` cleanup with dense layout
  golden refresh, and the later `ERROR`, `Payment`, `Cart`, `Server` rendered-width, `Dog`, and
  `Mineral` calc cleanups, followed by the `Duck`, `Item`, `Order`, `Wheel`, `connects`, and
  `builds`, `parses`, `emits`, `feedback`, `returns`, `wraps`, `reads`, `depends`, `owns`, and
  `may-fail`, `references`, `int chimp`, `int gorilla`, `+int age`, `int id`, `int[] id`,
  `+eat()`, `+mate()`, `+run()`, `+quack()`, `+swim()`, `+template()`, `bar()`, and
  `+isOk() : bool`, `+logout()`, `+start()`, and `+addUser(user: User)` rendered-width cleanups
  stayed green under Class DOM parity, layout snapshot, and strict gates. The
  `+handle(req: Request) : Response`, `+query(sql: String) : Rows`, and
  `+request() : Response` rendered-width guards were restored after a `parity-root` recheck showed
  `stress_class_styles_multiple_classdef_016` root max-width drifting from `890.25px` to
  `890.5px` without them, leaving the global text lookup total at 480.
- Root viewport footprint is now tracked at `760` entries after the broad pruning work and the
  later restoration of ten guards required by a full `parity-root` release-gate sweep: 19
  architecture pins after topology-driven calibration covered the matching profiles, most journey
  pins after the deterministic viewport path covered the matching fixtures, and 11 kanban pins
  after profile-based root height
  calibration covered the remaining Kanban `parity-root` profiles, plus 4 sankey pins now covered
  by deterministic emitted bounds, 9 timeline pins now covered by deterministic root output, and 12
  pie pins now covered by deterministic root output, plus 12 ER pins now covered by deterministic
  root output, 35 requirement pins now covered by deterministic root output, and 16 C4 pins now
  covered by deterministic root output. The Block root override table was then deleted after all
  119 entries proved obsolete under `parity-root`, followed by 68 State pins now covered by
  deterministic root output. The Class root table then dropped 166 obsolete pins and gained one
  missing docs root pin, making Class `parity-root` green with a 165-entry net reduction. Gitgraph
  then dropped 6 obsolete pins while staying green under `parity-root`, and Sequence dropped 32
  obsolete pins while staying green under `parity-root`. Flowchart then dropped 131 obsolete pins
  and later cleared `upstream_docs_math_flowcharts_001` by normalizing the browser-sensitive math
  baseline and measuring sanitized KaTeX MathML through the Node probe, so Flowchart `parity-root`
  is green without root override growth. Pie then replaced its 23 remaining root pins with a typed
  empty-pie root viewport rule plus shared 1/64px-quantized legend SVG bbox measurement, deleting
  the Pie root override module. Mindmap then refreshed typed root viewport profile calibration,
  added two small model-derived root profiles, and pruned 28 obsolete root pins while keeping
  `parity-root` green. Class then moved its remaining 31 root viewport pins into typed profile
  calibration and namespace render-mode rules, deleting the Class root override module while
  keeping `parity-root` green. Architecture then added default root viewport calibration for
  nested-groups and reasonable-height profiles and pruned 70 obsolete fixture-scoped pins, leaving
  31 Architecture root pins that still guard real `parity-root` drift. Empty-diagram root viewport
  behavior then moved into Flowchart, State, ER, and Requirement renderer logic, deleting 21 more
  root pins while the affected normal and `parity-root` DOM filters stayed green. The full
  `parity-root` sweep then restored 6 Sequence, 2 Journey, and 2 GitGraph root guards so
  `compare-all-svgs --check-dom --dom-mode parity-root --dom-decimals 3` is green.
- The five largest remaining root buckets now have complete disabled-root audit evidence:
  GitGraph, Flowchart, Sequence, Mindmap, and State all still show broad `max-width` or viewBox
  drift without the lookup tables. These buckets remain typed bounds or measurement derivation
  targets, not blind-pruning targets.
- A later GitGraph disabled-root mismatch cross-check removed two stale retained root pins
  (`upstream_cypress_gitgraph_spec_88_should_hide_branches_with_tb_orientation_when_showbranches_is_092`
  and `upstream_direction_bt`) while focused/full GitGraph `parity-root` stayed green. The current
  root viewport total is `616`, with `226` GitGraph entries, and the no-growth budget is tightened
  to `616`.
- A follow-up GitGraph title-bounds pass included `gitTitleText` in emitted root bbox derivation,
  removed 13 title-dominated GitGraph root pins, and tightened the current root viewport total to
  `603`, with `213` GitGraph entries. A disabled-root cross-check now shows all retained GitGraph
  pins still correspond to DOM mismatches.
- A follow-up Sequence frontmatter-title pass propagated metadata titles into layout root bounds,
  removed `upstream_html_demos_sequence_sequence_diagram_demos_002`, and tightened the current root
  viewport total to `602`, with `79` Sequence entries.
- A follow-up GitGraph branch-line endpoint pass included renderer-owned branch line endpoints in
  root bbox derivation, collapsing the empty-graph package bucket from roughly `+34.750px`
  disabled-root width drift to sub-pixel branch-label drift. The GitGraph table remains at `213`
  entries because the disabled-root mismatch set still exactly matches the retained pins.
- The obsolete flowchart degenerate path helper remains in place after strict-gate rechecks without
  it produced DOM mismatches on subgraph-descendant flowchart fixtures.
- The redundant flowchart cluster-run helper remains in place after strict-gate rechecks without
  it produced the same cluster/subgraph flowchart DOM mismatches.
- The flowchart cyclic-special basis helper was deleted after strict-gate rechecks stayed green
  without it, removing `maybe_pad_cyclic_special_basis_route` from
  `svg/parity/flowchart/edge_geom/basis.rs`.
- The C4 root viewport overrides were rechecked by bypassing the lookup entirely; all 35 entries
  still drift, so the C4 table remains in place for now.
- The Timeline root viewport overrides were rechecked by bypassing the lookup entirely; the 9-entry
  table still guards stress/cypress `parity-root` drift, so it remains in place for now.
- The State root viewport overrides were rechecked by bypassing the lookup entirely before the
  empty-diagram cleanup; the remaining 45-entry table still guards broad stress/cypress
  `parity-root` drift, so it remains in place for now.

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
- Recorded the XYChart render allocation cleanup in
  `docs/performance/spotcheck_2026-05-09_xychart_render_allocation_cleanup.md`; the local
  Criterion smoke shows `render/xychart_medium` at `113.74-122.92 us` after the SVG renderer
  stopped building per-node `BTreeMap` tables and temporary CSS strings.
- Recorded the XYChart layout tick-cache cleanup in
  `docs/performance/spotcheck_2026-05-09_xychart_layout_tick_cache.md`; the local Criterion smoke
  shows `layout/xychart_medium` at `55.129-60.551 us` after tick labels moved from repeated
  generation to axis-state reuse.
- Revalidated the full package benchmark gate with `cargo bench -p merman --features render`;
  the run completed successfully after a longer timeout window and is recorded in
  `docs/performance/spotcheck_2026-05-10_full_bench_gate.md`.
- Revalidated `cargo run -p xtask -- verify --strict` after the Class text lookup cleanup; the run
  covered fmt, all-features check, workspace clippy, no-growth override reporting at `480` text
  lookup entries, feature matrix checks, workspace nextest, and strict normal SVG DOM parity.
- `xtask verify --strict` now also includes the full SVG root parity sweep after normal DOM parity.
- Revalidated `cargo run -p xtask -- verify --strict` after adding full root parity to the strict
  gate; the run passed with `760` root viewport entries, `480` text lookup entries, `1013` nextest
  tests passed, `3` skipped, and both normal DOM parity plus root parity green.
- Revalidated `cargo bench -p merman --features render` after the Class text lookup cleanup; the
  full bench gate completed under a longer timeout window and the representative estimates are
  recorded in `docs/performance/spotcheck_2026-05-11_full_bench_gate_after_class_cleanup.md`.
- Revalidated `cargo run -p xtask -- verify --strict` after the M5 closeout and root-delta report
  parser hardening; the run passed with `760` root viewport entries, `480` text lookup entries,
  `1016` nextest tests passed, `3` skipped, and both normal DOM parity plus root parity green.

Exit criteria:

- `cargo run -p xtask -- verify --strict`
- `cargo check --workspace --all-features`
- `cargo clippy --workspace --all-targets --all-features -- -D warnings`
- `cargo bench -p merman --features render`
- Workstream TODO has no unresolved P0 items.
- Release notes call out internal cleanup and any public API changes.
