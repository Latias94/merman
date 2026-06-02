# Headless Parity Deepening - Evidence And Gates

Status: Active
Last updated: 2026-06-02

## HPD-020 - Baseline Registry

Outcome:

- Added [crates/merman-core/src/baseline.rs](/F:/SourceCodes/Rust/merman/crates/merman-core/src/baseline.rs)
  as the explicit baseline truth seam for:
  - pinned Mermaid tag
  - pinned Mermaid version
  - pinned Mermaid version suffix
  - explicit legacy generated suffix
- `Engine::default()` now uses pinned-baseline registry constructors rather than
  `default_mermaid_11_12_2*`.
- `DetectorRegistry`, `DiagramRegistry`, and `RenderDiagramRegistry` now expose pinned-baseline
  constructors as the live API, with `default_mermaid_11_12_2*` retained only as deprecated
  compatibility aliases.
- xtask importers, bench entrypoints, and baseline report labeling now route through the pinned
  baseline path instead of presenting `11.12.x` names as current truth.
- Historical generated filenames and lookup modules are still legacy-suffixed; that is now explicit
  provenance, not the live baseline identity.

Touched production surfaces:

- [crates/merman-core/src/lib.rs](/F:/SourceCodes/Rust/merman/crates/merman-core/src/lib.rs)
- [crates/merman-core/src/detect/mod.rs](/F:/SourceCodes/Rust/merman/crates/merman-core/src/detect/mod.rs)
- [crates/merman-core/src/diagram/mod.rs](/F:/SourceCodes/Rust/merman/crates/merman-core/src/diagram/mod.rs)
- [crates/xtask/src/cmd/overrides/report.rs](/F:/SourceCodes/Rust/merman/crates/xtask/src/cmd/overrides/report.rs)
- [crates/xtask/src/cmd/root_override_audit.rs](/F:/SourceCodes/Rust/merman/crates/xtask/src/cmd/root_override_audit.rs)
- [crates/xtask/src/cmd/import/docs.rs](/F:/SourceCodes/Rust/merman/crates/xtask/src/cmd/import/docs.rs)
- [crates/xtask/src/cmd/import/examples.rs](/F:/SourceCodes/Rust/merman/crates/xtask/src/cmd/import/examples.rs)
- [crates/xtask/src/cmd/import/html.rs](/F:/SourceCodes/Rust/merman/crates/xtask/src/cmd/import/html.rs)
- [crates/xtask/src/cmd/import/pkg_tests.rs](/F:/SourceCodes/Rust/merman/crates/xtask/src/cmd/import/pkg_tests.rs)
- [crates/xtask/src/cmd/import/cypress.rs](/F:/SourceCodes/Rust/merman/crates/xtask/src/cmd/import/cypress.rs)
- [crates/merman/benches/pipeline.rs](/F:/SourceCodes/Rust/merman/crates/merman/benches/pipeline.rs)

Focused verification:

- `cargo fmt --all`
- `cargo test -p merman-core baseline --lib`
- `rg -n "default_mermaid_11_12_2|default_mermaid_11_12_2_full|default_mermaid_11_12_2_tiny" crates -g '!target/**'`

Verification notes:

- The remaining `default_mermaid_11_12_2*` hits are compatibility alias definitions only; live
  call sites have been moved to the pinned-baseline constructors.
- An attempted `cargo test -p xtask ... --lib` check was invalid because `xtask` has no library
  target. The baseline-report path was instead verified by code inspection plus the existing
  `pinned_mermaid_baseline_label_reads_lockfile_ref` unit test in
  `crates/xtask/src/cmd/overrides/report.rs`.

## HPD-030 - Residual Taxonomy

Outcome:

- Added an explicit six-class residual taxonomy to
  [docs/workstreams/headless-parity-deepening/DESIGN.md](/F:/SourceCodes/Rust/merman/docs/workstreams/headless-parity-deepening/DESIGN.md):
  - source-backed behavior gap
  - generated measurement gap
  - browser lattice tail
  - stale baseline or stale override
  - solver / phase residual
  - scope boundary
- Applied that taxonomy to the active root residual lane in
  [docs/workstreams/mermaid-11-15-root-viewport-residuals/DESIGN.md](/F:/SourceCodes/Rust/merman/docs/workstreams/mermaid-11-15-root-viewport-residuals/DESIGN.md).
- Froze the intended interpretation of the current active buckets:
  - Flowchart: mostly browser lattice tails
  - Architecture: main solver/phase residual front, plus some measurement tails
  - Sequence: mixed generated-measurement gaps and browser tails
  - Class: generated-measurement gap plus stale-table audit front
  - Timeline/Journey: browser tails unless stronger evidence emerges

Validation basis:

- Current active counts and classifications were derived from:
  - [docs/workstreams/mermaid-11-15-root-viewport-residuals/HANDOFF.md](/F:/SourceCodes/Rust/merman/docs/workstreams/mermaid-11-15-root-viewport-residuals/HANDOFF.md)
  - [docs/workstreams/mermaid-11-15-root-viewport-residuals/DESIGN.md](/F:/SourceCodes/Rust/merman/docs/workstreams/mermaid-11-15-root-viewport-residuals/DESIGN.md)
  - [docs/alignment/STATUS.md](/F:/SourceCodes/Rust/merman/docs/alignment/STATUS.md)
  - [docs/quality/ARCHITECTURE_ISSUES_2026-06-01.md](/F:/SourceCodes/Rust/merman/docs/quality/ARCHITECTURE_ISSUES_2026-06-01.md)

Verification notes:

- This task intentionally does not claim new residual counts beyond the current authoritative
  workstream state.
- The deliverable is a durable classification system and queue-shaping mapping, not a fresh one-off
  report or a pseudo-precise completion metric.

## HPD-040 - Measurement / Root Bounds Platform

Outcome:

- Moved the SVG emitted-bounds scanner from the State renderer submodule into
  [crates/merman-render/src/svg/parity/emitted_bounds.rs](/F:/SourceCodes/Rust/merman/crates/merman-render/src/svg/parity/emitted_bounds.rs).
  This matches its real ownership: it is shared root-bounds infrastructure for State,
  Architecture, and GitGraph, not State-specific rendering logic.
- Extracted Sequence note final wrapping into `sequence_note_final_wrapped_lines(...)` and final
  wrapped-text measurement into `measure_sequence_note_final_text(...)`.
- Reused the Sequence note final wrap path from layout, root-bounds, and SVG rendering so Mermaid
  source-backed note wrapping is not re-derived in three places.
- Removed the now-unneeded crate-level re-export of Sequence note slack constants. The constants
  remain internal to the Sequence seam and its tests.
- Added no new fixture-keyed text tables, root overrides, or ad hoc parity constants.

Touched production surfaces:

