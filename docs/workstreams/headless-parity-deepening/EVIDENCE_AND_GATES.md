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
