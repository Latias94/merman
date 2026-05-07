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
- [ ] Move gantt or kanban render path toward a typed render model after sequence.
  Selection rule: choose the diagram with better test coverage and less parser churn.
- [ ] Add parse/render timing samples before and after each typed migration.
  Gate: `MERMAN_PARSE_TIMING=1` plus targeted render benchmarks.
  Sequence status: post-migration baseline captured in
  `docs/performance/spotcheck_2026-05-07_sequence_typed_render_model.md`; keep this item open so
  the next typed migration captures a same-machine pre-migration baseline first.

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
  - `text/overrides.rs` (text override lookup boundary started)
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
- [x] Document when a text width override is allowed.
  Evidence: `OVERRIDE_POLICY.md` records allowed sources, disallowed shortcuts, placement rules,
  evidence checklist, and review questions.

## P1: Renderer Boundary Cleanup

- [ ] Split `svg/parity/class/render.rs`.
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
- [ ] Split `svg/parity/sequence/render.rs`.
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
- [ ] Split `svg/parity/architecture.rs`.
  Proposed boundaries:
  - typed model extraction (JSON and typed render-model access now live in
    `architecture/model.rs`)
  - service/group layout emission (group rectangle recursion and shared bounds helpers now live in
    `architecture/geometry.rs`)
  - edge rendering (direction, arrow, and edge-id helpers now live in `architecture/geometry.rs`)
  - icon/text XHTML normalization (foreignObject XHTML normalization and entity-safe ampersand
    escaping now live in `architecture/foreign_object.rs`; built-in icon SVG bodies now live in
    `architecture/icons.rs`; SVG label wrapping/text emission now lives in
    `architecture/labels.rs`)
  - root/viewBox handling (SVG root opening, accessibility title/description emission, empty
    diagram fallback sizing, and root viewBox/max-width placeholders now live in
    `architecture/root.rs`; root viewport calibration still needs a dedicated owner)
  - CSS/theme emission
- [ ] Prefer small render context structs over long parameter lists.
  Progress: sequence block frame helpers now share `SequenceBlockRenderContext`; keep open for
  remaining renderer helpers with repeated long argument lists.
- [ ] Remove dead debug helpers once equivalent `xtask` commands exist.

## P2: Override Hygiene

- [x] Run and record override footprint.
  Command: `cargo run -p xtask -- report-overrides`.
  Evidence: `OVERRIDE_FOOTPRINT.md`.
- [ ] Classify overrides by category:
  - generated text metrics
  - root viewport
  - raw SVG/path precision
  - temporary parity bridge
  Status: `xtask report-overrides` now scans all generated override modules by category; keep open
  until temporary raw SVG/path bridges have owners and removal criteria.
- [ ] Add comments or metadata for temporary overrides with removal criteria.
- [ ] Delete overrides made obsolete by typed model or measurement fixes.
- [ ] Prevent override tables from becoming the default fix for model bugs.

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
- [x] Profile JSON clone cost in `layout_parsed` and public wrapper APIs.
  Evidence: `JSON_CLONE_AUDIT.md`. Decision: `layout_parsed` keeps the owned semantic clone for the
  compatibility API; public `render_svg_sync` already uses typed layout/render dispatch and avoids
  owned semantic JSON.
- [ ] Reduce repeated `serde_json::Value` cloning in layout/render-only paths.
  Progress: class typed/config layout and render now keep `&MermaidConfig` through note HTML
  measurement/sanitization, and sequence SVG rendering no longer clones the typed render model for
  title fallback. Remaining clones are mostly legacy `&Value` compatibility bridges or
  diagram-specific sanitize/config wrappers that still need ownership review.
- [ ] Audit hot loops for avoidable string cloning in flowchart/class/sequence renderers.
  Progress: `HOT_LOOP_CLONE_AUDIT.md` records the first pass. Flowchart layout now borrows normal
  edges in the self-loop expansion stage, and layout/SVG render share an explicit helper-edge
  constructor that clones only retained source fields. Sequence block collection now borrows block
  labels and message ids from the typed render model; sequence activation planning now borrows
  active message and actor ids; non-wrapped sequence actor/message/note labels now render borrowed
  `<br>` split lines. Class edge rendering now reuses sorted edge order, reuses marker-adjusted
  point buffers, and borrows edge ids for edge-label center lookup.
- [ ] Add focused benchmarks before optimizing text measurement caches.

## P3: Public API and CLI Cleanup

- [ ] Review public `merman::render` API after typed render migration.
- [ ] Keep synchronous executor-free API as the default path.
- [ ] Decide whether async wrappers should remain simple aliases or be feature-gated later.
- [ ] Audit CLI option parsing for duplicated raster branches.
- [ ] Consider a small internal `RenderRequest`/`RasterRequest` struct for CLI command execution.

## P3: Documentation Cleanup

- [ ] Update README architecture notes after pipeline cleanup.
- [ ] Add a short contributor guide for adding a new typed diagram renderer.
- [ ] Document standard gates for parity, refactor, and release work.
- [ ] Document what "Mermaid parity" means for generated override data.
