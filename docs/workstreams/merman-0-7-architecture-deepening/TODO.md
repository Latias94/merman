# Merman 0.7 Architecture Deepening — TODO

Status: Active
Last updated: 2026-06-06

## M0 — Scope And Evidence Freeze

- [x] M07A-010 [owner=planner] [deps=none] [scope=docs/workstreams/merman-0-7-architecture-deepening,CONTEXT.md]
  Goal: Freeze the 0.7.0 architecture deepening targets, deletion constraints, validation gates,
  and first executable task.
  Validation: `git diff --check -- CONTEXT.md docs/workstreams/merman-0-7-architecture-deepening`
  Review: Planner self-review before implementation starts.
  Evidence: `docs/workstreams/merman-0-7-architecture-deepening/DESIGN.md`
  Context: `docs/workstreams/merman-0-7-architecture-deepening/CONTEXT.jsonl`
  Handoff: DONE on 2026-06-06. Workstream docs opened from the architecture review and aligned
  around the first Headless Render Operation task.
  State: TASKS.jsonl entry M07A-010 is done.

## M1 — Canonical Headless Render Operation

- [x] M07A-020 [owner=codex] [deps=M07A-010] [scope=crates/merman/src/render]
  Goal: Introduce the smallest behavior-bearing Headless Render Operation module and route the
  existing Rust facade SVG helpers through it without changing public behavior.
  Validation: `cargo test -p merman --no-run`; `cargo nextest run -p merman-bindings-core render_svg`; `cargo nextest run -p merman-ffi render_svg`; `cargo fmt --all --check`
  Review: Ensure the new module owns parse/layout/SVG/pipeline ordering and is not a pass-through
  wrapper around the previous helpers.
  Evidence: `crates/merman/src/render/operation.rs`; downstream bindings/FFI render SVG tests.
  Context: this workstream context manifest plus ADR 0004, 0063, 0064.
  Handoff: DONE on 2026-06-06. `render_svg_sync` and `render_svg_with_pipeline_sync` now share
  a private operation module; no public methods were deleted.
  State: TASKS.jsonl entry M07A-020 is done.

- [x] M07A-030 [owner=codex] [deps=M07A-020] [scope=crates/merman-bindings-core/src/render.rs,crates/merman-cli/src/render.rs,crates/merman/src/render/raster.rs]
  Goal: Migrate bindings-core, CLI, and raster source preparation toward the canonical operation
  where it reduces duplicated render flow ordering.
  Validation: `cargo nextest run -p merman-bindings-core render_svg`; `cargo nextest run -p merman-cli render`; `cargo nextest run -p merman --features raster raster`; `cargo fmt --all --check`
  Review: Adapters may own protocol and format policy, but not rebuild the core operation.
  Evidence: `crates/merman-cli/src/render.rs`; adapter tests and raster focused tests.
  Context: this workstream context manifest plus ADR 0059, 0063, 0066.
  Handoff: DONE on 2026-06-06. CLI Mermaid input now passes its postprocess pipeline to
  `render_svg_with_pipeline_sync`; existing SVG input keeps CLI-owned postprocessing as format
  Adapter policy. Bindings-core and raster were already operation-backed via facade helpers.
  State: TASKS.jsonl entry M07A-030 is done.

- [x] M07A-040 [owner=planner] [deps=M07A-020,M07A-030] [scope=crates/merman/src/render/mod.rs,crates/merman-bindings-core,crates/merman-ffi,docs/adr]
  Goal: Audit the pre-1.0 public adapter surface, delete or demote shallow convenience methods
  where safe, and create an ADR only if a public contract changes.
  Validation: `cargo test -p merman --no-run`; `cargo nextest run -p merman-bindings-core`; `cargo nextest run -p merman-ffi`; `cargo fmt --all --check`
  Review: Deletions must not force callers to manually rebuild the render operation.
  Evidence: public surface diff; no ADR needed because removed convenience methods had no repo/docs
  callers and were not documented public contract points.
  Context: ADR 0004, 0066, this workstream context manifest.
  Handoff: DONE on 2026-06-06. Removed uncommitted shallow `*_with_diagram_id` and
  `*_sync_with` convenience methods from `HeadlessRenderer`; retained documented
  readable/resvg-safe preset helpers.
  State: TASKS.jsonl entry M07A-040 is done.