- [crates/merman-render/src/svg/parity.rs](/F:/SourceCodes/Rust/merman/crates/merman-render/src/svg/parity.rs)
- [crates/merman-render/src/svg/parity/emitted_bounds.rs](/F:/SourceCodes/Rust/merman/crates/merman-render/src/svg/parity/emitted_bounds.rs)
- [crates/merman-render/src/svg/parity/state/mod.rs](/F:/SourceCodes/Rust/merman/crates/merman-render/src/svg/parity/state/mod.rs)
- [crates/merman-render/src/sequence.rs](/F:/SourceCodes/Rust/merman/crates/merman-render/src/sequence.rs)
- [crates/merman-render/src/sequence/notes.rs](/F:/SourceCodes/Rust/merman/crates/merman-render/src/sequence/notes.rs)
- [crates/merman-render/src/sequence/root_bounds.rs](/F:/SourceCodes/Rust/merman/crates/merman-render/src/sequence/root_bounds.rs)
- [crates/merman-render/src/svg/parity/sequence/notes.rs](/F:/SourceCodes/Rust/merman/crates/merman-render/src/svg/parity/sequence/notes.rs)

Focused verification:

- `cargo fmt --all`
- `cargo test -p merman-render svg_emitted_bounds --lib`
- `cargo test -p merman-render sequence_long_leftof_notes_keep_mermaid_11_15_note_width --test sequence_svg_test`
- `cargo test -p merman-render architecture_text_constants_match_mermaid --lib`
- `cargo run -p xtask -- report-overrides --check-no-growth`

Negative / residual evidence:

- `cargo nextest run -p merman-render svg_emitted_bounds` was attempted first but the local toolchain
  does not have `cargo-nextest` installed, so verification used `cargo test`.
- `cargo test -p merman-render sequence_long_leftof_notes_keep_mermaid_11_15_root_width --test sequence_svg_test`
  still fails. This is intentional evidence that HPD-040 did not claim forced root parity closure.
- Deterministic CLI render for
  `upstream_cypress_sequencediagram_spec_should_render_long_notes_wrapped_inline_left_of_actor_026`
  produced `max-width: 570px` / `viewBox="-170 -10 570 412"` while the upstream target remains
  `566px`.
- `cargo run -p xtask -- compare-sequence-svgs --filter upstream_cypress_sequencediagram_spec_should_render_long_notes_wrapped_inline_left_of_actor_026 --report-root --report-root-all`
  reported headless vendored local `585.000px` vs upstream `566.000px` (`+19.000px`). Treat this as
  a Sequence measurement/root residual for later classification, not as a reason to add a local
  width override.

## HPD-050 - Architecture Layout Engine Audit

First slice outcome:

- Extracted Architecture's FCoSE node-bounds adapter from the main Architecture layout function.
  After the later HPD-050 seam cleanup this adapter is named
  `architecture_fcose_node_bounds_extras(...)` and owns only the part the renderer actually feeds
  into `manatee`: per-node `BoundsExtras` for Cytoscape
  `compound-sizing-wrt-labels: include` approximation.
- Removed the layout-view group-title field. Current source/evidence says group titles are rendered
  inside compound bounds and do not participate in the pre-layout center used for FCoSE relocation;
  final SVG rendering still reads titles from semantic model data.
- Added direct unit coverage for the node-bounds helper so service label border/bottom extras stay
  explicit.
- This is an adapter-boundary refactor, not a residual-count claim. It keeps Mermaid/Cytoscape
  approximation policy in `merman-render` instead of leaking another diagram-specific rule into
  `manatee`.

Focused verification:

- `cargo fmt --all`
- `cargo test -p merman-render architecture_fcose_node_bounds_extras_feed_label_bounds --lib`
- `cargo test -p merman-render architecture_relative_constraints_preserve_mermaid_duplicate_bfs_pops --lib`
- `cargo test -p merman-render --test architecture_layout_test`
- `cargo run -p xtask -- report-overrides --check-no-growth`
- `git diff --check`

Residual evidence:

- `cargo run -p xtask -- compare-architecture-svgs --filter stress_architecture_batch5_long_titles_and_punct_076 --check-dom --dom-mode parity-root --dom-decimals 3 --report-root-all --out target/compare/architecture_batch5_after_prelayout_adapter.md`
  still fails with the known root-only tail: upstream `542.926px`, local `547.926px`, delta
  `+5.000px`.
- The unchanged focused tail is intentional evidence that this pass moved ownership boundaries
  without silently tuning Architecture root widths.

Second slice outcome:

- Re-audited `stress_architecture_batch4_init_small_icons_061` against Mermaid source and the
  existing browser-probe evidence instead of treating it as a service-label scale problem.
- Mermaid's `svgDraw.ts` renders Architecture edge labels through `createText(...)` and then
  rotates Y-axis labels with `transform="translate(... ) rotate(-90)"`; the root viewport comes
  from `setupGraphViewbox(svg.getBBox() + padding)`.
- The old local root-bounds model treated edge-label bboxes as centered AABBs. That missed the
  positive local `createText` y-range, which becomes a rightward x-extension after `rotate(-90)`.
- Extracted `architecture_create_text_bbox_y_range_px(...)` in
  [crates/merman-render/src/architecture_metrics.rs](/F:/SourceCodes/Rust/merman/crates/merman-render/src/architecture_metrics.rs)
  and made Architecture edge-label plans carry transformed `Bounds` instead of only centered
  width/height pairs.
- Corrected `architecture_create_text_compound_label_extra_bottom_px(...)` to the source-backed
  `fontSize + 1px` rule. The previous `fontSize * 17 / 16` formula was only equivalent at the
  default `16px` font size and undercounted custom Architecture font sizes such as `12px`.
- Added regression coverage for the small-icon fixture: service/group sizing remains icon-floor
  dominated, but the vertical edge label now contributes to the root width and the compound label
  bottom follows `architecture.fontSize + 1px`.

Focused verification:

- `cargo fmt --all`
- `cargo test -p merman-render architecture_text_constants_match_mermaid --lib`
- `cargo test -p merman-render architecture_vertical_edge_label_bounds_use_create_text_y_offsets --test architecture_svg_test`
- `cargo test -p merman-render --test architecture_svg_test`
- `cargo run -p xtask -- compare-architecture-svgs --filter stress_architecture_batch4_init_small_icons_061 --check-dom --dom-mode parity-root --dom-decimals 3 --report-root-all --out target/compare/architecture_batch4_small_icons_hpd050_edge_label_bounds.md`
- `cargo run -p xtask -- compare-architecture-svgs --check-dom --dom-mode parity --dom-decimals 3 --out target/compare/architecture_report_parity_after_hpd050_edge_label_bounds.md`
- `cargo run -p xtask -- compare-architecture-svgs --check-dom --dom-mode parity-root --dom-decimals 3 --report-root-all --out target/compare/architecture_report_parity_root_after_hpd050_edge_label_bounds.md`
- `cargo run -p xtask -- report-overrides --check-no-growth`
- `git diff --check`

Residual evidence after the second slice:

- The focused small-icon row is now root-green: upstream and local both report
  `187.859x191.571`.
- The full Architecture structural `parity` gate remains green.
- Full Architecture `parity-root` still fails, but the mismatch count dropped from `29` to `26`.
  `stress_architecture_batch4_init_small_icons_061`,
  `stress_architecture_batch4_init_fontsize_wrap_063`, and
  `stress_architecture_edge_label_corner_cases_012` are now `+0.000` root delta rows.
