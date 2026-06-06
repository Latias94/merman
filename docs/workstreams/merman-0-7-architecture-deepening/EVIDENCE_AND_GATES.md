# Merman 0.7 Architecture Deepening — Evidence And Gates

Status: Active
Last updated: 2026-06-06

## Smallest Current Repro

The latest completed implementation slice is M07A-100:

```bash
cargo nextest run -p merman-core flowchart
cargo nextest run -p merman-core
cargo nextest run -p merman-render flowchart
cargo run -p xtask -- compare-flowchart-svgs --check-dom --dom-decimals 3 --filter basic
cargo fmt --all --check
git diff --check
```

## Gate Set

### Documentation Gate

```bash
git diff --check -- CONTEXT.md docs/workstreams/merman-0-7-architecture-deepening
```

Proves the workstream docs and root context update have no whitespace errors.

### Render Operation Iteration Gate

```bash
cargo test -p merman --no-run
cargo nextest run -p merman-bindings-core render_svg
cargo nextest run -p merman-ffi render_svg
```

Proves the public Rust facade still compiles and downstream render adapters preserve current
SVG/pipeline behavior after canonical operation refactors. `merman` currently has no nextest-run
unit tests, so `cargo test -p merman --no-run` is the direct facade compilation gate for M07A-020.

### Adapter Gate

```bash
cargo nextest run -p merman-bindings-core render_svg
cargo nextest run -p merman-cli render
cargo nextest run -p merman --features raster raster
cargo nextest run -p merman-ffi
```

Proves adapters still preserve protocol, error, and option behavior when migrated.

### Headless Output Size Gate

```bash
cargo nextest run -p merman --features raster svg_to_pdf
cargo nextest run -p merman-cli pdf
```

Proves PNG/JPG/PDF-facing conversion paths keep explicit size-budget behavior and do not let PDF
fit/page options bypass intrinsic SVG complexity limits.

### Core Facts Gate

```bash
cargo nextest run -p merman-core detect
cargo nextest run -p merman-core registry
cargo nextest run -p merman-bindings-core metadata
cargo run -p xtask -- check-alignment
```

Proves detector, parser registry, supported diagram metadata, and alignment projections agree.

### SVG Parity Gate

Use targeted family gates during development:

```bash
cargo run -p xtask -- compare-flowchart-svgs --check-dom --dom-decimals 3 --filter <fixture>
cargo run -p xtask -- compare-class-svgs --check-dom --dom-decimals 3 --filter <fixture>
```

Use the root gate when root SVG, viewport, emitted bounds, or overrides change:

```bash
cargo run -p xtask -- compare-all-svgs --check-dom --dom-mode parity-root --dom-decimals 3
```

Proves renderer changes do not hide semantic drift behind broad normalization.

### Semantic Ownership Gate

```bash
cargo nextest run -p merman-core sanitize
cargo nextest run -p merman-core flowchart
cargo nextest run -p merman-render
```

Proves family-owned sanitization and typed/JSON projection changes preserve semantic and render
behavior.

### Closeout Gate

```bash
cargo fmt --all --check
cargo nextest run --workspace
cargo run -p xtask -- check-alignment
```

Add the SVG parity gate that matches the touched surface. If the full gate is too expensive for a
task closeout, record the narrowed gate and why it is sufficient. Fresh verification is required
before marking a task, Codex goal, or lane complete.

## Evidence Anchors

- `docs/workstreams/merman-0-7-architecture-deepening/DESIGN.md`
- `docs/workstreams/merman-0-7-architecture-deepening/TODO.md`
- `docs/workstreams/merman-0-7-architecture-deepening/TASKS.jsonl`
- `docs/workstreams/merman-0-7-architecture-deepening/CAMPAIGNS.jsonl`
- `docs/workstreams/merman-0-7-architecture-deepening/MILESTONES.md`
- `docs/workstreams/merman-0-7-architecture-deepening/WORKSTREAM.json`
- `CONTEXT.md`
- implementation tests and parity compare outputs recorded in this file as tasks land

## Current Evidence