## M2 — Diagram Family Facts And Admission Inventory

- [x] M07A-050 [owner=codex] [deps=M07A-010] [scope=crates/merman-core/src/detect,crates/merman-core/src/diagram.rs,crates/merman-core/src/diagram,crates/merman-bindings-core/src/metadata.rs]
  Goal: Introduce a deeper diagram family facts module that can project detector order, aliases,
  feature profile, parser adapters, typed render adapters, known-type side effects, and supported
  diagram metadata.
  Validation: `cargo nextest run -p merman-core detect`; `cargo nextest run -p merman-core registry`; `cargo nextest run -p merman-bindings-core metadata`
  Review: The new facts module must shrink caller knowledge; avoid introducing a hypothetical seam
  with only one projection.
  Evidence: registry/detection tests and metadata projection tests.
  Context: ADR 0006, 0012, 0014, this workstream context manifest.
  Handoff: Can run after M07A-020 if write sets stay disjoint.
  Handoff: DONE on 2026-06-06. Added a core Diagram Family Facts module and projected it into
  detector order, fast detection profile behavior, semantic parser registry, typed render parser
  registry, known-type detector side effects, and binding supported diagram metadata.
  State: TASKS.jsonl entry M07A-050 is done.

- [x] M07A-060 [owner=codex] [deps=M07A-050] [scope=fixtures,docs/alignment,crates/xtask/src/cmd]
  Goal: Build an admission inventory that records raw/normalized/deferred status, semantic/layout/SVG/root coverage, skip/defer reason, and family ownership from one source.
  Validation: `cargo run -p xtask -- check-alignment`; targeted compare command for at least one promoted family.
  Review: Tests and xtask commands should consume inventory projections rather than duplicate hand lists.
  Evidence: inventory module/doc and updated gate output.
  Context: ADR 0014, 0050, 0052, 0062, `docs/alignment/UNSUPPORTED_FAMILY_ADMISSION_RUBRIC.md`.
  Handoff: Do not use this task to silently admit unsupported fixtures.
  Handoff: DONE on 2026-06-06. Added an xtask admission inventory with fixture, coverage,
  root-viewport, compare-command, owner-doc, and defer-reason fields. `compare-all-svgs` now reads
  the primary matrix and root-deferred projection from that inventory; `check-alignment` validates
  the inventory paths and evidence without moving fixtures or silently admitting families.
  State: TASKS.jsonl entry M07A-060 is done.

## M3 — SVG Root, Viewport, And Theme Deepening

- [x] M07A-070 [owner=codex] [deps=M07A-010] [scope=crates/merman-render/src/svg/parity/root_svg.rs,crates/merman-render/src/svg/parity/tree_view.rs,crates/merman-render/tests/tree_view_svg_test.rs]
  Goal: Move generic root SVG, viewport, emitted bounds, and override lookup behavior behind a
  deeper SVG parity module while preserving family-specific Mermaid deltas.
  Validation: `cargo nextest run -p merman-render root_viewport_plan`;
  `cargo nextest run -p merman-render tree_view`;
  `cargo run -p xtask -- compare-tree-view-svgs --check-dom --dom-mode parity --dom-decimals 3 --filter upstream_docs_treeview_basic`;
  `cargo run -p xtask -- compare-tree-view-svgs --check-dom --dom-mode parity-root --dom-decimals 3 --filter upstream_docs_treeview_basic`;
  `cargo run -p xtask -- compare-all-svgs --diagram treeView --check-dom --dom-mode parity-root --dom-decimals 3 --filter upstream_docs_treeview_basic`;
  `cargo fmt --all --check`.
  Review: Comparator normalization must stay narrow and non-semantic.
  Evidence: root DOM signature tests and parity-root compare output.
  Context: ADR 0050, 0057, 0062, this workstream context manifest.
  Handoff: DONE on 2026-06-06. Added shared `RootViewportPlan`/`ViewBox` planning plus root
  override precedence helpers in `root_svg.rs`; migrated `treeView` root `viewBox`/`width`/`height`
  / `style` emission to the canonical plan without changing layout bounds, theme, fixtures, or
  comparator normalization.
  State: TASKS.jsonl entry M07A-070 is done.