- The remaining top Architecture residuals are still led by
  `stress_architecture_junction_fork_join_026` (`+13.976px`),
  `stress_architecture_batch5_long_titles_and_punct_076` (`+5.000px`), and
  `stress_architecture_html_titles_and_escapes_041` (`+5.000px`). These remain open and should not
  be closed by constants without new source-backed evidence.

Third slice classification:

- Rechecked `stress_architecture_junction_fork_join_026`, the largest remaining Architecture
  root residual, after the edge-label bounds fix.
- Local debug still feeds the source-backed FCoSE inputs already documented in M15RV-089:
  ungrouped junction parents, 9 relative-placement constraints including duplicate `join -> db`
  and `join -> cache`, configured group padding, and the current `eles.boundingBox()` relocation
  approximation.
- The old saved Mermaid browser probe
  [target/compare/arch_junction_fork_join_probe_m15rv089.json](/F:/SourceCodes/Rust/merman/target/compare/arch_junction_fork_join_probe_m15rv089.json)
  has final service positions that match the current local SVG to floating-point noise.
- A fresh `check-upstream-svgs` run using Edge as `PUPPETEER_EXECUTABLE_PATH` reproduced the stored
  upstream SVG fixture exactly: both report `max-width: 2808.126708984375px` and
  `viewBox="-1362.063232421875 -1213.2674560546875 2808.126708984375 2557.534912109375"`.
- Therefore the previous "stored upstream baseline drift" reading was too broad. The old saved debug
  browser probe is the divergent path here: its service positions match local output, but differ
  from the current CLI/Edge baseline by about `7-10px` on X and `6-12px` on Y.
- Treat the remaining `+13.976px` root tail as a probe-harness / CLI-harness divergence plus a
  solver/phase residual candidate. Do not tune `manatee` against the saved probe alone, and do not
  refresh or discard the stored fixture on the basis of that probe.

Focused verification:

- `cargo run -p xtask -- compare-architecture-svgs --filter stress_architecture_junction_fork_join_026 --check-dom --dom-mode parity-root --dom-decimals 3 --report-root-all --out target/compare/architecture_junction_fork_join_hpd050_debug.md`
- `$env:PUPPETEER_EXECUTABLE_PATH='C:\Program Files (x86)\Microsoft\Edge\Application\msedge.exe'; cargo run -p xtask -- check-upstream-svgs --diagram architecture --filter stress_architecture_junction_fork_join_026 --check-dom --dom-mode parity-root --dom-decimals 3`
- `$env:PUPPETEER_EXECUTABLE_PATH='C:\Program Files (x86)\Microsoft\Edge\Application\msedge.exe'; cargo run -p xtask -- check-upstream-svgs --diagram architecture --filter stress_architecture_junction_fork_join_026 --check-dom --dom-mode parity --dom-decimals 3`
- PowerShell JSON/SVG comparison of the old saved
  `target/compare/arch_junction_fork_join_probe_m15rv089.json`
  final positions against the local SVG showed deltas at floating-point noise level.
- The same comparison against
  `fixtures/upstream-svgs/architecture/stress_architecture_junction_fork_join_026.svg` showed the
  debug probe / CLI baseline split: e.g. probe-minus-fixture deltas are `auth.x=+10.376px`,
  `cache.x=+10.376px`, `api.y=-12.358px`, and `db.y=-12.358px`.

Fourth slice seam cleanup:

- Audited the remaining `+5px` Architecture root rows
  `stress_architecture_batch5_long_titles_and_punct_076` and
  `stress_architecture_html_titles_and_escapes_041` against saved Mermaid browser probes and the
  current upstream/local SVGs.
- For both rows, upstream service positions match the saved browser probe while local service
  positions differ only by about `0.5px` in X. The root-width delta is controlled by the final
  group rectangle:
  - `batch5_long_titles`: upstream group width `462.925633px`, local `467.925633px`
  - `html_titles`: upstream group width `399.925633px`, local `404.925633px`
- The shared old name `architecture_compound_bbox_padding_px(...)` implied one padding policy for
  multiple Cytoscape phases. That was misleading. Mermaid's final group rect path reads
  `node.boundingBox()` in `svgDraw.ts`, while manatee's relocation/element bbox approximation is a
  separate layout-engine phase.
- Renamed the renderer helper to `architecture_svg_group_bbox_padding_px(...)` and removed the
  unused renderer-side `initial_center` / pre-layout group bbox model. Relocation-centering remains
  owned by `manatee`'s indexed graph adapter, where the actual layout consumes it.
- This was an honesty/refactor slice, not a root-width tune. The two focused `+5px` rows remain
  open as group/service Cytoscape bbox measurement residuals until generated browser evidence or a
  better deterministic canvas-bbox seam justifies narrowing the approximation.

Focused verification:

- `cargo fmt --all`
- `cargo test -p merman-render architecture_fcose_node_bounds_extras_feed_label_bounds --lib`
- `cargo test -p merman-render architecture_svg_group_bbox_padding_adds_headless_cytoscape_extra --lib`
- `cargo test -p merman-render architecture_text_constants_match_mermaid --lib`
- `cargo test -p merman-render --test architecture_svg_test`
- `cargo run -p xtask -- compare-architecture-svgs --filter stress_architecture_batch5_long_titles_and_punct_076 --check-dom --dom-mode parity-root --dom-decimals 3 --report-root-all --out target/compare/architecture_batch5_hpd050_bounds_extras_refactor.md`
- `cargo run -p xtask -- compare-architecture-svgs --filter stress_architecture_html_titles_and_escapes_041 --check-dom --dom-mode parity-root --dom-decimals 3 --report-root-all --out target/compare/architecture_html_titles_hpd050_bounds_extras_refactor.md`
- `cargo run -p xtask -- compare-architecture-svgs --check-dom --dom-mode parity --dom-decimals 3 --out target/compare/architecture_report_parity_after_hpd050_bounds_extras_refactor.md`
- `cargo run -p xtask -- compare-architecture-svgs --check-dom --dom-mode parity-root --dom-decimals 3 --report-root-all --out target/compare/architecture_report_parity_root_after_hpd050_bounds_extras_refactor.md`
- `cargo run -p xtask -- report-overrides --check-no-growth`
- `git diff --check`

Fifth slice probe-harness correction:

- Re-audited the Architecture FCoSE browser probe after `junction_fork_join_026` showed a
  probe/fixture split.
- The actual installed baseline package
  `tools/mermaid-cli/node_modules/mermaid/package.json` is `mermaid@11.15.0`, and the generated
  `dist/mermaid.js` used by `check-upstream-svgs` does not contain the later `withSeededRandom`
  Architecture source path seen in `repo-ref/mermaid/packages/mermaid/src/diagrams/architecture`.
  It still uses the xtask page-level deterministic prelude and the Architecture FCoSE config fields
  `randomize`, `nodeSeparation`, `idealEdgeLengthMultiplier`, `edgeElasticity`, and `numIter`.
- Updated
  [tools/debug/arch_fcose_browser_probe_fixture_025.js](/F:/SourceCodes/Rust/merman/tools/debug/arch_fcose_browser_probe_fixture_025.js)
  so it:
  - documents that it is a manual diagnostic reconstruction rather than a full Mermaid CLI render
    replacement,
  - mirrors the xtask deterministic page prelude more closely by also patching
    `crypto.getRandomValues`, and
  - reads the same currently shipped Architecture FCoSE config fields instead of hard-coding
    same-group ideal length and elasticity.