- 2026-06-06: Workstream opened from the read-only architecture review. No Rust code changed yet.
- 2026-06-06: M07A-020 introduced `crates/merman/src/render/operation.rs` and routed
  `render_svg_sync` / `render_svg_with_pipeline_sync` through it. Validation passed:
  `cargo test -p merman --no-run`; `cargo nextest run -p merman-bindings-core render_svg`;
  `cargo nextest run -p merman-ffi render_svg`; `cargo fmt --all --check`.
  Attempted `cargo nextest run -p merman render_svg` and
  `cargo nextest run -p merman render::svg_pipeline_tests`, but both matched zero tests and are not
  counted as pass evidence.
- 2026-06-06: M07A-030 migrated CLI Mermaid input rendering to call
  `render_svg_with_pipeline_sync` with the CLI-owned postprocess pipeline. Existing SVG input keeps
  CLI-owned postprocessing because it is a format Adapter path, not Mermaid render flow. Bindings-core
  and raster already used operation-backed facade helpers after M07A-020. Validation passed:
  `cargo nextest run -p merman-bindings-core render_svg`; `cargo nextest run -p merman-cli render`;
  `cargo nextest run -p merman --features raster raster`; `cargo fmt --all --check`.
- 2026-06-06: M07A-040 removed shallow `HeadlessRenderer` convenience methods that only cloned
  `SvgRenderOptions` to override `diagram_id` or passed one-shot `SvgRenderOptions` through
  existing helpers. Repo/docs search found no callers and no README/bindings docs commitments for
  the removed methods. Kept documented readable/resvg-safe preset helpers. Validation passed:
  `cargo test -p merman --no-run`; `cargo nextest run -p merman-bindings-core`;
  `cargo nextest run -p merman-ffi`; `cargo fmt --all --check`.
  `cargo nextest run -p merman` still matched zero tests and is not counted as pass evidence.
- 2026-06-06: M07A-050 introduced `crates/merman-core/src/family.rs` as the core Diagram Family
  Facts projection source. Detector order, fast detect profile behavior, semantic parser registry,
  typed render parser registry, known-type detector side effects, `RenderSemanticModel` alias
  support, and bindings supported diagram metadata now project from those facts. Validation passed:
  `cargo nextest run -p merman-core detect`; `cargo nextest run -p merman-core registry`;
  `cargo nextest run -p merman-bindings-core metadata`;
  `cargo nextest run -p merman-core --no-default-features detect`; `cargo fmt --all --check`.
- 2026-06-06: M07A-060 introduced `crates/xtask/src/cmd/admission.rs` and
  `docs/alignment/ADMISSION_INVENTORY.md`. The inventory records admission state, fixture corpus
  state, semantic/layout/SVG/root coverage, compare-command ownership, owner docs, and defer
  reasons. `compare-all-svgs` now reads its primary matrix and root-deferred diagram projection
  from the inventory; `check-alignment` validates inventory paths and evidence. Validation passed:
  `cargo run -p xtask -- check-alignment`;
  `cargo run -p xtask -- compare-tree-view-svgs --check-dom --dom-mode parity --dom-decimals 3 --filter upstream_docs_treeview_basic`;
  `cargo run -p xtask -- compare-all-svgs --diagram treeView --check-dom --dom-mode parity --dom-decimals 3 --filter upstream_docs_treeview_basic`;
  `cargo fmt --all --check`.
- 2026-06-06: M07A-070 introduced shared root viewport planning in
  `crates/merman-render/src/svg/parity/root_svg.rs`. `RootViewportPlan` now owns the canonical
  root `viewBox`, `width`/`height`, and responsive `style` emission rules for the proof slice, with
  root override precedence covered by unit tests. `treeView` now emits root SVG viewport attrs
  through that plan while retaining its family-owned layout bounds and theme CSS. Validation passed:
  `cargo nextest run -p merman-render root_viewport_plan`;
  `cargo nextest run -p merman-render tree_view`;
  `cargo run -p xtask -- compare-tree-view-svgs --check-dom --dom-mode parity --dom-decimals 3 --filter upstream_docs_treeview_basic`;
  `cargo run -p xtask -- compare-tree-view-svgs --check-dom --dom-mode parity-root --dom-decimals 3 --filter upstream_docs_treeview_basic`;
  `cargo run -p xtask -- compare-all-svgs --diagram treeView --check-dom --dom-mode parity-root --dom-decimals 3 --filter upstream_docs_treeview_basic`;
  `cargo fmt --all --check`.
