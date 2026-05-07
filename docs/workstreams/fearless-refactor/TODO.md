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
- [ ] Add a documented "fast local refactor gate" command set.
  Gap check: confirm which nextest packages and snapshot gates give the best signal per minute.
- [ ] Audit feature flags and remove or document stale experimental flags.
  Candidates: `flowchart_root_pack`.

## P1: Typed Render Pipeline Cleanup

- [ ] Inventory all diagrams by render model mode.
  Output: table of `typed`, `JSON-for-render`, and `JSON-only` diagrams.
- [ ] Remove duplicate error-diagram construction paths in `Engine`.
  Direction: centralize suppressed-error model construction for JSON and typed render models.
- [ ] Decide the future of `parse_diagram_for_render_sync`.
  Options:
  - Keep as compatibility-only and route all new render code through typed models.
  - Deprecate after public wrapper APIs no longer use it.
- [ ] Move sequence diagram render path toward a typed render model.
  Rationale: sequence has a large renderer and frequent layout/render coupling.
- [ ] Move gantt or kanban render path toward a typed render model after sequence.
  Selection rule: choose the diagram with better test coverage and less parser churn.
- [ ] Add parse/render timing samples before and after each typed migration.
  Gate: `MERMAN_PARSE_TIMING=1` plus targeted render benchmarks.

## P1: Text and Measurement Module Split

- [ ] Split `crates/merman-render/src/text.rs` by responsibility.
  Proposed modules:
  - `text/metrics.rs`
  - `text/html_label.rs`
  - `text/markdown.rs`
  - `text/svg_text.rs`
  - `text/overrides.rs`
  - `text/font_metrics.rs`
- [ ] Keep public re-exports stable from `text.rs` or `text/mod.rs`.
- [ ] Move markdown-only tests next to markdown code.
- [ ] Move override lookup tests next to override code.
- [ ] Separate "browser compatibility measurement" from "deterministic fallback measurement".
- [ ] Document when a text width override is allowed.
  Rule: override only after a fixture/probe proves upstream browser/font behavior.

## P1: Renderer Boundary Cleanup

- [ ] Split `svg/parity/class/render.rs`.
  Proposed boundaries:
  - render context and ids
  - class box geometry
  - relation paths and labels
  - note rendering
  - namespace/subgraph rendering
  - debug SVG helpers
- [ ] Split `svg/parity/sequence/render.rs`.
  Proposed boundaries:
  - actors and participants
  - messages
  - notes
  - loops/alt/par/rect blocks
  - activation rendering
  - viewport/bounds
- [ ] Split `svg/parity/architecture.rs`.
  Proposed boundaries:
  - typed model extraction
  - service/group layout emission
  - edge rendering
  - icon/text XHTML normalization
  - CSS/theme emission
- [ ] Prefer small render context structs over long parameter lists.
- [ ] Remove dead debug helpers once equivalent `xtask` commands exist.

## P2: Override Hygiene

- [ ] Run and record override footprint.
  Command: `cargo run -p xtask -- report-overrides`.
- [ ] Classify overrides by category:
  - generated text metrics
  - root viewport
  - raw SVG/path precision
  - temporary parity bridge
- [ ] Add comments or metadata for temporary overrides with removal criteria.
- [ ] Delete overrides made obsolete by typed model or measurement fixes.
- [ ] Prevent override tables from becoming the default fix for model bugs.

## P2: Performance and Allocation

- [ ] Establish baseline benchmark numbers for current `main`.
  Commands:
  - `cargo bench -p merman --features render`
  - targeted flowchart/architecture/mindmap benches
- [ ] Profile JSON clone cost in `layout_parsed` and public wrapper APIs.
- [ ] Reduce repeated `serde_json::Value` cloning in render-only paths.
- [ ] Audit hot loops for avoidable string cloning in flowchart/class/sequence renderers.
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