- A rejected exploratory patch changed `manatee` from the current xorshift deterministic baseline
  to the later repo-ref `mulberry32` seed helper. It was reverted before commit because the shipped
  npm `mermaid@11.15.0` baseline does not contain that path. Do not repeat that change unless the
  baseline package changes or `dist/mermaid.js` confirms the source path.
- A refreshed probe
  `target/compare/arch_junction_fork_join_probe_hpd050_debug_tool_refresh.json` still does not
  reproduce the CLI fixture. It reports probe-minus-fixture deltas such as
  `auth.x=+12.684px`, `cache.x=+12.684px`, `api.y=-15.004px`, and `db.y=-15.004px`. It is closer
  to local output than to the fixture, but no longer exactly identical after the config/prelude
  cleanup. Treat it as diagnostic evidence only.

Focused verification:

- `cargo fmt --all`
- `cargo test -p manatee xorshift64star_next_f64_unit_matches_seeded_upstream_baseline --lib`
- `cargo test -p merman-render architecture_fcose_node_bounds_extras_feed_label_bounds --lib`
- `$env:PUPPETEER_EXECUTABLE_PATH='C:\Program Files (x86)\Microsoft\Edge\Application\msedge.exe'; node tools/debug/arch_fcose_browser_probe_fixture_025.js stress_architecture_junction_fork_join_026 > target/compare/arch_junction_fork_join_probe_hpd050_debug_tool_refresh.json`
- `cargo run -p xtask -- compare-architecture-svgs --filter stress_architecture_junction_fork_join_026 --check-dom --dom-mode parity-root --dom-decimals 3 --report-root-all --out target/compare/architecture_junction_fork_join_hpd050_debug_tool_refresh.md`
  expected failure remains `2808.127px` upstream vs `2822.102px` local (`+13.976px`).

Sixth slice source-checkout guard:

- Checked the local reference checkout before continuing source-backed Architecture work:
  - `git -C repo-ref/mermaid rev-parse HEAD` => `9bae92cd3214f9ec99369ab314ef41ffb283f6b6`
  - `git -C repo-ref/mermaid status --short --branch` => `develop...origin/develop`
  - `tools/upstreams/REPOS.lock.json` pins Mermaid to
    `41646dfd43ac83f001b03c70605feb036afae46d` (`mermaid@11.15.0`)
- The repo-ref checkout is therefore ahead of the active baseline. This explains why reading
  `repo-ref/mermaid/packages/mermaid/src/diagrams/architecture/architectureRenderer.ts` directly
  exposed a later `withSeededRandom` path that is absent from the installed
  `tools/mermaid-cli/node_modules/mermaid/dist/mermaid.js` and from the locked
  `41646dfd...` source.
- For HPD-050 source-backed claims, use one of:
  - `git -C repo-ref/mermaid show 41646dfd43ac83f001b03c70605feb036afae46d:<path>` for source,
  - `tools/mermaid-cli/node_modules/mermaid/dist/mermaid.js` for the actual baseline renderer
    bundle, or
  - fresh `check-upstream-svgs` output for rendered behavior.
- Do not use the current `repo-ref/mermaid` working tree path as baseline truth unless it has first
  been verified against `tools/upstreams/REPOS.lock.json`.

Focused verification:

- `git -C repo-ref/mermaid show --no-patch --oneline 41646dfd43ac83f001b03c70605feb036afae46d`
- `git -C repo-ref/mermaid show 41646dfd43ac83f001b03c70605feb036afae46d:packages/mermaid/src/diagrams/architecture/architectureRenderer.ts`
  confirmed locked source has `gap: 1.5 * db.getConfigField('iconSize')`, reads
  `idealEdgeLengthMultiplier`, `edgeElasticity`, `randomize`, `nodeSeparation`, and `numIter`, and
  has no `withSeededRandom` path.

Residual evidence after the fourth slice:

- Full Architecture structural `parity` remains green.
- Full Architecture `parity-root` remains an expected failure with `26` mismatches.
- `stress_architecture_batch5_long_titles_and_punct_076` remains upstream `542.926px` vs local
  `547.926px` (`+5.000px`).
- `stress_architecture_html_titles_and_escapes_041` remains upstream `479.926px` vs local
  `484.926px` (`+5.000px`).
- Override growth remains unchanged.

Seventh slice Cytoscape bbox phase split:

- Enhanced
  [tools/debug/arch_fcose_browser_probe_fixture_025.js](/F:/SourceCodes/Rust/merman/tools/debug/arch_fcose_browser_probe_fixture_025.js)
  so pre-layout node diagnostics include Cytoscape `labelWidth`, `labelHeight`, `labelBounds`,
  `bodyBounds`, `autoWidth`, `autoHeight`, and `autoPadding`.
- The refreshed diagnostic probe for
  `stress_architecture_batch6_init_fontsize_icon_size_wrap_093` reports:
  - `api` service `labelWidth=95`, `labelBounds=99x22`, `bodyBounds=42x42`,
    `node.boundingBox()=101x62`
  - `db` service `labelWidth=78`, `labelBounds=82x22`, `node.boundingBox()=84x62`
  - `left` group `autoWidth=99`, `autoHeight=61`, `outerWidth=160x122`,
    `node.boundingBox()=162x124`
- This proves the row needs separate handling for leaf default `node.boundingBox()`, child
  `updateCompoundBounds()` contribution, final group `node.boundingBox()`, and manatee relocation
  bbox approximation.
- A source-shaped exploratory production patch changed Architecture bbox math to
  `ceil(canvas)+labelBounds` and group extra `+1.5`. It was rejected before commit:
  - `batch6_init_fontsize_icon_size_wrap_093` became root-exact (`325.105x380.479`)
  - `batch4_init_small_icons_061` stayed root-exact (`187.859x191.571`)
  - full Architecture root mismatches increased from `26` to `47`
  - `batch5_long_titles_and_punct_076` worsened from `+5.000px` to `+7.500px`
  - `html_titles_and_escapes_041` worsened from `+5.000px` to `+7.500px`
- The production patch was reverted. Keep the diagnostic probe output, but do not apply a single
  global Cytoscape bbox formula until the renderer/manatee seam can represent the separate phases.

Focused verification:

- `$env:PUPPETEER_EXECUTABLE_PATH='C:\Program Files (x86)\Microsoft\Edge\Application\msedge.exe'; node tools/debug/arch_fcose_browser_probe_fixture_025.js stress_architecture_batch6_init_fontsize_icon_size_wrap_093 > target/compare/arch_batch6_init_fontsize_icon_size_wrap_probe_hpd050_metrics.json`
- `cargo run -p xtask -- compare-architecture-svgs --filter stress_architecture_batch6_init_fontsize_icon_size_wrap_093 --check-dom --dom-mode parity-root --dom-decimals 3 --report-root-all --out target/compare/architecture_batch6_init_fontsize_icon_size_wrap_hpd050_cytoscape_bbox_seam_y.md`
- `cargo run -p xtask -- compare-architecture-svgs --filter stress_architecture_batch4_init_small_icons_061 --check-dom --dom-mode parity-root --dom-decimals 3 --report-root-all --out target/compare/architecture_batch4_small_icons_hpd050_cytoscape_bbox_seam_y.md`
- `cargo run -p xtask -- compare-architecture-svgs --check-dom --dom-mode parity-root --dom-decimals 3 --report-root-all --out target/compare/architecture_report_parity_root_after_hpd050_cytoscape_bbox_seam.md`
- `cargo run -p xtask -- compare-architecture-svgs --check-dom --dom-mode parity-root --dom-decimals 3 --report-root-all --out target/compare/architecture_report_parity_root_after_hpd050_probe_metrics_only.md`
- `cargo test -p merman-render architecture_text_constants_match_mermaid --lib`