- 2026-06-06: M07A-075 closed a PDF output size-budget gap found during headless boundary audit.
  `svg_to_pdf` now validates default intrinsic SVG size limits, `svg_to_pdf_with_options` shares the
  same validation, and the CLI PDF branch validates the original SVG before any PDF wrapper page is
  applied. `--pdfFit`, scale, and page wrapping are not treated as complexity reducers; trusted
  oversized export must use explicit unbounded raster options. Validation passed:
  `cargo nextest run -p merman --features raster svg_to_pdf`;
  `cargo nextest run -p merman-cli pdf`;
  `cargo fmt --all --check`.
- 2026-06-06: Verification refresh after adding CLI PDF regression coverage and reconciling
  workstream docs. Validation passed:
  `cargo nextest run -p merman --features raster svg_to_pdf`;
  `cargo nextest run -p merman-cli pdf`;
  `cargo nextest run -p merman-render root_viewport_plan`;
  `cargo nextest run -p merman-render tree_view`;
  `cargo nextest run -p merman-core tree_view`;
  `cargo run -p xtask -- compare-tree-view-svgs --check-dom --dom-mode parity --dom-decimals 3 --filter upstream_docs_treeview_basic`;
  `cargo run -p xtask -- compare-tree-view-svgs --check-dom --dom-mode parity-root --dom-decimals 3 --filter upstream_docs_treeview_basic`;
  `cargo run -p xtask -- compare-all-svgs --diagram treeView --check-dom --dom-mode parity-root --dom-decimals 3 --filter upstream_docs_treeview_basic`;
  `cargo fmt --all --check`;
  `git diff --check -- CONTEXT.md docs/workstreams/merman-0-7-architecture-deepening crates/merman-cli/tests/pdf_smoke.rs crates/merman-cli/src/render.rs crates/merman/src/render/raster.rs crates/merman-core/src/diagrams/tree_view.rs crates/merman-render`;
  `jq -c . docs/workstreams/merman-0-7-architecture-deepening/TASKS.jsonl`;
  `jq -e . docs/workstreams/merman-0-7-architecture-deepening/WORKSTREAM.json`;
  `cargo run -p xtask -- check-alignment`.
  Broader `cargo nextest run --workspace` was not run because this closeout is for scoped M07A-070
  and M07A-075 slices, and the touched behavior is covered by targeted package, adapter, and parity
  gates above.
- 2026-06-06: M07A-076 split raw SVG raster/PDF input from Mermaid-generated SVG postprocessing.
  Raw SVG input now starts from `SvgPipeline::resvg_safe()` before CLI background/CSS
  postprocessors, while Mermaid source rendering keeps the parity pipeline and the final
  raster/PDF conversion still applies the normal resvg-safe cleanup and size limits. The CLI README
  now documents raw SVG as trusted input rather than a general-purpose untrusted-SVG sanitizer.
  Validation passed:
  `cargo nextest run -p merman-cli raw_svg`;
  `cargo nextest run -p merman-cli png`;
  `cargo nextest run -p merman-cli pdf`;
  `cargo run -p xtask -- check-alignment`;
  `cargo fmt --all --check`.
- 2026-06-06: M07A-077 exposed fixed local-time controls through the CLI and aligned typed
  render-model parsing with semantic JSON parsing. CLI parse/render/top-level modes now accept
  `--fixed-today` and `--fixed-local-offset-minutes`; `Engine::parse_diagram_for_render_model_sync`
  now applies `Engine::with_fixed_today` and `Engine::with_fixed_local_offset_minutes` while
  parsing typed render models, closing the Gantt SVG/render determinism gap. Validation passed:
  `cargo nextest run -p merman-core gantt`;
  `cargo nextest run -p merman-cli fixed`;
  `cargo nextest run -p merman-cli`;
  `cargo fmt --all --check`;
  `cargo run -p xtask -- check-alignment`;
  `git diff --check`;
  `jq -c . docs/workstreams/merman-0-7-architecture-deepening/TASKS.jsonl >/dev/null`;
  `jq -e . docs/workstreams/merman-0-7-architecture-deepening/WORKSTREAM.json >/dev/null`.
  Broader `cargo nextest run --workspace` was not run because this slice touched core Gantt
  fixed-time handling plus CLI adapter exposure; the Gantt family suite and full `merman-cli`
  package gate cover the changed behavior.