- [x] M07A-075 [owner=codex] [deps=M07A-030] [scope=crates/merman/src/render/raster.rs,crates/merman-cli/src/render.rs,crates/merman-cli/tests/pdf_smoke.rs]
  Goal: Close the headless PDF size-budget gap found during boundary audit by making PDF export
  enforce the same intrinsic SVG size-limit policy as raster outputs.
  Validation: `cargo nextest run -p merman --features raster svg_to_pdf`;
  `cargo nextest run -p merman-cli pdf`;
  `cargo fmt --all --check`.
  Review: `--pdfFit`, `scale`, and wrapper page sizing must not be treated as vector-complexity
  reducers; oversized intrinsic SVGs fail unless the caller explicitly opts into unbounded export.
  Evidence: `validate_svg_pdf_size`; CLI PDF branch; PDF smoke and raster tests.
  Context: ADR 0004, 0059, 0063, this workstream context manifest.
  Handoff: DONE on 2026-06-06. PDF conversion now validates SVG intrinsic size before conversion,
  keeps `svg_to_pdf` on default limits, and exposes explicit `svg_to_pdf_with_options` behavior for
  trusted unbounded callers.
  State: TASKS.jsonl entry M07A-075 is done.

- [x] M07A-076 [owner=codex] [deps=M07A-075] [scope=crates/merman-cli/src/render.rs,crates/merman-cli/tests/png_smoke.rs,crates/merman-cli/README.md]
  Goal: Split raw SVG raster/PDF input from Mermaid-generated SVG postprocessing so external SVG
  does not enter through the parity postprocess path.
  Validation: `cargo nextest run -p merman-cli raw_svg`;
  `cargo nextest run -p merman-cli png`;
  `cargo nextest run -p merman-cli pdf`;
  `cargo run -p xtask -- check-alignment`;
  `cargo fmt --all --check`.
  Review: Keep this as a boundary hardening slice, not a full SVG sanitizer rewrite.
  Evidence: raw SVG raster pipeline helper; CLI smoke for foreignObject/style hazard SVG input;
  README trusted-input note.
  Context: ADR 0059, 0063, 0064, this workstream context manifest.
  Handoff: DONE on 2026-06-06. Raw SVG raster/PDF input now starts from a `resvg_safe` pipeline
  before CLI background/CSS postprocessors, and conversion still applies normal resvg-safe cleanup
  and size limits.
  State: TASKS.jsonl entry M07A-076 is done.

- [x] M07A-077 [owner=codex] [deps=M07A-030,M07A-076] [scope=crates/merman-core/src/lib.rs,crates/merman-core/src/diagrams/gantt/tests.rs,crates/merman-cli/src/cli.rs,crates/merman-cli/src/config.rs,crates/merman-cli/src/render.rs,crates/merman-cli/tests/cli_compat.rs,crates/merman-cli/README.md,docs/alignment/CLI_COMPATIBILITY.md]
  Goal: Expose fixed Gantt/local-time controls through CLI parse/render entry points and ensure
  typed render-model parsing uses the same fixed-time context as semantic JSON parsing.
  Validation: `cargo nextest run -p merman-core gantt_render_model_uses_fixed_today_for_missing_year_dates`;
  `cargo nextest run -p merman-cli fixed`;
  `cargo nextest run -p merman-cli top_level_gantt_fixed_today_is_carried_through_export_args`;
  `cargo fmt --all --check`.
  Review: Keep this as a deterministic headless adapter slice; do not rewrite Gantt parser,
  layout, theme, or SVG parity behavior.
  Evidence: CLI fixed time flags, Engine construction, render-model fixed-time wrapper, and CLI/core
  Gantt regressions.
  Context: this workstream context manifest plus ADR 0004 and 0014.
  Handoff: DONE on 2026-06-06. CLI now accepts `--fixed-today` and
  `--fixed-local-offset-minutes`; semantic JSON and typed render-model paths both honor those
  fixed local-time controls, so Gantt parse and SVG render outputs can be made reproducible from
  the CLI.
  State: TASKS.jsonl entry M07A-077 is done.