Eighth slice service bounds phase-name refactor:

- Renamed Architecture service bounds estimate fields so the renderer explicitly distinguishes:
  - `emitted_icon_bounds`
  - `svg_root_bounds`
  - `cytoscape_group_child_bounds`
- This is a behavior-preserving seam cleanup after the rejected bbox formula. It keeps the current
  broad approximation while preventing future work from treating root SVG getBBox, emitted icon
  bounds, and group child bounds as one interchangeable `compound_bounds` phase.

Focused verification:

- `cargo fmt --all`
- `cargo test -p merman-render architecture_text_constants_match_mermaid --lib`
- `cargo test -p merman-render --test architecture_svg_test`
- `cargo run -p xtask -- compare-architecture-svgs --filter stress_architecture_batch6_init_fontsize_icon_size_wrap_093 --check-dom --dom-mode parity-root --dom-decimals 3 --report-root-all --out target/compare/architecture_batch6_init_fontsize_icon_size_wrap_hpd050_phase_names_refactor.md`
  expected failure remains upstream `325.105px` vs local `322.605px` (`-2.500px`).
- `cargo run -p xtask -- compare-architecture-svgs --check-dom --dom-mode parity --dom-decimals 3 --out target/compare/architecture_report_parity_after_hpd050_phase_names_refactor.md`
  passed.
- `cargo run -p xtask -- compare-architecture-svgs --check-dom --dom-mode parity-root --dom-decimals 3 --report-root-all --out target/compare/architecture_report_parity_root_after_hpd050_phase_names_refactor.md`
  expected failure remains `26` Architecture root mismatches.

Ninth slice Dugong / Graphlib audit:

- Cloned `repo-ref/dagre` and `repo-ref/graphlib`, then checked them out to the lockfile commits:
  - Dagre: `ba986662394f8f3ed608717194e5958f3386ce01`
  - Graphlib: `380d5efa1f4ab0904539f046bdba583d14ac2add`
- Added
  [docs/dugong/GRAPHLIB_UPSTREAM_TEST_COVERAGE.md](/F:/SourceCodes/Rust/merman/docs/dugong/GRAPHLIB_UPSTREAM_TEST_COVERAGE.md)
  so Graphlib test coverage is no longer only an implicit assumption behind Dagre coverage.
- Ported the currently exposed Graphlib helper algorithm cases from upstream:
  `components`, `findCycles`, `preorder`, and `postorder`.
- Tightened `dugong_graphlib::alg::{preorder, postorder}` so missing roots panic instead of
  silently traversing a non-existent root, matching upstream Graphlib's invalid-root throw
  behavior in Rust form.
- Fixed
  [tools/dagre-harness/run.mjs](/F:/SourceCodes/Rust/merman/tools/dagre-harness/run.mjs)
  to import `dagre-d3-es` from `tools/mermaid-cli/node_modules`, which makes the reference runner
  executable in the current repository layout.
- Updated
  [tools/dagre-harness/README.md](/F:/SourceCodes/Rust/merman/tools/dagre-harness/README.md)
  so it describes the pinned Mermaid `11.15.0` / `dagre-d3-es@7.0.14` toolchain instead of the old
  11.12-era package facts.

Focused verification:

- `cargo test -p dugong-graphlib --tests`
- `cargo test -p dugong --tests`
- `node tools/dagre-harness/run.mjs --help`
- `cargo run -p xtask -- compare-dagre-layout --fixture basic --out-dir target/compare/dagre-layout-hpd050-graphlib-audit`

Verification notes:

- `cargo run -p xtask -- compare-dagre-layout --help` still returns `Error: Usage`; that is the
  command's existing lack of help output, not a harness import failure.
- The focused `compare-dagre-layout` run for `fixtures/state/basic.mmd` completed and reported
  `max node delta: 0.000000` and `max edge delta: 0.000000`.
- This slice does not claim full Graphlib parity. The next useful audit target is the public
  `Graph` API subset consumed by `dugong` and Mermaid-facing renderers, not unused shortest-path
  algorithms.

Tenth slice Graphlib Graph core coverage:

- Added
  [crates/dugong-graphlib/tests/graph_core_test.rs](/F:/SourceCodes/Rust/merman/crates/dugong-graphlib/tests/graph_core_test.rs)
  as the first direct source-test slice from `repo-ref/graphlib/test/graph-test.js`.
- Covered current public Rust API equivalents for initial options, graph labels, node defaults,
  source queries, edge creation/update, named multiedges, path edges, parent/child moves, root
  children, and remove-node cleanup.
- Tightened `Graph::set_parent_ix(...)` so assigning a node under its own descendant panics with
  `set_parent would create a cycle`, matching upstream Graphlib's tree-invariant throw in Rust
  form.
- Updated
  [docs/dugong/GRAPHLIB_UPSTREAM_TEST_COVERAGE.md](/F:/SourceCodes/Rust/merman/docs/dugong/GRAPHLIB_UPSTREAM_TEST_COVERAGE.md)
  to classify `test/graph-test.js` as partially ported and list the mapped cases.

Focused verification:

- `cargo test -p dugong-graphlib --tests`
- `cargo test -p dugong --tests`
- `cargo test -p merman-render --test flowchart_layout_test`
- `cargo test -p merman-render --test state_layout_test`
- `cargo test -p merman-render --test class_layout_test`
- `cargo test -p merman-render --test er_layout_test`
- `cargo test -p dugong --tests`

Verification notes:

- The invalid non-compound `setParent(...)` upstream throw remains a deliberate open API-shape
  decision; current Rust methods still no-op on non-compound graphs. Do not change that casually
  without auditing downstream callers.

Eleventh slice Graphlib edge-query coverage:

- Extended the direct `repo-ref/graphlib/test/graph-test.js` coverage in
  [crates/dugong-graphlib/tests/graph_core_test.rs](/F:/SourceCodes/Rust/merman/crates/dugong-graphlib/tests/graph_core_test.rs)
  to cover `sinks`, `predecessors`, `successors`, `neighbors`, `isLeaf`, `inEdges`, `outEdges`,
  `nodeEdges`, and remove-edge adjacency updates.
- Added Rust API seams for source-backed Graphlib behavior that previously had no public equivalent:
  `Graph::sinks(...)`, `Graph::is_leaf(...)`, and `Graph::node_edges_between(...)`.