- 2026-06-06: M07A-078 exposed fixed local-time controls through the shared binding
  `options_json` contract and Rust headless renderer facades. `HeadlessRenderer` and
  `HeadlessAsciiRenderer` now have fixed today/offset builders; bindings-core parses top-level
  `fixed_today` and `fixed_local_offset_minutes`, validates their ranges, and applies them to the
  shared renderer construction used by stateless and cached render engines. This lets existing C,
  UniFFI, WASM, Python, Android, Apple, Flutter, and Web paths inherit fixed-time controls without
  ABI growth. Validation passed:
  `cargo nextest run -p merman-bindings-core parse_json_accepts_fixed_time_options` first failed
  as RED evidence before implementation;
  `cargo nextest run -p merman-bindings-core`;
  `cargo nextest run -p merman-bindings-core --features ascii`;
  `cargo nextest run -p merman --features render headless_renderer_fixed_time_controls_semantic_parse`;
  `cargo nextest run -p merman --features ascii headless_ascii_renderer_fixed_time_controls_semantic_parse`;
  `npm run build:ts --prefix platforms/web`;
  `cargo run -p xtask -- check-alignment`;
  `cargo fmt --all --check`;
  `git diff --check`;
  `jq -c . docs/workstreams/merman-0-7-architecture-deepening/TASKS.jsonl >/dev/null`;
  `jq -e . docs/workstreams/merman-0-7-architecture-deepening/WORKSTREAM.json >/dev/null`.
- 2026-06-06: M07A-079 aligned Flowchart numeric zero spacing with pinned Mermaid source.
  `repo-ref/mermaid/packages/mermaid/src/diagrams/flowchart/flowRenderer-v3-unified.ts` sets
  `data4Layout.nodeSpacing = conf?.nodeSpacing || 50` and
  `data4Layout.rankSpacing = conf?.rankSpacing || 50`, so JSON numeric `0` falls back to the
  default spacing. The same source keeps `diagramPadding` on `?? 8`, so explicit
  `diagramPadding=0` must remain valid. Validation passed:
  `cargo nextest run -p merman-render flowchart_node_spacing_zero_falls_back_to_mermaid_default flowchart_rank_spacing_zero_falls_back_to_mermaid_default flowchart_diagram_padding_zero_is_preserved`
  after first failing the two spacing tests as RED evidence;
  `cargo nextest run -p merman-render --test flowchart_layout_test --test flowchart_svg_test`;
  `cargo fmt --all --check`;
  `git diff --check`;
  `jq -c . docs/workstreams/merman-0-7-architecture-deepening/TASKS.jsonl >/dev/null`;
  `jq -e . docs/workstreams/merman-0-7-architecture-deepening/WORKSTREAM.json >/dev/null`;
  `cargo run -p xtask -- check-alignment`.
- 2026-06-06: M07A-080 completed the render-side `PresentationTheme` migration slice for XyChart by moving visible role resolution into the crate-level theme view. `crates/merman-render/src/theme.rs`
  now exposes the renderer-facing theme entry point, `svg::parity::theme::PresentationTheme`
  computes the XyChart role bundle, and `chart_palette` only owns palette parsing/derivation.
  XyChart layout tests still pass with explicit theme overrides, and the family now consumes the
  shared role surface instead of repeating its own raw `themeVariables` fallback chain. Validation
  passed:
  `cargo fmt --all --check`;
  `cargo nextest run -p merman-render presentation_theme`;
  `cargo nextest run -p merman-render chart_palette`;
  `cargo nextest run -p merman-render xychart`;
  `cargo nextest run -p merman-render theme`;
  `cargo nextest run -p merman-render quadrantchart`;
  `git diff --check`.