- [ ] M07A-080 [owner=unassigned] [deps=M07A-010] [scope=crates/merman-render/src/svg/parity/theme.rs,crates/merman-render/src/config.rs,crates/merman-render/src/xychart.rs,crates/merman-render/src/quadrantchart.rs,crates/merman-render/src/svg/parity]
  Goal: Continue ADR 0068 by migrating repeated raw `themeVariables` fallback chains into
  renderer-facing `PresentationTheme` roles.
  Validation: targeted theme tests; `cargo nextest run -p merman-render theme`; targeted SVG compare for migrated families.
  Review: Keep Mermaid-compatible core config ownership in `merman-core`; renderer roles must not become host styling policy.
  Evidence: theme role tests and family compare evidence.
  Context: ADR 0068, 0063, 0064, this workstream context manifest.
  Handoff: Prefer chart and shared CSS surfaces before rare family-local constants.
  State: TASKS.jsonl entry M07A-080 is draft.

## M4 — Typed Semantic Ownership And JSON Fallback

- [ ] M07A-090 [owner=unassigned] [deps=M07A-010] [scope=crates/merman-core/src/lib.rs,crates/merman-core/src/diagram,crates/merman-core/src/diagrams]
  Goal: Move typed semantic sanitization field knowledge out of Engine and into family-owned
  semantic construction/projection paths.
  Validation: `cargo nextest run -p merman-core sanitize`; targeted family semantic tests.
  Review: Engine should orchestrate stages, not know every family field that needs sanitization.
  Evidence: tests for title, accTitle, accDescr sanitization through family interfaces.
  Context: ADR 0010, 0011, 0020, this workstream context manifest.
  Handoff: Start with one high-coverage family before broad adoption.
  State: TASKS.jsonl entry M07A-090 is draft.

- [ ] M07A-100 [owner=unassigned] [deps=M07A-090] [scope=crates/merman-core/src/diagrams/flowchart.rs,crates/merman-core/src/diagrams/flowchart]
  Goal: Collapse Flowchart JSON and typed render projections around one semantic source while
  preserving Mermaid FlowDB ordering traces.
  Validation: `cargo nextest run -p merman-core flowchart`; targeted flowchart SVG compare.
  Review: Do not change LALRPOP/lexer strategy or delete `vertexCalls`-style ordering evidence.
  Evidence: semantic JSON parity tests, typed render tests, and selected SVG compare output.
  Context: ADR 0010, 0013, 0014, this workstream context manifest.
  Handoff: Requires careful review because Flowchart is the largest parity-risk family.
  State: TASKS.jsonl entry M07A-100 is draft.

- [ ] M07A-110 [owner=planner] [deps=M07A-050,M07A-060,M07A-090] [scope=crates/merman-core/src/diagram,crates/merman-render/src/lib.rs,crates/merman-render/src/svg/parity.rs,docs/adr]
  Goal: Fence `RenderSemanticModel::Json` as a compatibility adapter or delete paths proven
  unnecessary by admission evidence, while preserving compatibility JSON public output.
  Validation: `cargo nextest run -p merman-core`; `cargo nextest run -p merman-render`; relevant compare-all gate.
  Review: Any change to public JSON output requires ADR review; renderer input cleanup alone should not.
  Evidence: admission inventory proof, renderer dispatch tests, optional ADR.
  Context: ADR 0004, 0010, 0011, this workstream context manifest.
  Handoff: Speculative until M07A-050/M07A-060 prove the release surface.
  State: TASKS.jsonl entry M07A-110 is draft.

## M5 — Final Verification And Closeout

- [ ] M07A-120 [owner=planner] [deps=M07A-040,M07A-060,M07A-070,M07A-077,M07A-080,M07A-100,M07A-110] [scope=docs/workstreams/merman-0-7-architecture-deepening,CONTEXT.md,docs/alignment]
  Goal: Run final gates, reconcile TODO.md/TASKS.jsonl/CAMPAIGNS.jsonl, close or split remaining
  architecture follow-ons, and prepare 0.7.0-facing release notes.
  Validation: `cargo fmt --all --check`; `cargo nextest run --workspace`; `cargo run -p xtask -- check-alignment`; full or justified narrowed SVG parity gate.
  Review: `review-workstream` and `verify-rust-workstream` before closing.
  Evidence: final command evidence in `EVIDENCE_AND_GATES.md`.
  Context: this workstream context manifest.
  Handoff: Do not close with speculative JSON fallback or public surface risks unrecorded.
  State: TASKS.jsonl entry M07A-120 is draft.