- Updated
  [docs/dugong/GRAPHLIB_UPSTREAM_TEST_COVERAGE.md](/F:/SourceCodes/Rust/merman/docs/dugong/GRAPHLIB_UPSTREAM_TEST_COVERAGE.md)
  so this slice is mapped to pinned upstream case names and the remaining JS/Rust API-shape
  differences are explicit.

Focused verification:

- `cargo test -p dugong-graphlib --tests`
- `cargo test -p dugong --tests`

Verification notes:

- Missing-node query behavior is intentionally not claimed as identical: upstream JS returns
  `undefined` for several query methods, while the current Rust collection API returns empty
  vectors.
- Upstream chainability for `removeEdge(...)` is not copied into Rust; tests cover the state and
  adjacency effects that matter to consumers.

Twelfth slice Graphlib edge invariant coverage:

- Tightened [crates/dugong-graphlib/src/graph/core.rs](/F:/SourceCodes/Rust/merman/crates/dugong-graphlib/src/graph/core.rs)
  so `set_edge_named(..., Some(name), ...)` on a non-multigraph panics instead of silently
  discarding the name.
- Edge lookup/removal views now keep the supplied name even for non-multigraphs, so
  `has_edge("a", "b", Some("name"))`, `edge(...)`, and `remove_edge(...)` no longer alias the
  unnamed simple edge.
- Added direct graph-test coverage for edge-key listing, directed vs. undirected edge lookup,
  missing edge lookup, named-edge rejection, named edge removal, and undirected remove-edge
  endpoint normalization.
- Production Mermaid-facing named-edge graph construction had already been audited as multigraph
  based, so this is an invariant fix rather than a forced renderer behavior change.

Focused verification:

- `cargo test -p dugong-graphlib --tests`
- `cargo test -p dugong --tests`
- `cargo test -p merman-render --test flowchart_layout_test`
- `cargo test -p merman-render --test state_layout_test`
- `cargo test -p merman-render --test class_layout_test`
- `cargo test -p merman-render --test er_layout_test`

Thirteenth slice Dagre reference adapter extraction:

- Extracted
  [crates/xtask/src/cmd/debug/dagre_reference.rs](/F:/SourceCodes/Rust/merman/crates/xtask/src/cmd/debug/dagre_reference.rs)
  as the Rust-side adapter for the Dagre JS reference harness.
- The adapter now owns the reference input JSON schema, Rust output snapshots, JS harness
  invocation, JS output parsing, max node/edge delta calculation, and the compound-edge endpoint
  normalization mirrored from `tools/dagre-harness/run.mjs`.
- `compare-dagre-layout` remains State-only in this slice. It now acts as a graph producer plus
  command wrapper, which keeps future Dagre-backed audits from copying the reference machinery.
- Added a unit test for compound-edge normalization so the extracted adapter is covered below the
  command-smoke level.

Focused verification:

- `cargo fmt --all`
- `cargo check -p xtask`
- `cargo test -p xtask compound_edge_normalization_moves_edges_to_non_cluster_child`
- `cargo test -p xtask`
- `node tools/dagre-harness/run.mjs --help`
- `cargo run -p xtask -- compare-dagre-layout --fixture basic --out-dir target\compare\dagre-layout-hpd050-reference-adapter`
- `cargo run -p xtask -- compare-dagre-layout --fixture stress_state_composite_with_external_edges_028 --out-dir target\compare\dagre-layout-hpd050-reference-adapter-composite`
- `cargo run -p xtask -- compare-dagre-layout --fixture stress_state_composite_with_external_edges_028 --cluster state-Big-7 --out-dir target\compare\dagre-layout-hpd050-reference-adapter-cluster`

Verification notes:

- The three focused layout comparisons all reported `max node delta: 0.000000` and
  `max edge delta: 0.000000`.
- This is an ARCH-022 seam cleanup, not a claim that the Dagre reference adapter now supports every
  diagram family. Add non-State graph producers only when a source-backed residual audit needs one.

Fourteenth slice Architecture Cytoscape label-extension seam:

- Added `ArchitectureCytoscapeServiceLabelExtension` in
  [crates/merman-render/src/architecture_metrics.rs](/F:/SourceCodes/Rust/merman/crates/merman-render/src/architecture_metrics.rs)
  so FCoSE node `BoundsExtras` and SVG root/group service-bounds estimation share the same
  Cytoscape service-label half-width and compound-label bottom-extension calculation.
- Kept SVG root `createText(...)` measurement separate from Cytoscape compound-child label
  measurement. This is a phase split, not a root-width tune.
- Added focused unit coverage for the shared extension and empty-title behavior.

Focused verification:

- `cargo fmt --all`
- `cargo test -p merman-render architecture_cytoscape_service_label_extension_centralizes_compound_label_phase --lib`
- `cargo test -p merman-render architecture_text_constants_match_mermaid --lib`
- `cargo test -p merman-render architecture_fcose_node_bounds_extras_feed_label_bounds --lib`
- `cargo test -p merman-render architecture_node_bbox_extras_convert_to_manatee_bounds_extras --lib`
- `cargo test -p merman-render --test architecture_svg_test`
- `cargo run -p xtask -- compare-architecture-svgs --check-dom --dom-mode parity --dom-decimals 3 --out target\compare\architecture_report_parity_after_hpd050_cy_label_extension.md`
- `cargo run -p xtask -- compare-architecture-svgs --check-dom --dom-mode parity-root --dom-decimals 3 --report-root-all --out target\compare\architecture_report_parity_root_after_hpd050_cy_label_extension.md`

Verification notes:

- Architecture structural parity remained green.
- Architecture parity-root remained the expected 26 mismatches.
- The root report top rows remained the known residual front, led by `junction_fork_join_026`,
  `batch5_long_titles_and_punct_076`, `html_titles_and_escapes_041`, and
  `batch6_init_fontsize_icon_size_wrap_093`.

## HPD-080 - Visible Rendering Defect Triage

First slice outcome:

- Promoted visible rendering defects above fine numeric root residuals in the workstream policy.
  DOM structural parity can be green while an SVG is still functionally broken if text is invisible,
  branch labels become dark blocks, diagram cards lose theme rules, or semantic color cues are not
  emitted.
- Audited pinned Mermaid `11.15.0` diagram style providers at commit
  `41646dfd43ac83f001b03c70605feb036afae46d`:
  - `packages/mermaid/src/diagrams/kanban/styles.ts`
  - `packages/mermaid/src/diagrams/packet/styles.ts`
  - `packages/mermaid/src/diagrams/sankey/styles.js`
  - `packages/mermaid/src/diagrams/c4/styles.js`
  - `packages/mermaid/src/diagrams/git/styles.js`
- Kanban now emits Mermaid 11.15 section/ticket/icon/label theme CSS. The user-provided metadata
  example renders readable cards and labels.
- Packet now maps Mermaid 11.15 `packet.*` style options into emitted CSS rather than hardcoding
  defaults.
- Sankey now emits config-aware info CSS plus source-backed label, node-label, outlined-label, and
  link rules.
- C4 now emits config-aware base CSS and source-backed `.person` theme colors.
- GitGraph now emits Mermaid 11.15 classic/default per-branch theme rules, including
  `.branch-labelN`, `.commitN`, `.commit-highlightN`, `.labelN`, `.arrowN`, and merge/reverse/
  highlight-inner colors. The user-provided three-branch merge graph now has readable branch labels
  and visible colored branch/merge paths.