- 2026-06-06: M07A-080 completed the render-side `PresentationTheme` migration for QuadrantChart.
  `PresentationTheme::quadrantchart()` now owns the quadrant default derivation and explicit
  override handling, while `quadrantchart.rs` consumes the shared role bundle instead of repeating
  the raw `themeVariables` fallback chain. Validation passed:
  `cargo fmt --all --check`;
  `cargo nextest run -p merman-render chart_palette`;
  `cargo nextest run -p merman-render theme`;
  `cargo nextest run -p merman-render xychart`;
  `cargo nextest run -p merman-render quadrantchart`;
  `git diff --check`.
- 2026-06-06: M07A-090 moved typed render-model common DB sanitization field knowledge out of
  `Engine`. `RenderSemanticModel::sanitize_common_db_fields` now owns the typed dispatch and
  delegates to family-owned model methods for typed families that expose `title`, `accTitle`, and/or
  `accDescr`; JSON fallback models still use `common_db::apply_common_db_sanitization`. This also
  closes typed path gaps where Flowchart `accTitle` / `accDescr` and Treemap `title` were not
  sanitized consistently with the JSON common DB path. Validation passed:
  `cargo nextest run -p merman-core sanitize`;
  `cargo nextest run -p merman-core`;
  `cargo fmt --all --check`;
  `git diff --check`.
  Attempted `jq -c . docs/workstreams/merman-0-7-architecture-deepening/TASKS.jsonl` and
  `jq -e . docs/workstreams/merman-0-7-architecture-deepening/WORKSTREAM.json`, but this Windows
  shell does not have `jq` installed; replacement PowerShell JSON parsing was run after the docs
  update.
- 2026-06-06: M07A-100 collapsed Flowchart semantic JSON and typed render-model parsing around one
  internal `FlowchartSemanticSource`. The shared source now owns parse/build/subgraph/semantic
  application once, then projects into compatibility JSON or `FlowchartV2Model`, preserving FlowDB
  ordering traces such as `vertexCalls`. A public API regression proves the JSON and typed render
  paths agree for shared fields including accessibility metadata, class defs, tooltips, edge
  defaults, vertex calls, nodes, edges, and subgraphs. No LALRPOP/lexer strategy or public
  compatibility JSON contract was changed. Validation passed:
  `cargo nextest run -p merman-core parse_flowchart_json_and_typed_render_model_share_semantic_source`;
  `cargo nextest run -p merman-core flowchart`;
  `cargo nextest run -p merman-core`;
  `cargo nextest run -p merman-render flowchart`;
  `cargo run -p xtask -- compare-flowchart-svgs --check-dom --dom-decimals 3 --filter basic`;
  `cargo fmt --all --check`.
  `cargo run -p xtask -- check-alignment` was also attempted after this slice and failed on a
  pre-existing admission-inventory fixture-state issue: `state`, `block`, and `ishikawa` are marked
  `NormalizedWithDeferred`, but the ignored local directories `fixtures/_deferred/state`,
  `fixtures/_deferred/block`, and `fixtures/_deferred/ishikawa` are absent from this working tree.
  This task did not touch admission inventory or fixture state, so that failure is recorded as a
  follow-on M07A-110/M07A-120 planner concern rather than as Flowchart projection evidence.
  The narrowed SVG compare is sufficient for this task because the implementation changes only the
  core semantic projection source and leaves the renderer, layout, fixtures, baselines, root
  overrides, and comparator normalization unchanged; the selected Flowchart DOM gate still proves
  the typed render path reaches SVG parity for the touched family.
- 2026-06-06: M07A-110 fenced the render-model JSON semantic fallback. `Engine` now refuses
  `RenderSemanticModel::Json` fallback for built-in non-error Mermaid families when a typed render
  parser is missing, while preserving JSON fallback for the built-in `error` diagram and custom
  diagram adapters. Family-fact registry coverage proves every pinned non-error semantic parser has
  a typed render parser. Public compatibility JSON parsing/output remains unchanged because the
  fence is only on the render-model path after typed parser lookup. The admission inventory
  alignment gate now treats `fixtures/_deferred` as an ignored local investigation corpus: it keeps
  `NormalizedWithDeferred` metadata but no longer requires those directories to exist in every
  checkout. Validation passed:
  `cargo nextest run -p merman-core`;
  `cargo nextest run -p merman-render`;
  `cargo fmt --all --check`;
  `cargo run -p xtask -- check-alignment`;
  `git diff --check`.