Touched production surfaces:

- [crates/merman-render/src/svg/parity/packet.rs](/F:/SourceCodes/Rust/merman/crates/merman-render/src/svg/parity/packet.rs)
- [crates/merman-render/src/svg/parity/css.rs](/F:/SourceCodes/Rust/merman/crates/merman-render/src/svg/parity/css.rs)
- [crates/merman-render/src/svg/parity/sankey.rs](/F:/SourceCodes/Rust/merman/crates/merman-render/src/svg/parity/sankey.rs)
- [crates/merman-render/src/svg/parity/c4.rs](/F:/SourceCodes/Rust/merman/crates/merman-render/src/svg/parity/c4.rs)
- [crates/merman-render/src/svg/parity/gitgraph.rs](/F:/SourceCodes/Rust/merman/crates/merman-render/src/svg/parity/gitgraph.rs)

Focused verification:

- `cargo test -p merman-render kanban`
- `cargo test -p merman-render packet_css_honors_mermaid_11_15_packet_style_options`
- `cargo test -p merman-render sankey_css_honors_mermaid_11_15_theme_options`
- `cargo test -p merman-render c4_css_honors_mermaid_11_15_person_theme_options`
- `cargo test -p merman-render gitgraph_css_includes_mermaid_11_15_branch_theme_rules`
- `cargo run -p xtask -- compare-kanban-svgs --check-dom --dom-mode parity --dom-decimals 3`
- `cargo run -p xtask -- compare-packet-svgs --check-dom --dom-mode parity --dom-decimals 3`
- `cargo run -p xtask -- compare-sankey-svgs --check-dom --dom-mode parity --dom-decimals 3`
- `cargo run -p xtask -- compare-c4-svgs --check-dom --dom-mode parity --dom-decimals 3`
- `cargo run -p xtask -- compare-gitgraph-svgs --check-dom --dom-mode parity --dom-decimals 3`
- `cargo fmt --check -p merman-render`
- `git diff --check`

Manual render evidence:

- `target/compare/kanban_user_metadata.fixed.png`
- `target/compare/gitgraph_user_merge.png`

Negative / residual evidence:

- A broad-filter `cargo test -p merman-render mermaid_11_15` run also matched the pre-existing
  Sequence root-width residual test
  `sequence_long_leftof_notes_keep_mermaid_11_15_root_width`, which still fails for the documented
  note/root measurement tail. The new HPD-080 CSS tests passed; this slice does not claim Sequence
  root closure.

Second slice outcome:

- Audited additional pinned Mermaid 11.15 style providers with the HPD-080 visible-rendering lens:
  - `packages/mermaid/src/diagrams/gantt/styles.js`
  - `packages/mermaid/src/diagrams/treemap/styles.ts`
  - `packages/mermaid/src/diagrams/requirement/styles.js`
- Gantt previously discarded `effective_config` when emitting CSS. It now reads source-backed Gantt
  theme variables for section backgrounds, grid/today colors, task text, task bars, active/done/
  critical states, vertical markers, and title text.
- Gantt now emits Mermaid 11.15 outside done/doneCrit text contrast rules, preventing labels that
  move outside bars from inheriting the wrong bar-text color.
- Treemap now maps Mermaid 11.15 `treemap.*` style options and theme title/text colors for section,
  leaf, label, value, and title CSS.
- Requirement now maps Mermaid 11.15 requirement theme variables for requirement boxes, labels,
  relationship lines/labels, edge-label backgrounds, node text, and divider colors.
- Requirement intentionally did not emit the upstream `[data-look][data-color-id]` color scale
  rules because current local Requirement SVGs do not emit those attributes; inert CSS would not
  improve renderability.

Touched production surfaces:

- [crates/merman-render/src/svg/parity/css.rs](/F:/SourceCodes/Rust/merman/crates/merman-render/src/svg/parity/css.rs)
- [crates/merman-render/src/svg/parity/gantt.rs](/F:/SourceCodes/Rust/merman/crates/merman-render/src/svg/parity/gantt.rs)
- [crates/merman-render/src/svg/parity/treemap.rs](/F:/SourceCodes/Rust/merman/crates/merman-render/src/svg/parity/treemap.rs)

Focused verification:

- `cargo test -p merman-render css_honors_mermaid_11_15`
- `cargo run -p xtask -- compare-gantt-svgs --check-dom --dom-mode parity --dom-decimals 3`
- `cargo run -p xtask -- compare-treemap-svgs --check-dom --dom-mode parity --dom-decimals 3`
- `cargo run -p xtask -- compare-requirement-svgs --check-dom --dom-mode parity --dom-decimals 3`

Third slice outcome:

- Audited pinned Mermaid 11.15 Mindmap style provider:
  - `packages/mermaid/src/diagrams/mindmap/styles.ts`
- Mindmap previously emitted the default section palette as fixed CSS and only colored
  `.section-2 span`, while the local renderer actually emits label text through
  `<span class="nodeLabel">...`. Custom `themeVariables.cScaleLabel*` values therefore did not
  apply to most rendered labels.
- Mindmap now reads Mermaid 11.15 `THEME_COLOR_LIMIT`, `cScale*`, `cScaleLabel*`, `cScaleInv*`,
  `git0`, `gitBranchLabel0`, `nodeBorder`, `theme`, and `look` when emitting section/root CSS.
- Section `span` labels and node icons now follow source-backed `cScaleLabel*` rules. Root text and
  root `span` labels now follow `gitBranchLabel0`, or `nodeBorder` for redux-style themes, matching
  the upstream visible color rule.
- `look: neo` now uses Mermaid 11.15's source-backed Mindmap edge-depth stroke width formula.
- The upstream `[data-look="neo"]` gradient/drop-shadow rules were intentionally not emitted in
  this slice because current local Mindmap SVG nodes do not emit the required `data-look`
  attributes. Adding those inert rules would not improve renderability and would create a false
  parity signal.

Touched production surfaces:

- [crates/merman-render/src/svg/parity/mindmap.rs](/F:/SourceCodes/Rust/merman/crates/merman-render/src/svg/parity/mindmap.rs)

Focused verification:

- `cargo test -p merman-render mindmap_css_honors_mermaid_11_15_theme_sections`
- `cargo run -p xtask -- compare-mindmap-svgs --check-dom --dom-mode parity --dom-decimals 3`

Fourth slice outcome:

- Audited pinned Mermaid 11.15 Pie style provider:
  - `packages/mermaid/src/diagrams/pie/pieStyles.ts`
- Pie previously emitted fixed default CSS through `pie_css(diagram_id)` and discarded
  `effective_config`, so Mermaid 11.15 pie style variables could not affect slice stroke/opacity,
  outer ring stroke, title text, slice labels, or legend text. This is a visible theme/readability
  defect, especially for dark or custom themes.
- Pie CSS now reads Mermaid 11.15 `themeVariables` for `pieStrokeColor`, `pieStrokeWidth`,
  `pieOpacity`, `pieOuterStrokeColor`, `pieOuterStrokeWidth`, `pieTitleTextSize`,
  `pieTitleTextColor`, `pieSectionTextSize`, `pieSectionTextColor`, `pieLegendTextSize`, and
  `pieLegendTextColor`, while preserving the default-theme fallback values from the upstream theme.
- `pie_css` now accepts `effective_config`, uses config-aware base CSS parts, and keeps `:root`
  last like the other HPD-080 style emitters.
- Removed the now-unused fixed `info_css(...)` wrapper; remaining emitters use either
  `info_css_into(...)` directly or config-aware CSS parts.

Touched production surfaces:

- [crates/merman-render/src/svg/parity/css.rs](/F:/SourceCodes/Rust/merman/crates/merman-render/src/svg/parity/css.rs)
- [crates/merman-render/src/svg/parity/pie.rs](/F:/SourceCodes/Rust/merman/crates/merman-render/src/svg/parity/pie.rs)

Focused verification:

- `cargo test -p merman-render pie_css_honors_mermaid_11_15_theme_options`
- `cargo run -p xtask -- compare-pie-svgs --check-dom --dom-mode parity --dom-decimals 3`

## HPD-060 - Semantic / Render Unification Pilot

Outcome:

- Selected Sequence as the bounded pilot because it already had a typed render model and an obvious
  duplicate compatibility JSON construction path.
- Added `SequenceDiagramRenderModel::to_compat_json(...)` in
  [crates/merman-core/src/diagrams/sequence/render_model.rs](/F:/SourceCodes/Rust/merman/crates/merman-core/src/diagrams/sequence/render_model.rs)
  as the compatibility adapter from typed model to legacy JSON shape.
- Replaced the manual `SequenceDb::into_model(...)` JSON builder in
  [crates/merman-core/src/diagrams/sequence/db.rs](/F:/SourceCodes/Rust/merman/crates/merman-core/src/diagrams/sequence/db.rs)
  with `self.into_render_model().to_compat_json(...)`.
- Kept compatibility JSON field semantics explicit:
  - top-level `type` and `constants.placement` are still added by the adapter,
  - ordinary message `placement` is omitted when absent,
  - `centralConnection` is omitted when zero,
  - `from` / `to` still serialize as nullable compatibility fields.
- Expanded the typed-vs-JSON parse test in
  [crates/merman-core/src/tests/misc.rs](/F:/SourceCodes/Rust/merman/crates/merman-core/src/tests/misc.rs)
  to cover actor order, messages, notes, boxes, create/destroy actor indexes, and omitted optional
  message fields.

Focused verification:

- `cargo fmt --all`
- `cargo test -p merman-core parse_sequence_render_model_uses_typed_variant_without_changing_json_parse --lib`
- `cargo test -p merman-core sequence --lib`
- `cargo test -p merman-core --lib`
- `cargo test -p merman-render sequence_long_leftof_notes_keep_mermaid_11_15_note_width --test sequence_svg_test`
- `cargo run -p xtask -- compare-sequence-svgs --check-dom --dom-mode parity --dom-decimals 3 --out target\compare\sequence_report_parity_after_hpd060_typed_projection.md`
- `cargo run -p xtask -- compare-sequence-svgs --check-dom --dom-mode parity-root --dom-decimals 3 --report-root-all --out target\compare\sequence_report_parity_root_after_hpd060_typed_projection.md`

Verification notes:

- Sequence structural parity remained green: the post-HPD-060 `parity` report says all fixtures
  matched.
- The Sequence `parity-root` report remains an expected failure with `28` dom mismatches. The top
  rows are existing measurement/root residuals, led by long left-of wrapped notes at `+19px` and
  math/line-break rows.
- Full `cargo test -p merman-render --test sequence_svg_test` still fails on existing measurement
  gates:
  - `sequence_note_width_expands_for_literal_br_backslash_t_in_vendored_mode` reports local
    `152.0` vs expected `151.0`,
  - `sequence_long_leftof_notes_keep_mermaid_11_15_root_width` remains the documented long-note
    root-width residual.
- This pilot does not claim repo-wide semantic/render unification. It proves the narrower pattern:
  use one typed semantic source, then project compatibility JSON as an adapter instead of keeping a
  second parser-owned JSON master.

## HPD-070 - Unsupported-Family Rubric

Outcome:

- Added
  [docs/alignment/UNSUPPORTED_FAMILY_ADMISSION_RUBRIC.md](/F:/SourceCodes/Rust/merman/docs/alignment/UNSUPPORTED_FAMILY_ADMISSION_RUBRIC.md)
  as the durable admission policy for unsupported Mermaid families.
- Updated the Mermaid 11.15 unsupported-family table in
  [docs/alignment/STATUS.md](/F:/SourceCodes/Rust/merman/docs/alignment/STATUS.md) so it uses the
  locked Mermaid source commit rather than the current `repo-ref/mermaid` working tree when those
  diverge.
- Classified pinned Mermaid 11.15 unsupported families in priority order:
  1. `treeView-beta` header / `treeView` id
  2. `ishikawa` / `ishikawa-beta`
  3. `eventmodeling`
  4. `venn-beta`
  5. `wardley-beta`
- Marked `railroad-*` and `cynefin-beta` as not part of the Mermaid 11.15 parity backlog because
  they are absent from the pinned `41646dfd...` source tree.

Source evidence:

- `git -C repo-ref/mermaid ls-tree --name-only 41646dfd43ac83f001b03c70605feb036afae46d:packages/mermaid/src/diagrams`
  listed `eventmodeling`, `ishikawa`, `treeView`, `venn`, and `wardley`, but not `railroad` or
  `cynefin`.
- Pinned `diagram-orchestration.ts` registers lazy detector ids for `eventmodeling`, `treeView`,
  `ishikawa`, `venn`, and `wardley`. The `treeView` detector accepts the `treeView-beta` header.
- Pinned source shape used for prioritization:
  - `treeView`: parser `16` lines, DB `69`, renderer `114`
  - `ishikawa`: parser `45`, DB `79`, renderer `468`
  - `eventmodeling`: parser `25`, DB `602`, renderer `138`
  - `venn`: parser `110`, DB `116`, renderer `336`, plus `@upsetjs/venn.js`
  - `wardley`: parser `218`, DB `138`, renderer `971`, plus `WardleyBuilder`

Verification:

- `git -C repo-ref/mermaid ls-tree ...` and pinned-source `git show ...` checks listed in the
  journal.
- `rg --files fixtures docs | rg "treeView|treeview|venn|ishikawa|eventmodeling|wardley|railroad|cynefin"`
  found no local fixtures/docs besides the new rubric/status updates, confirming these families are
  not already partially admitted locally.
- JSONL validation for `CONTEXT.jsonl`, `TASKS.jsonl`, and `CAMPAIGNS.jsonl`.

Review notes:

- `treeView` is the recommended first new-family workstream only when new-family implementation is
  actually approved. HPD-070 does not start that work.
- `venn` should not be implemented with a guessed local circle layout. It needs either a port or
  source-backed audit of `@upsetjs/venn.js`.
- `railroad-*` and `cynefin-beta` may exist in newer Mermaid development branches, but they are not
  part of the current pinned 11.15 scope.
