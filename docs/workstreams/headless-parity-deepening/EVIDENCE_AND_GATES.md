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

Fifteenth slice Architecture disconnected-islands root-bounds audit:

- Re-audited `stress_architecture_disconnected_islands_046`, a height-only residual where width is
  already aligned: upstream `823.346x768.460`, current local `823.346x775.647`.
- Source check: pinned Mermaid 11.15 `setupGraphViewbox(...)` uses browser `svg.getBBox()` plus
  padding for both size attributes and `viewBox`.
- Fresh `debug-svg-bbox` evidence shows local emitted geometry alone is too short
  (`823.346x751.460` with padding), while the final local root is too tall only after the root
  viewport code unions synthetic `content_bounds` for label extents.
- A temporary experiment that used `cytoscape_group_child_bounds` for top-level services fixed this
  single fixture, but full Architecture `parity-root` mismatches grew from `26` to `84`. The
  experiment was rejected and reverted.
- Conclusion: the next fix must be a phase-specific root label contribution model. Do not globally
  collapse top-level services from `svg_root_bounds` to Cytoscape group-child bounds.

Focused verification:

- `cargo run -p xtask -- compare-architecture-svgs --filter stress_architecture_disconnected_islands_046 --check-dom --dom-mode parity-root --dom-decimals 3 --report-root-all --out target\compare\architecture_disconnected_islands_current_hpd050_audit.md`
  failed as expected on the current residual.
- `cargo run -p xtask -- debug-svg-bbox --svg fixtures\upstream-svgs\architecture\stress_architecture_disconnected_islands_046.svg --padding 40`
- `cargo run -p xtask -- debug-svg-bbox --svg target\compare\architecture\stress_architecture_disconnected_islands_046.svg --padding 40`
- `Select-String` counts confirmed current full root report has `26` mismatches, while the rejected
  top-level Cytoscape experiment report has `84`.

Sixteenth slice Architecture isolated top-level service root-bounds seam:

- Added `architecture_top_level_service_root_bounds(...)` in
  [crates/merman-render/src/architecture_metrics.rs](/F:/SourceCodes/Rust/merman/crates/merman-render/src/architecture_metrics.rs)
  so the top-level service root contribution decision is explicit instead of hidden in the SVG
  renderer loop.
- The rule is deliberately narrow: `cytoscape_group_child_bounds` is used only for isolated
  top-level services in diagrams that also contain groups. Connected top-level services and
  no-group singleton/iconText rows keep `svg_root_bounds`.
- This closes `stress_architecture_disconnected_islands_046` without applying the rejected global
  top-level-service switch that regressed full Architecture root mismatches from `26` to `84`.
- Full Architecture structural parity remains green. Full Architecture `parity-root` remains an
  expected failure, but its mismatch count moved from `26` to `25`.
- This is a phase-specific root contribution seam, not a browser-exact text measurement claim.
  Remaining residuals still need source/evidence-backed audits rather than constants.

Focused verification:

- `cargo fmt -p merman-render`
- `cargo test -p merman-render architecture_top_level_service_root_bounds_splits_isolated_group_component_phase --lib`
- `cargo fmt --check -p merman-render`
- `cargo test -p merman-render --test architecture_svg_test`
- `cargo run -p xtask -- compare-architecture-svgs --check-dom --dom-mode parity --dom-decimals 3 --out target\compare\architecture_report_parity_after_hpd050_isolated_root_bounds.md`
  passed.
- `cargo run -p xtask -- compare-architecture-svgs --filter stress_architecture_disconnected_islands_046 --check-dom --dom-mode parity-root --dom-decimals 3 --report-root-all --out target\compare\architecture_disconnected_islands_isolated_service_experiment.md`
  passed with upstream/local root `823.346x768.460`.
- `cargo run -p xtask -- compare-architecture-svgs --check-dom --dom-mode parity-root --dom-decimals 3 --report-root-all --out target\compare\architecture_report_parity_root_after_hpd050_isolated_root_bounds.md`
  remains an expected failure with `25` dom mismatches.

Seventeenth slice Graphlib filter/default-label API coverage:

- Added source-backed `Graph::filter_nodes(...)` in
  [crates/dugong-graphlib/src/graph/core.rs](/F:/SourceCodes/Rust/merman/crates/dugong-graphlib/src/graph/core.rs).
  The method copies selected nodes, graph label, options, edge labels whose endpoints remain in the
  filtered graph, and Graphlib's compound parent promotion behavior when an intermediate parent is
  filtered out.
- Deepened Graphlib default-label parity without adding a global graph type constraint:
  `set_default_node_label(...)` and `set_default_edge_label(...)` keep the existing no-arg Rust API,
  while `set_default_node_label_with_id(...)` and
  `set_default_edge_label_with_endpoints(...)` expose Graphlib's source-backed callback arguments.
- `filter_nodes(...)` has method-level `Clone` bounds only for the copied graph labels. Ordinary
  Dagre graph construction and layout mutation remain unconstrained.
- Updated
  [docs/dugong/GRAPHLIB_UPSTREAM_TEST_COVERAGE.md](/F:/SourceCodes/Rust/merman/docs/dugong/GRAPHLIB_UPSTREAM_TEST_COVERAGE.md)
  to map the newly ported `repo-ref/graphlib/test/graph-test.js` cases.
- This is public Graph API parity work, not a renderer tune and not a move toward unused Graphlib
  shortest-path algorithms.

Focused verification:

- `$env:RUSTFLAGS='-C linker=rust-lld'; cargo test -p dugong-graphlib --test graph_core_test`
  passed with `52` tests.
- `$env:RUSTFLAGS='-C linker=rust-lld'; cargo nextest run -p dugong-graphlib`
  passed with `78` tests.
- `$env:RUSTFLAGS='-C linker=rust-lld'; cargo nextest run -p dugong`
  passed with `267` tests.

Eighteenth slice Graphlib children/root API coverage:

- Added `Graph::children_opt(...)` in
  [crates/dugong-graphlib/src/graph/core.rs](/F:/SourceCodes/Rust/merman/crates/dugong-graphlib/src/graph/core.rs)
  as a narrow optional-return seam for Graphlib's `children(v)` shape. It returns `None` for a
  missing queried node, `Some([])` for an existing node with no children, and direct children for
  compound nodes.
- Kept the existing ergonomic `children(parent) -> Vec<&str>` behavior unchanged so current Rust
  callers that expect empty vectors do not regress.
- Reused the existing `children_root()` API as the Rust mapping for Graphlib's no-argument
  `children()` root query. For non-compound graphs it returns all nodes; for compound graphs it
  returns nodes without a parent.
- Updated the Graphlib coverage ledger so pinned `repo-ref/graphlib/test/graph-test.js`
  `children` cases map to concrete Rust tests instead of remaining implicit under parent tests.
- This is a public Graph API shape seam. It does not force JS overloads, `undefined`, chainability,
  ID stringification, or non-compound `setParent(...)` throws into the existing Rust APIs.

Focused verification:

- `$env:RUSTFLAGS='-C linker=rust-lld'; cargo nextest run -p dugong-graphlib children_opt`
  failed first because `Graph::children_opt(...)` did not exist.
- `$env:RUSTFLAGS='-C linker=rust-lld'; cargo nextest run -p dugong-graphlib children_opt`
  passed after the seam was implemented.
- `$env:RUSTFLAGS='-C linker=rust-lld'; cargo nextest run -p dugong-graphlib children`
  passed with `3` tests.
- `$env:RUSTFLAGS='-C linker=rust-lld'; cargo nextest run -p dugong-graphlib`
  passed with `80` tests.
- `$env:RUSTFLAGS='-C linker=rust-lld'; cargo nextest run -p dugong`
  passed with `267` tests.

Nineteenth slice Graphlib setPath label API coverage:

- Added `Graph::set_path_with_label(...)` in
  [crates/dugong-graphlib/src/graph/core.rs](/F:/SourceCodes/Rust/merman/crates/dugong-graphlib/src/graph/core.rs)
  as the Rust mapping for Graphlib's `setPath(nodes, value)` behavior.
- The method sets the same label on every edge in the path and updates existing edge labels,
  matching pinned `repo-ref/graphlib/lib/graph.js` `setPath(...)`.
- The `Clone` bound is method-scoped to this batch-label API. Ordinary graph construction and
  layout mutation still do not require cloneable edge labels.
- Updated the Graphlib coverage ledger so `setPath / can set a value for all of the edges` maps to
  a concrete Rust regression.

Focused verification:

- `$env:RUSTFLAGS='-C linker=rust-lld'; cargo nextest run -p dugong-graphlib set_path_with_label`
  failed first because `Graph::set_path_with_label(...)` did not exist.
- `$env:RUSTFLAGS='-C linker=rust-lld'; cargo nextest run -p dugong-graphlib set_path_with_label`
  passed after the seam was implemented.
- `$env:RUSTFLAGS='-C linker=rust-lld'; cargo nextest run -p dugong-graphlib`
  passed with `81` tests.
- `$env:RUSTFLAGS='-C linker=rust-lld'; cargo nextest run -p dugong`
  passed with `267` tests.

Twentieth slice Graphlib setNodes API coverage:

- Added `Graph::set_nodes(...)` and `Graph::set_nodes_with_label(...)` in
  [crates/dugong-graphlib/src/graph/core.rs](/F:/SourceCodes/Rust/merman/crates/dugong-graphlib/src/graph/core.rs)
  as the Rust mapping for Graphlib's `setNodes(nodes)` and `setNodes(nodes, value)` behavior.
- `set_nodes(...)` reuses the existing default node label seam and preserves existing node labels,
  matching `setNode(v)` no-value behavior in pinned `repo-ref/graphlib/lib/graph.js`.
- `set_nodes_with_label(...)` sets and updates one label across every listed node with only a
  method-scoped `N: Clone` bound.
- Updated the Graphlib coverage ledger so the source `setNodes` cases map to concrete Rust
  regressions. This is a small graph-construction seam, not JS argument overloading or chainable API
  porting.

Focused verification:

- `$env:RUSTFLAGS='-C linker=rust-lld'; cargo nextest run -p dugong-graphlib set_nodes`
  failed first because `Graph::set_nodes(...)` and `Graph::set_nodes_with_label(...)` did not
  exist.
- `$env:RUSTFLAGS='-C linker=rust-lld'; cargo nextest run -p dugong-graphlib set_nodes`
  passed after the seam was implemented.
- `cargo fmt --check -p dugong-graphlib`
  passed.
- `$env:RUSTFLAGS='-C linker=rust-lld'; cargo nextest run -p dugong-graphlib`
  passed with `83` tests.
- `$env:RUSTFLAGS='-C linker=rust-lld'; cargo nextest run -p dugong`
  passed with `267` tests.

Twenty-first slice Graphlib parent/clear-parent coverage:

- Added direct regression coverage in
  [crates/dugong-graphlib/tests/graph_core_test.rs](/F:/SourceCodes/Rust/merman/crates/dugong-graphlib/tests/graph_core_test.rs)
  for Graphlib's `parent(v)` optional query shape: non-compound graphs, missing nodes, unassigned
  compound nodes, and assigned compound parents.
- Extended `clear_parent_returns_node_to_root_children` to cover idempotent parent removal, mapping
  Graphlib's `setParent(v)` / `setParent(v, undefined)` clear-parent state behavior onto Rust's
  explicit `clear_parent(v)` API.
- Updated the Graphlib coverage ledger so source `setParent` parent-removal and `parent` query cases
  are no longer implicit assumptions behind Dugong/renderer usage.
- No production code changed in this slice. Current Rust APIs already matched the state behavior,
  while JS chainability, optional-argument overloading, and ID stringification remain explicit shape
  differences.

Focused verification:

- `$env:RUSTFLAGS='-C linker=rust-lld'; cargo nextest run -p dugong-graphlib parent`
  passed with `9` tests.
- `cargo fmt --check -p dugong-graphlib`
  passed.
- `$env:RUSTFLAGS='-C linker=rust-lld'; cargo nextest run -p dugong-graphlib`
  passed with `84` tests.
- `$env:RUSTFLAGS='-C linker=rust-lld'; cargo nextest run -p dugong`
  passed with `267` tests.

Twenty-second slice Graphlib setEdge optional-label / EdgeKey coverage:

- Added direct regressions in
  [crates/dugong-graphlib/tests/graph_core_test.rs](/F:/SourceCodes/Rust/merman/crates/dugong-graphlib/tests/graph_core_test.rs)
  for Graphlib's `setEdge(..., undefined)` state behavior. Rust maps this through explicit
  `Option<T>` edge labels: `Some(None)` clears an existing optional label while keeping the edge
  present.
- Added `set_edge_key_sets_simple_and_named_edge_labels` so Graphlib's edge-object and multi-edge
  object `setEdge({ v, w, name }, value)` cases map to the Rust `EdgeKey` API.
- Updated the Graphlib coverage ledger so these source `setEdge` cases are no longer implicit.
- No production code changed in this slice. JS stringification, chainability, and argument
  overloading remain explicit Rust/JS API-shape differences.

Focused verification:

- `$env:RUSTFLAGS='-C linker=rust-lld'; cargo nextest run -p dugong-graphlib set_edge`
  failed first on an unnamed `EdgeKey::new(..., None)` type-inference issue in the new test.
- `$env:RUSTFLAGS='-C linker=rust-lld'; cargo nextest run -p dugong-graphlib set_edge`
  passed after spelling the unnamed edge object as `None::<String>`.

Full verification:

- `cargo fmt --check -p dugong-graphlib` passed.
- `$env:RUSTFLAGS='-C linker=rust-lld'; cargo nextest run -p dugong-graphlib`
  passed with `87` tests.
- `$env:RUSTFLAGS='-C linker=rust-lld'; cargo nextest run -p dugong`
  passed with `267` tests.
- JSONL validation passed for `CONTEXT.jsonl`, `TASKS.jsonl`, and `CAMPAIGNS.jsonl`.
- `git diff --check` passed.

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

Fifth slice outcome:

- Audited pinned Mermaid 11.15 Journey style provider:
  - `packages/mermaid/src/diagrams/user-journey/styles.js`
- Journey previously mixed layout-derived presentation attributes with fixed CSS fallback colors.
  This is visible because class CSS such as `.task-type-*` and `.section-type-*` overrides SVG
  `fill` presentation attributes, so fixed CSS can make configured section/task colors appear to
  be ignored.
- Journey CSS now reads Mermaid 11.15 theme variables for `faceColor`, `mainBkg`, `nodeBorder`,
  `arrowheadColor`, `edgeLabelBackground`, `titleColor`, `tertiaryColor`, `border2`, `fillType0`
  through `fillType7`, and optional `actor0` through `actor5`.
- The generic `line` rule now follows upstream `textColor`; edge/flowchart link rules still use
  source-backed `lineColor`.
- Actor color CSS is only emitted when the corresponding `themeVariables.actorN` exists. This keeps
  default layout-derived actor colors intact while allowing theme variables to override the visible
  SVG when they are provided.

Touched production surfaces:

- [crates/merman-render/src/svg/parity/journey.rs](/F:/SourceCodes/Rust/merman/crates/merman-render/src/svg/parity/journey.rs)

Focused verification:

- `cargo test -p merman-render journey_css_honors_mermaid_11_15_theme_options`
- `cargo run -p xtask -- compare-journey-svgs --check-dom --dom-mode parity --dom-decimals 3`

Sixth slice outcome:

- Audited pinned Mermaid 11.15 ER style provider:
  - `packages/mermaid/src/diagrams/er/styles.ts`
- ER previously emitted older hardcoded default-theme CSS for entity boxes, relationship labels,
  node shapes, relationship lines, markers, and label text. This could make custom/dark-theme ER
  diagrams structurally valid but visually stale or low contrast.
- ER CSS now reads Mermaid 11.15 theme variables for `mainBkg`, `nodeBorder`, `nodeTextColor`,
  `textColor`, `lineColor`, `errorBkgColor`, `errorTextColor`, `tertiaryColor`,
  `edgeLabelBackground`, optional `erEdgeLabelBackground`, and `strokeWidth` when `look: neo`.
- Added a narrow render-side `css_rgba_fade(...)` utility for the ER `fade(tertiaryColor, 0.5)`
  rule. It uses the existing `svgtypes` CSS color parser and returns `None` for unresolved runtime
  CSS values rather than pretending browser expression support.
- The upstream ER `[data-look][data-color-id]` color-theme rules and `[data-look=neo].labelBkg`
  rule were intentionally not emitted because current local ER SVGs do not emit the required
  attributes on those elements. Adding those inert rules would not improve visible renderability.

Touched production surfaces:

- [crates/merman-render/src/svg/parity.rs](/F:/SourceCodes/Rust/merman/crates/merman-render/src/svg/parity.rs)
- [crates/merman-render/src/svg/parity/css.rs](/F:/SourceCodes/Rust/merman/crates/merman-render/src/svg/parity/css.rs)
- [crates/merman-render/src/svg/parity/util.rs](/F:/SourceCodes/Rust/merman/crates/merman-render/src/svg/parity/util.rs)

Focused verification:

- `cargo test -p merman-render er_css_honors_mermaid_11_15_theme_options`
- `cargo test -p merman-render css_rgba_fade_parses_css_colors`
- `cargo run -p xtask -- compare-er-svgs --check-dom --dom-mode parity --dom-decimals 3`

Seventh slice outcome:

- Audited pinned Mermaid 11.15 Radar style provider:
  - `packages/mermaid/src/diagrams/radar/styles.ts`
- Radar already consumed `themeVariables.radar.*`, but ignored top-level `radar.*` style overrides.
  Mermaid 11.15 merges `themeVariables.radar` with the top-level `radar` config before emitting
  CSS, so user overrides such as `radar.axisColor`, `radar.curveOpacity`, and
  `radar.graticuleStrokeWidth` should affect visible output.
- Radar CSS now resolves `radar.<styleKey>` before `themeVariables.radar.<styleKey>` for axis,
  graticule, curve, and legend style options while keeping `cScale*` palette rules sourced from
  theme variables.

Touched production surfaces:

- [crates/merman-render/src/svg/parity/radar.rs](/F:/SourceCodes/Rust/merman/crates/merman-render/src/svg/parity/radar.rs)

Focused verification:

- `cargo test -p merman-render radar_css_honors_top_level_style_overrides`
- `cargo run -p xtask -- compare-radar-svgs --check-dom --dom-mode parity --dom-decimals 3`

Eighth slice outcome:

- Audited pinned Mermaid 11.15 Block style provider:
  - `packages/mermaid/src/diagrams/block/styles.ts`
- Block already consumed most theme variables, but composite cluster CSS used raw `clusterBkg` and
  `clusterBorder` values. Mermaid 11.15 fades those colors for `.node .cluster`, which affects the
  visual weight and readability of nested block regions.
- Block cluster CSS now applies `css_rgba_fade(clusterBkg, 0.5)` and
  `css_rgba_fade(clusterBorder, 0.2)` when colors are parseable, with fallback to the configured
  value for unresolved runtime CSS expressions.

Touched production surfaces:

- [crates/merman-render/src/svg/parity/block.rs](/F:/SourceCodes/Rust/merman/crates/merman-render/src/svg/parity/block.rs)
- [crates/merman-render/tests/block_svg_test.rs](/F:/SourceCodes/Rust/merman/crates/merman-render/tests/block_svg_test.rs)

Focused verification:

- `cargo test -p merman-render block_svg_fades_cluster_theme_colors --test block_svg_test`
- `cargo run -p xtask -- compare-block-svgs --check-dom --dom-mode parity --dom-decimals 3`

Ninth slice outcome:

- Audited pinned Mermaid 11.15 Sequence style provider:
  - `packages/mermaid/src/diagrams/sequence/styles.js`
- Sequence CSS previously still mirrored an older hardcoded stylesheet: actor boxes, actor labels,
  lifelines, signal lines, message labels, label boxes, loop/section titles, notes, activation
  bars, base markers, and root text color ignored effective Mermaid theme variables.
- Sequence CSS now receives `effective_config`, uses the shared `SvgTheme` seam, and maps the
  source-backed Mermaid 11.15 options for `actorBorder`, `actorBkg`, `strokeWidth`, `actorTextColor`,
  `actorLineColor`, `signalColor`, `sequenceNumberColor`, `signalTextColor`,
  `labelBoxBorderColor`, `labelBoxBkgColor`, `labelTextColor`, `loopTextColor`,
  `noteBorderColor`, `noteBkgColor`, `noteTextColor`, optional numeric/string
  `noteFontWeight`, `activationBkgColor`, `activationBorderColor`, `nodeBorder`, `dropShadow`,
  `textColor`, `lineColor`, and error colors.
- Added `SvgTheme::optional_value(...)` so non-color CSS values such as `noteFontWeight` are not
  read through the misleading `optional_color(...)` API.
- Upstream Sequence `data-look` / `outer-path` neo selectors remain intentionally un-emitted in
  this slice because current local Sequence SVG elements do not emit those attributes; emitted CSS
  is limited to selectors that affect current render output.

Touched production surfaces:

- [crates/merman-render/src/svg/parity/sequence/css.rs](/F:/SourceCodes/Rust/merman/crates/merman-render/src/svg/parity/sequence/css.rs)
- [crates/merman-render/src/svg/parity/sequence/render.rs](/F:/SourceCodes/Rust/merman/crates/merman-render/src/svg/parity/sequence/render.rs)
- [crates/merman-render/src/svg/parity/util.rs](/F:/SourceCodes/Rust/merman/crates/merman-render/src/svg/parity/util.rs)
- [crates/merman-render/tests/sequence_svg_test.rs](/F:/SourceCodes/Rust/merman/crates/merman-render/tests/sequence_svg_test.rs)

Focused verification:

- `cargo fmt -p merman-render`
- `cargo fmt --check -p merman-render`
- `cargo test -p merman-render sequence_css_uses_configured_font_size`
- `cargo test -p merman-render sequence_css_honors_mermaid_11_15_theme_options`
- `cargo test -p merman-render sequence_svg_honors_mermaid_11_15_theme_css_options --test sequence_svg_test`
- `cargo run -p xtask -- compare-sequence-svgs --check-dom --dom-mode parity --dom-decimals 3`

Residual note:

- This slice fixes Sequence theme/readability CSS emission only. It does not claim to close the
  known Sequence generated-measurement/root-width residuals documented under HPD-040 and HPD-060.

Tenth slice outcome:

- Audited pinned Mermaid 11.15 State style provider:
  - `packages/mermaid/src/diagrams/state/styles.js`
- State CSS previously emitted a mostly hardcoded 11.12-era stylesheet for state nodes, clusters,
  transitions, labels, notes, start/end markers, special states, and title text. This made dark and
  custom themes visually stale even when DOM structural parity stayed green.
- State CSS now reads Mermaid 11.15 theme variables through the shared `SvgTheme` seam for root
  text, error colors, `lineColor`, `transitionColor`, `nodeBorder`, `stateLabelColor`, `mainBkg`,
  `background`, `altBackground`, `strokeWidth`, `note*`, `labelBackgroundColor`,
  `edgeLabelBackground`, `transitionLabelColor`/`tertiaryTextColor`, `specialStateColor`,
  `innerEndBackground`, `compositeBackground`, `stateBkg`, `stateBorder`, and
  `compositeTitleBackground`.
- The local State marker id is prefixed (`<diagram>_stateDiagram-barbEnd`), so the stylesheet now
  uses source-backed suffix selectors (`[id$="-barbEnd"]`) instead of the old exact
  `#statediagram-barbEnd` selector that did not hit current output.
- The old `dependencyStart` / `dependencyEnd` CSS rule was removed from local State CSS because
  current local State SVG output does not emit those dependency marker ids.
- Upstream State neo cluster gradient/drop-shadow selectors remain intentionally un-emitted in this
  slice because local State rendering does not emit the corresponding gradient/drop-shadow defs.

Touched production surfaces:

- [crates/merman-render/src/svg/parity/state/style.rs](/F:/SourceCodes/Rust/merman/crates/merman-render/src/svg/parity/state/style.rs)
- [crates/merman-render/tests/state_svg_test.rs](/F:/SourceCodes/Rust/merman/crates/merman-render/tests/state_svg_test.rs)

Focused verification:

- `cargo fmt -p merman-render`
- `cargo fmt --check -p merman-render`
- `cargo test -p merman-render state_css_honors_mermaid_11_15_theme_options`
- `cargo test -p merman-render state_svg_honors_mermaid_11_15_theme_css_options --test state_svg_test`
- `cargo test -p merman-render --test state_svg_test`
- `cargo run -p xtask -- compare-state-svgs --check-dom --dom-mode parity --dom-decimals 3`

Residual note:

- This slice fixes State theme/readability CSS only. It does not claim State layout or root-bounds
  closure.

Eleventh slice outcome:

- Audited pinned Mermaid 11.15 Flowchart style provider:
  - `packages/mermaid/src/diagrams/flowchart/styles.ts`
- Flowchart CSS previously hardcoded node strokes to `1px` and edge-path strokes to `2.0px`.
  Mermaid 11.15 drives both from `themeVariables.strokeWidth` and appends `px` in the Flowchart
  stylesheet. This is visible for `theme: neo`, redux themes, and custom numeric
  `themeVariables.strokeWidth` values.
- Flowchart CSS now reads `strokeWidth` through `SvgTheme::css_value(...)`, preserving numeric
  theme defaults and user overrides instead of treating the value as a color or ignoring it.
- A focused Class audit during this slice found an unrelated structural namespace issue:
  `Outer.Foo --> Bar` style relations still differ from the pinned upstream fixture because local
  Class rendering collapses namespace-qualified relation ids that upstream keeps fully qualified
  and, in some cases, as a separate top-level class. That is recorded as a future Class semantic
  slice, not hidden inside this Flowchart CSS change.

Touched production surfaces:

- [crates/merman-render/src/svg/parity/flowchart/css.rs](/F:/SourceCodes/Rust/merman/crates/merman-render/src/svg/parity/flowchart/css.rs)
- [crates/merman-render/tests/flowchart_svg_test.rs](/F:/SourceCodes/Rust/merman/crates/merman-render/tests/flowchart_svg_test.rs)

Focused verification:

- `cargo fmt -p merman-render`
- `cargo fmt --check -p merman-render`
- `cargo test -p merman-render flowchart_svg_honors_mermaid_11_15_numeric_stroke_width_theme --test flowchart_svg_test`
- `cargo run -p xtask -- compare-flowchart-svgs --check-dom --dom-mode parity --dom-decimals 3`

Residual note:

- This slice fixes Flowchart theme/readability CSS only. It does not claim Flowchart root-bounds
  closure.
- `cargo run -p xtask -- compare-class-svgs --check-dom --dom-mode parity --dom-decimals 3` was
  run as part of the adjacent Class audit and exposed three namespace structural mismatches
  (`stress_class_comments_inside_namespaces_024`,
  `stress_class_nested_namespaces_many_levels_021`, and `stress_class_unicode_namespace_mix_017`).
  Those mismatches were not caused by the Flowchart change and were handled by the dedicated
  source-backed Class namespace/qualified-id slice below.

Twelfth slice outcome:

- Audited pinned Mermaid 11.15 Class DB source:
  - `packages/mermaid/src/diagrams/class/classDb.ts`
- Mermaid 11.15 `addRelation(...)` calls `addClass(...)` for each relation endpoint and then keeps
  `classRelation.id1/id2` as `splitClassNameAndType(endpoint).className`. It does not resolve a
  namespace-qualified relation endpoint such as `Outer.Foo` back to an existing namespace member
  `Foo`.
- Local Class core had an ASCII-oriented shortcut that collapsed namespace-qualified relation
  endpoints to the namespace member id when that member existed. That made ASCII output concise, but
  broke pinned SVG structural parity for relation ids and implicit namespace-qualified facade class
  nodes.
- Class core now preserves Mermaid's namespace-qualified facade semantics for relation endpoints.
  The focused parser test documents the extra facade classes and fully-qualified relation endpoints.
- ASCII rendering keeps its user-friendly output by folding only empty namespace facade classes back
  to their declared namespace member as a view-layer alias. This avoids duplicate terminal boxes
  without changing the core semantic model or SVG parity surface.
- The Class SVG HTML-cap fixture was updated to the current deterministic headless output after the
  surrounding renderer changes; `compare-class-svgs` remains the authority for pinned upstream DOM
  structure.

Touched production surfaces:

- [crates/merman-core/src/diagrams/class/db.rs](/F:/SourceCodes/Rust/merman/crates/merman-core/src/diagrams/class/db.rs)
- [crates/merman-ascii/src/class/render.rs](/F:/SourceCodes/Rust/merman/crates/merman-ascii/src/class/render.rs)
- [crates/merman-core/src/diagrams/class/tests.rs](/F:/SourceCodes/Rust/merman/crates/merman-core/src/diagrams/class/tests.rs)
- [crates/merman-render/tests/class_svg_test.rs](/F:/SourceCodes/Rust/merman/crates/merman-render/tests/class_svg_test.rs)

Focused verification:

- `cargo fmt -p merman-core -p merman-render -p merman-ascii`
- `cargo fmt --check -p merman-core -p merman-render -p merman-ascii`
- `cargo test -p merman-core class --lib`
- `cargo test -p merman-render --test class_svg_test`
- `cargo test -p merman-ascii class --test class_model`
- `cargo run -p xtask -- compare-class-svgs --check-dom --dom-mode parity --dom-decimals 3`

Residual note:

- This slice fixes Class namespace-qualified relation semantic/structural parity only. It does not
  claim Class root-bounds or browser text-measurement closure.

Thirteenth slice outcome:

- Audited pinned Mermaid 11.15 Timeline style provider:
  - `packages/mermaid/src/diagrams/timeline/styles.js`
- Timeline CSS already consumed `cScale*`, `cScaleLabel*`, `cScaleInv*`, `git0`, and
  `gitBranchLabel0`, but still hardcoded `.disabled` fills to `lightgray` and `#efefef`.
- Mermaid 11.15 emits `.disabled` node fill from `themeVariables.tertiaryColor` and disabled text
  fill from `themeVariables.clusterBorder`, with those hardcoded values only as missing-option
  fallbacks. Default pinned SVG baselines therefore contain expanded theme values such as
  `hsl(80, 100%, 96.2745098039%)` and `#aaaa33`.
- Timeline CSS now reads both values through the shared theme lookup path. This fixes a visible
  theme/readability gap without emitting inert redux/neo/gradient rules that local Timeline SVG
  nodes do not currently support.

Touched production surfaces:

- [crates/merman-render/src/svg/parity/timeline.rs](/F:/SourceCodes/Rust/merman/crates/merman-render/src/svg/parity/timeline.rs)
- [crates/merman-render/tests/timeline_svg_test.rs](/F:/SourceCodes/Rust/merman/crates/merman-render/tests/timeline_svg_test.rs)

Focused verification:

- `cargo fmt -p merman-render`
- `cargo fmt --check -p merman-render`
- `cargo test -p merman-render timeline_svg_honors_mermaid_11_15_disabled_theme_colors --test timeline_svg_test`
- `cargo run -p xtask -- compare-timeline-svgs --check-dom --dom-mode parity --dom-decimals 3`
- `git diff --check`

Residual note:

- Timeline redux/neo gradient/drop-shadow selectors remain intentionally un-emitted where local SVG
  does not emit the required `data-look`/defs support. This slice fixes only source-backed CSS that
  applies to the current local Timeline stylesheet.

Fourteenth slice outcome:

- Audited pinned Mermaid 11.15 Architecture style provider and option type:
  - `packages/mermaid/src/diagrams/architecture/architectureStyles.ts`
  - `packages/mermaid/src/diagrams/architecture/architectureTypes.ts`
- Mermaid 11.15 Architecture CSS consumes `archEdgeColor`, `archEdgeArrowColor`,
  `archEdgeWidth`, `archGroupBorderColor`, and `archGroupBorderWidth` directly. The local theme
  expansion already populated these values, but Architecture CSS still emitted generic
  `lineColor`, `primaryBorderColor`, and hardcoded `3` / `2px` widths.
- Architecture CSS now reads those source-backed `arch*` theme variables through the shared config
  CSS-token path. Custom Architecture edge/group styling therefore reaches the final SVG stylesheet
  instead of being parsed and then ignored.

Touched production surfaces:

- [crates/merman-render/src/svg/parity/css.rs](/F:/SourceCodes/Rust/merman/crates/merman-render/src/svg/parity/css.rs)
- [crates/merman-render/tests/architecture_svg_test.rs](/F:/SourceCodes/Rust/merman/crates/merman-render/tests/architecture_svg_test.rs)

Focused verification:

- `cargo fmt -p merman-render`
- `cargo fmt --check -p merman-render`
- `cargo test -p merman-render architecture_css_with_config_honors_font_and_theme_colors`
- `cargo test -p merman-render architecture_svg_honors_mermaid_11_15_style_theme_variables --test architecture_svg_test`
- `cargo test -p merman-render --test architecture_svg_test`
- `cargo run -p xtask -- compare-architecture-svgs --check-dom --dom-mode parity --dom-decimals 3`

Residual note:

- This slice fixed Architecture theme CSS emission only. It did not change Architecture layout,
  Cytoscape/manatee phase modeling, SVG root bounds, or the then-known 26 Architecture
  `parity-root` residuals.

Fifteenth slice outcome:

- Audited pinned Mermaid 11.15 Class note construction:
  - `packages/mermaid/src/diagrams/class/classDb.ts`
  - `packages/mermaid/src/diagrams/class/styles.js`
- Mermaid 11.15 writes `themeVariables.noteBkgColor` and `noteBorderColor` into each Class note
  node's `cssStyles`, while Class stylesheet uses `noteTextColor` for `.noteLabel` text.
- Local Class CSS already consumed `noteTextColor`, but both HTML-label and `htmlLabels:false`
  note shape render paths still hardcoded `#fff5ad` / `#aaaa33`. Custom note backgrounds and
  borders were therefore parsed but ignored in final SVG output.
- Class note rendering now reads `noteBkgColor` and `noteBorderColor` from `effective_config` for
  both note branches, preserving the same inline style shape that Mermaid emits after theme
  expansion.

Touched production surfaces:

- [crates/merman-render/src/svg/parity/class/note.rs](/F:/SourceCodes/Rust/merman/crates/merman-render/src/svg/parity/class/note.rs)
- [crates/merman-render/tests/class_svg_test.rs](/F:/SourceCodes/Rust/merman/crates/merman-render/tests/class_svg_test.rs)

Focused verification:

- `cargo fmt -p merman-render`
- `cargo fmt --check -p merman-render`
- `cargo test -p merman-render class_svg_honors_configured_note_theme_colors --test class_svg_test`
- `cargo test -p merman-render --test class_svg_test`
- `cargo run -p xtask -- compare-class-svgs --check-dom --dom-mode parity --dom-decimals 3`

Residual note:

- This slice fixes Class note theme/readability only. It does not change Class namespace cluster
  inline styling, Class layout, browser text measurement, or root-bounds residuals.

Sixteenth slice outcome:

- Audited pinned Mermaid 11.15 Class style provider:
  - `packages/mermaid/src/diagrams/class/styles.js`
- Class CSS already covered the basic label, relation, marker, and note text color rules, but it
  still missed source-backed rules that apply to current local output: `g.classGroup text`, cluster
  labels/rects, node shape selectors, dividers, `g.classGroup` rect/line, `.classLabel`,
  `.edgeTerminals`, and the source-shaped `.classTitleText` rule.
- The same audit found that Class `strokeWidth` was read through `theme.color(...)`, so numeric
  `themeVariables.strokeWidth` values were silently ignored. Mermaid 11.15 treats `strokeWidth` as
  a CSS token in the Class stylesheet.
- Class CSS now reads `strokeWidth` through `SvgTheme::css_value(...)` and emits the source-backed
  rules above for selectors that hit the current local SVG surface. This makes custom node/relation
  stroke widths and Class theme colors visible without adding browser-dependent measurement logic.
- Upstream icon rules and neo-only rules remain intentionally deferred where local Class rendering
  does not yet emit `.label-icon`, `data-look="neo"`, or the matching neo support shapes.

Touched production surfaces:

- [crates/merman-render/src/svg/parity/class/css.rs](/F:/SourceCodes/Rust/merman/crates/merman-render/src/svg/parity/class/css.rs)
- [crates/merman-render/tests/class_svg_test.rs](/F:/SourceCodes/Rust/merman/crates/merman-render/tests/class_svg_test.rs)

Focused verification:

- `cargo fmt -p merman-render`
- `cargo fmt --check -p merman-render`
- `cargo test -p merman-render class_svg_honors_numeric_stroke_width_theme_css --test class_svg_test`
- `cargo test -p merman-render --test class_svg_test`
- `cargo run -p xtask -- compare-class-svgs --check-dom --dom-mode parity --dom-decimals 3`
- `git diff --check`

Residual note:

- This slice fixes Class stylesheet/theme emission only. It does not claim Class root-bounds,
  namespace cluster inline style, icon label, or neo rendering support closure.

Seventeenth slice outcome:

- Audited Zed integration feedback from
  [zed-industries/zed#57967](https://github.com/zed-industries/zed/pull/57967), which updates Zed
  from `merman = "0.4"` to `0.6` and adopts `SvgPipeline::resvg_safe()`.
- The reviewer-visible color changes in
  [issuecomment-4598335939](https://github.com/zed-industries/zed/pull/57967#issuecomment-4598335939)
  and the follow-up
  [issuecomment-4599604388](https://github.com/zed-industries/zed/pull/57967#issuecomment-4599604388)
  are host theme override concerns, not evidence that merman should inject Zed's palette by
  default. Zed's `color cleanup` commit rewrites its own background, edge-label, and tag-label
  injection behavior to preserve Zed's current visual style.
- The same commit changed Zed's fallback cleanup from "drop all fallback groups when any native
  text exists" to "drop only fallback groups whose text duplicates native SVG text." That is a
  general `resvg_safe` integration need: some diagrams need fallback labels for raster safety, while
  others can double-render labels when native SVG text already exists.
- Added `DropNativeDuplicateFallbacksPostprocessor` as a public optional pipeline pass exported
  through both `merman_render::svg` and `merman::render`. It preserves the default `resvg_safe()`
  contract while allowing hosts to compose:
  `SvgPipeline::resvg_safe().with_postprocessor(DropNativeDuplicateFallbacksPostprocessor)`.
- The pass uses the existing `data-merman-foreignobject="fallback"` and
  `merman-foreignobject-fallback-text` marker contract, collects native non-fallback `<text>`
  contents, and removes only fallback `<g>` groups whose normalized text duplicates a native label.

Touched production surfaces:

- [crates/merman-render/src/svg/pipeline/builtin/foreign_object.rs](/F:/SourceCodes/Rust/merman/crates/merman-render/src/svg/pipeline/builtin/foreign_object.rs)
- [crates/merman-render/src/svg/pipeline/builtin/mod.rs](/F:/SourceCodes/Rust/merman/crates/merman-render/src/svg/pipeline/builtin/mod.rs)
- [crates/merman-render/src/svg/pipeline/mod.rs](/F:/SourceCodes/Rust/merman/crates/merman-render/src/svg/pipeline/mod.rs)
- [crates/merman-render/src/svg.rs](/F:/SourceCodes/Rust/merman/crates/merman-render/src/svg.rs)
- [crates/merman/src/render/mod.rs](/F:/SourceCodes/Rust/merman/crates/merman/src/render/mod.rs)

Focused verification:

- `cargo fmt -p merman-render -p merman`
- `cargo fmt --check -p merman-render -p merman`
- `cargo test -p merman-render drop_native_duplicate_fallbacks --lib`
- `cargo test -p merman-render resvg_safe_can_optionally_drop_native_duplicate_fallbacks --lib`
- `cargo test -p merman-render svg::pipeline --lib`
- `cargo test -p merman-render foreign_object --lib`
- `cargo test -p merman render_svg_sync_applies_scoped_theme_css_once --lib` compiled the top-level
  `merman` crate but matched zero tests.
- `git diff --check`

Known non-slice gate:

- `cargo test -p merman-render --lib` currently fails on existing measurement-sensitive tests:
  `sequence_default_message_widths_match_mermaid_default_font_family` (`161.0` vs `160.0`) and
  `node_katex_math_renderer_measures_sanitized_flowchart_browser_shell` (`matrix width =
  282.265625`). The new pipeline tests pass and these failures are unrelated to fallback
  de-duplication.

Residual note:

- This slice does not change default `resvg_safe()` behavior and does not make Zed's host theme CSS
  a merman default. Host palette injection remains the consumer's responsibility, while fallback
  marker/de-duplication is now available as a reusable pipeline contract.

Eighteenth slice outcome:

- Added
  [docs/workstreams/headless-parity-deepening/THEME_RENDERING_COVERAGE.md](/F:/SourceCodes/Rust/merman/docs/workstreams/headless-parity-deepening/THEME_RENDERING_COVERAGE.md)
  as the durable HPD-080 ledger for Mermaid 11.15 style-provider coverage and host theme boundaries.
- Re-audited the pinned Mermaid `11.15.0` source tree at
  `41646dfd43ac83f001b03c70605feb036afae46d` for diagram style providers, including provider names
  that are not simply `styles.js` / `styles.ts`:
  - `architecture/architectureStyles.ts`
  - `pie/pieStyles.ts`
  - conventional providers such as `class/styles.js`, `flowchart/styles.ts`,
    `sequence/styles.js`, and `treemap/styles.ts`
- Recorded implemented-matrix boundaries:
  - covered source-backed providers for current local output,
  - inline-only diagrams such as QuadrantChart and XYChart,
  - absent-provider diagrams such as Info/Error,
  - unsupported-family providers that belong to the admission rubric rather than HPD-080.
- Recorded negative gates:
  - do not add inert neo/gradient/drop-shadow/data-look CSS when local SVG does not emit the
    required elements, attributes, defs, or filters,
  - do not copy Zed-specific palette cleanup into default merman output,
  - do not globally remove root `background-color: white;` until upstream SVG baselines and
    Mermaid source/capture behavior are reconciled,
  - do not claim exact browser font parity through fixture-specific widths.
- Updated
  [docs/alignment/ZED_MERMAID_ISSUE_AUDIT.md](/F:/SourceCodes/Rust/merman/docs/alignment/ZED_MERMAID_ISSUE_AUDIT.md)
  with PR `zed-industries/zed#57967`, distinguishing host palette policy from the reusable fallback
  duplicate-cleanup pipeline contract.

Focused verification:

- `git -C repo-ref/mermaid ls-tree -r --name-only 41646dfd43ac83f001b03c70605feb036afae46d packages/mermaid/src/diagrams`
- `git -C repo-ref/mermaid grep -n "styles" 41646dfd43ac83f001b03c70605feb036afae46d -- packages/mermaid/src/diagrams/*/*Diagram.ts packages/mermaid/src/diagrams/*/*definition.ts packages/mermaid/src/diagrams/*/diagram.ts`
- `git -C repo-ref/mermaid show 41646dfd43ac83f001b03c70605feb036afae46d:packages/mermaid/src/diagrams/architecture/architectureStyles.ts`
- `git -C repo-ref/mermaid show 41646dfd43ac83f001b03c70605feb036afae46d:packages/mermaid/src/diagrams/pie/pieStyles.ts`

Nineteenth slice outcome:

- Audited Mermaid 11.15 XYChart theme behavior at pinned source commit
  `41646dfd43ac83f001b03c70605feb036afae46d`:
  - `xychartDb.ts` merges default `themeVariables.xyChart` with configured
    `config.themeVariables.xyChart`,
  - `xychartRenderer.ts` writes those values directly to SVG `fill` / `stroke` attributes,
  - `chartBuilder/interfaces.ts` defines `XYChartThemeConfig`,
  - `theme-default.js` and `theme-base.js` define the `xyChart` theme variable surface.
- Added a render-path regression in
  [crates/merman-render/tests/xychart_svg_test.rs](/F:/SourceCodes/Rust/merman/crates/merman-render/tests/xychart_svg_test.rs)
  using
  [fixtures/xychart/upstream_cypress_xychart_spec_render_all_the_theme_color_018.mmd](/F:/SourceCodes/Rust/merman/fixtures/xychart/upstream_cypress_xychart_spec_render_all_the_theme_color_018.mmd).
- The regression asserts that custom `themeVariables.xyChart` values reach the final SVG for:
  chart background, chart title, x/y axis titles, x/y labels, x/y ticks, x/y axis lines, and bar/line
  plot palette colors.
- No production change was needed. This slice confirms the correct headless parity boundary for
  XYChart: inline theme config is supported, and no CSS provider should be invented.

Focused verification:

- `cargo fmt -p merman-render`
- `cargo fmt --check -p merman-render`
- `cargo test -p merman-render xychart_svg_honors_mermaid_11_15_inline_theme_config --test xychart_svg_test`
- `cargo test -p merman-render --test xychart_svg_test`
- `cargo run -p xtask -- compare-xychart-svgs --check-dom --dom-mode parity --dom-decimals 3`
- `git diff --check`

Twentieth slice outcome:

- Added a public API dark-theme renderability smoke in
  [crates/merman/tests/theme_renderability_smoke.rs](/F:/SourceCodes/Rust/merman/crates/merman/tests/theme_renderability_smoke.rs)
  for Flowchart, Sequence, Kanban, GitGraph, and XYChart. The smoke checks readable labels,
  absence of broken geometry, and source-backed theme colors in final SVG output through
  `HeadlessRenderer`.
- The smoke found a real Flowchart theme omission. Mermaid 11.15
  `packages/mermaid/src/diagrams/flowchart/styles.ts` uses `nodeTextColor || textColor` for
  labels, while local Flowchart CSS only used `textColor`.
- Flowchart CSS now reads `themeVariables.nodeTextColor` and applies it to `.label` and
  `.label text, span`; `themeVariables.textColor` continues to drive root text fill.
- Added a focused renderer regression in
  [crates/merman-render/tests/flowchart_svg_test.rs](/F:/SourceCodes/Rust/merman/crates/merman-render/tests/flowchart_svg_test.rs).
- The same smoke clarified a test boundary for Kanban: pinned Mermaid 11.15 source/fixtures emit
  `class="cluster undefined ..."` and `class="node undefined"` placeholder classes, and priority
  metadata is rendered as a side-line rather than priority text. The smoke therefore permits those
  upstream placeholder class tokens but still rejects real `NaN` geometry and any remaining
  `undefined` leakage outside that placeholder shape.

Focused verification:

- `cargo fmt -p merman-render -p merman`
- `cargo test -p merman-render flowchart_svg_honors_node_text_color_theme_variable --test flowchart_svg_test`
- `cargo test -p merman-render --test flowchart_svg_test`
- `cargo test -p merman representative_dark_theme_diagrams_keep_visible_theme_signals --test theme_renderability_smoke --features render`
- `cargo fmt --check -p merman-render -p merman`
- `cargo run -p xtask -- compare-flowchart-svgs --check-dom --dom-mode parity --dom-decimals 3`
- `cargo run -p xtask -- compare-kanban-svgs --check-dom --dom-mode parity --dom-decimals 3`

Twenty-first slice outcome:

- Audited QuadrantChart inline theme behavior against pinned Mermaid 11.15:
  - `packages/mermaid/src/themes/theme-default.js`
  - `packages/mermaid/src/diagrams/quadrant-chart/quadrantRenderer.ts`
  - khroma `lighten`, `darken`, and `luminance` helpers from the installed baseline toolchain.
- Mermaid 11.15 intends default `quadrantPointFill` to be a lightened/darkened
  `quadrant1Fill`, but the shipped source calls khroma `lighten` / `darken` without the required
  amount argument. The saved upstream fixtures therefore contain invalid
  `hsl(240, 100%, NaN%)` point fill/stroke tokens.
- Local QuadrantChart output now treats that as an upstream invalid-token defect rather than a
  product requirement. When no valid `themeVariables.quadrantPointFill` is present, merman derives
  a valid 10% lightness-shift point color from `quadrant1Fill`; valid explicit
  `quadrantPointFill` values still win verbatim.
- Added renderer coverage in
  [crates/merman-render/tests/quadrantchart_svg_test.rs](/F:/SourceCodes/Rust/merman/crates/merman-render/tests/quadrantchart_svg_test.rs)
  for default point renderability and explicit point color/text overrides.
- Extended the public API dark-theme smoke to cover QuadrantChart inline theme variables.
- Updated xtask DOM parity normalization so only QuadrantChart default data-point circle
  `fill`/`stroke` maps upstream `hsl(...NaN%)` and local `rgb(185, 185, 255)` to one comparison
  slot. Strict mode still preserves the real attr difference.

Focused verification:

- `cargo fmt -p merman-render -p merman -p xtask`
- `cargo test -p merman-render --test quadrantchart_svg_test`
- `cargo test -p merman representative_dark_theme_diagrams_keep_visible_theme_signals --test theme_renderability_smoke --features render`
- `cargo test -p xtask parity_normalizes_quadrantchart_invalid_default_point_color`
- `cargo fmt --check -p merman-render -p merman -p xtask`
- `cargo run -p xtask -- compare-quadrantchart-svgs --check-dom --dom-mode parity --dom-decimals 3`
- `cargo run -p xtask -- compare-quadrantchart-svgs --check-dom --dom-mode parity-root --dom-decimals 3`

Residual note:

- This is an intentional renderability-over-byte-parity correction for an invalid upstream CSS
  token. It should not be generalized into broad color normalization or cosmetic palette changes.

Twenty-second slice outcome:

- Audited the remaining raw SVG `undefined` hits from the renderability scan:
  - `fixtures/er/basic.mmd`
  - `fixtures/mindmap/basic.mmd`
  - corresponding pinned upstream SVG fixtures.
- ER relationship paths and Mindmap edge paths both emitted
  `style="undefined;;;undefined"`. Pinned upstream fixtures contain the same artifact, but the
  attribute carries no useful visual semantics; edge appearance is driven by CSS classes such as
  `relationshipLine`, `edge`, `section-edge-*`, and `edge-depth-*`.
- Removed the invalid inline `style` attribute from local ER relationship paths and Mindmap edge
  paths instead of preserving a useless upstream token for byte parity.
- Added focused regression checks so the affected raw SVG paths no longer leak `style="undefined"`.

Focused verification:

- `cargo fmt -p merman-render -p merman`
- `cargo test -p merman-render er_svg_renders_entities_and_relationships --test er_svg_test`
- `cargo test -p merman mindmap_br_variants_031_matches_upstream_node_geometry --test mindmap_br_variants_031 --features render`
- `cargo run -p xtask -- compare-er-svgs --check-dom --dom-mode parity --dom-decimals 3`
- `cargo run -p xtask -- compare-mindmap-svgs --check-dom --dom-mode parity --dom-decimals 3`
- `cargo fmt --check -p merman-render -p merman`

Manual raw-output sample:

- `fixtures/er/basic.mmd`: no `style="undefined"`, no `undefined`, no `NaN`
- `fixtures/mindmap/basic.mmd`: no `style="undefined"`, no `undefined`, no `NaN`

Residual note:

- This is a narrow raw-SVG cleanliness fix. It does not imply that all empty `style=""` attributes
  should be removed, and it does not change ER or Mindmap layout/root residuals.

Twenty-third slice outcome:

- Rechecked Mermaid 11.15 theme registry evidence after the Zed host-theme audit:
  - `repo-ref/mermaid/packages/mermaid/src/themes/index.js`
  - `repo-ref/mermaid/packages/mermaid/src/config.type.ts`
  - `tools/mermaid-cli/node_modules/mermaid/dist/config.type.d.ts`
- Corrected the previous `neo/redux*` interpretation. These names are official Mermaid 11.15
  config themes, not snapshot-only local artifacts:
  - `neo`
  - `neo-dark`
  - `redux`
  - `redux-dark`
  - `redux-color`
  - `redux-dark-color`
- Core, bindings, and `@merman/web` now expose all 11 official Mermaid 11.15 theme names:
  `default`, `base`, `dark`, `forest`, `neutral`, `neo`, `neo-dark`, `redux`, `redux-dark`,
  `redux-color`, and `redux-dark-color`.
- Extended theme defaults expand from the generated Mermaid 11.15 theme-variable snapshot. Explicit
  direct `themeVariables` overrides still win, and unknown theme names still fall back to the
  default theme.
- This does not claim exact browser/source-equivalent override derivation for every `neo/redux*`
  variable. That remains a narrower follow-up audit instead of a fake parity claim.
- The Zed PR color cleanup remains classified as host palette policy. The common merman integration
  need is still fallback marking and optional duplicate fallback cleanup, not default editor-color
  rewriting.

Focused verification:

- `cargo fmt -p merman-core -p merman-bindings-core -p merman`
- `cargo test -p merman-core theme`
- `cargo test -p merman-bindings-core supported_themes_exposes_core_theme_surface`
- `cargo test -p merman external_ --features render`
- `npm run build:ts --prefix platforms/web`

Residual note:

- The Rust API can already support common host theme workflows through composed postprocessors.
  Non-Rust/JSON option consumers still need a first-class way to opt into duplicate fallback
  cleanup and possibly scoped host CSS policy; do not silently change `resvg_safe()` defaults for
  that.

Twenty-fourth slice outcome:

- Closed the generic part of the non-Rust host integration gap from the Zed/theme audit by adding
  `svg.drop_native_duplicate_fallbacks` to the shared binding `options_json` surface.
- The option defaults to `false`, so existing `parity`, `readable`, and `resvg-safe` pipeline
  contracts stay unchanged.
- When enabled, binding renderers build the selected `SvgPipeline` and append
  `DropNativeDuplicateFallbacksPostprocessor`, removing only fallback groups whose text duplicates
  native SVG `<text>` while preserving fallback-only labels.
- Updated `@merman/web` TypeScript options and `docs/bindings/OPTIONS_JSON.md` so host consumers do
  not need private JSON strings or downstream duplicate-fallback hacks.

Focused verification:

- `cargo fmt -p merman-bindings-core`
- `cargo test -p merman-bindings-core svg_options_can_drop_native_duplicate_fallbacks`
- `cargo test -p merman-bindings-core render_svg_accepts_options_json`
- `npm run build:ts --prefix platforms/web`

Residual note:

- Host palette replacement, root background replacement, scoped CSS injection, and
  `!important` cleanup are still not JSON options. Those require an explicit cascade/security
  design rather than a quick binding flag.

Twenty-fifth slice outcome:

- Re-audited Zed PR 57967 after the binding fallback option landed, including the `color cleanup`
  commit. The remaining Zed color differences are still host palette policy: white-background
  replacement, edge-label background/text fixes, and GitGraph tag-label text color are editor
  compatibility rules, not default Mermaid output requirements.
- Confirmed the current merman surface covers common Rust host theme needs:
  `HeadlessRenderer::with_site_config(...)`, Mermaid `theme` / `themeVariables` / `themeCSS`,
  `SvgPipeline::resvg_safe()`, `ScopedCssPostprocessor`, `CssOverridePostprocessor`, and
  `DropNativeDuplicateFallbacksPostprocessor`.
- Confirmed the shared binding surface covers raster-safe output and duplicate fallback cleanup, but
  does not yet expose external Mermaid site config or host-scoped CSS injection. That is an API
  ergonomics gap, not evidence for silently changing `resvg_safe()` defaults.
- Updated `THEME_RENDERING_COVERAGE.md` with a common-host-needs table so future HPD-080 work does
  not conflate Mermaid theme support with product-specific palette rewriting.

Focused verification:

- `gh pr view 57967 --repo zed-industries/zed --comments --json title,url,mergeStateStatus,state,body,files,commits,comments,reviews`
- `gh api repos/zed-industries/zed/commits/c85f29cd2e78ec8a68b20349606d8298eecf37bb --jq ...`
- `cargo nextest run -p merman-core theme`
- `cargo nextest run -p merman-bindings-core supported_themes_exposes_core_theme_surface svg_options_can_drop_native_duplicate_fallbacks`
- `cargo nextest run -p merman --features render external_ render_svg_sync_applies_scoped_theme_css_once`
- `npm run build:ts --prefix platforms/web`

Residual note:

- The next useful binding split is Mermaid `site_config` first and host-scoped CSS later. Host CSS
  still needs an explicit security/cascade/raster-safety contract; do not add a quick Zed-specific
  palette flag.

Twenty-sixth slice outcome:

- Closed the Mermaid-config part of the binding host-theme gap by adding top-level
  `options_json.site_config` to [crates/merman-bindings-core/src/lib.rs](/F:/SourceCodes/Rust/merman/crates/merman-bindings-core/src/lib.rs).
- `site_config` is validated as a JSON object and mapped to `HeadlessRenderer::with_site_config(...)`
  / `HeadlessAsciiRenderer::with_site_config(...)`, so binding consumers can pass `theme`,
  `themeVariables`, diagram options, and Mermaid `themeCSS` without embedding init directives in
  the source text.
- Updated `@merman/web` `BindingOptions` and `docs/bindings/OPTIONS_JSON.md` with the same
  top-level `site_config` contract.
- Kept host palette CSS as a boundary. This slice does not add a Zed-specific background or label
  color option and does not change `resvg_safe()` defaults.
- Updated `THEME_RENDERING_COVERAGE.md` so common host theme needs now distinguish supported
  Mermaid site config from still-manual host-owned palette postprocessing.

Focused verification:

- `$env:RUSTFLAGS='-C linker=rust-lld'; cargo test -p merman-bindings-core site_config --lib`
- `cargo fmt --check -p merman-bindings-core`
- `$env:RUSTFLAGS='-C linker=rust-lld'; cargo test -p merman-bindings-core --lib`
- `npm run build:ts --prefix platforms/web`
- JSONL validation for `CONTEXT.jsonl`, `TASKS.jsonl`, and `CAMPAIGNS.jsonl`
- `git diff --check`

Residual note:

- Binding host-owned scoped CSS remains the next possible API split. Plain Mermaid `themeCSS` is
  already covered through `site_config`.
- The default MSVC linker was not on PATH in this shell (`link.exe` not found), so Rust tests were
  verified with the toolchain-provided `rust-lld` linker.

Twenty-seventh slice outcome:

- Closed the binding host-owned CSS half of the Zed/theme integration gap by adding `svg.scoped_css`
  and `svg.css_override_policy` to [crates/merman-bindings-core/src/lib.rs](/F:/SourceCodes/Rust/merman/crates/merman-bindings-core/src/lib.rs).
- The binding API maps `svg.scoped_css` to `ScopedCssPostprocessor`, scopes selectors to the root
  SVG id, and injects host CSS after Mermaid CSS for normal cascade order.
- `svg.css_override_policy` accepts `preserve`, `strip-existing-important`, and
  `strip_existing_important`. Invalid values return `MERMAN_INVALID_ARGUMENT`.
- For `svg.pipeline="resvg-safe"`, the binding pipeline now runs `SanitizeCssPostprocessor` after
  host CSS injection, preserving the raster-safe preset's CSS-sanitization contract for injected
  host CSS.
- Updated `@merman/web`, `docs/bindings/OPTIONS_JSON.md`,
  `docs/rendering/SVG_OUTPUT_PIPELINE.md`, and `THEME_RENDERING_COVERAGE.md`.
- This still does not add Zed-specific palette defaults, root background stripping, or any change
  to the default Mermaid parity SVG output.

Focused verification:

- `$env:RUSTFLAGS='-C linker=rust-lld'; cargo test -p merman-bindings-core scoped_css --lib`
- `cargo fmt -p merman-bindings-core`
- `$env:RUSTFLAGS='-C linker=rust-lld'; cargo test -p merman-bindings-core --lib`
- `npm run build:ts --prefix platforms/web`
- JSONL validation for `CONTEXT.jsonl`, `TASKS.jsonl`, and `CAMPAIGNS.jsonl`
- `git diff --check`

Residual note:

- Binding host CSS is now an explicit host-owned option. The remaining root white-background
  question is still a source/capture audit before any default output policy changes.

Twenty-eighth slice outcome:

- Reconciled the root white-background question against pinned Mermaid 11.15 source, installed
  Mermaid 11.15 dist, and local capture code.
- Source-backed finding:
  - pinned `packages/mermaid/src/setupGraphViewbox.js` sets root `width="100%"` and
    `style="max-width: ...px;"` when `useMaxWidth` is enabled,
  - installed Mermaid 11.15 dist has the same `calculateSvgSizeAttrs(...)` behavior,
  - `xtask` upstream capture injects `background-color: white` by default through
    `ensureSvgBackgroundColor(...)`,
  - local parity renderers preserve that capture-compatible white root background across many
    implemented diagram families.
- Added `RootBackgroundPostprocessor` as the product-neutral host seam for this common editor/raster
  integration need. It rewrites only the root `<svg>` inline `background-color` or adds one when
  missing; it does not rewrite Mermaid-owned node, edge, label, or semantic palette colors.
- Added shared binding option `svg.root_background_color`, plus `@merman/web` typing and binding
  validation. This lets non-Rust hosts set the root canvas color without depending on CSS cascade
  over an inline style.
- Updated `THEME_RENDERING_COVERAGE.md`, `docs/rendering/SVG_OUTPUT_PIPELINE.md`, and
  `docs/bindings/OPTIONS_JSON.md` to classify root canvas replacement as supported opt-in host
  policy, not a default Mermaid parity output change.

Focused verification:

- `git -C repo-ref/mermaid show 41646dfd43ac83f001b03c70605feb036afae46d:packages/mermaid/src/setupGraphViewbox.js`
- `Get-Content tools/mermaid-cli/node_modules/mermaid/dist/chunks/mermaid.core/chunk-CSCIHK7Q.mjs`
  around `calculateSvgSizeAttrs(...)`
- `rg "ensureSvgBackgroundColor|background_color" crates/xtask/src/cmd/generate.rs`
- `$env:RUSTFLAGS='-C linker=rust-lld'; cargo test -p merman-render root_background --lib`
- `$env:RUSTFLAGS='-C linker=rust-lld'; cargo test -p merman-bindings-core root_background --lib`
- `npm run build:ts --prefix platforms/web`
- JSONL validation for `CONTEXT.jsonl`, `TASKS.jsonl`, and `CAMPAIGNS.jsonl`
- `git diff --check`

Residual note:

- Do not globally strip or recolor default root backgrounds. They remain part of the current
  fixture/capture comparison surface. Host canvas color is now explicit opt-in policy through the
  Rust pipeline or binding options.

Twenty-ninth slice outcome:

- Re-audited Zed PR 57967 and the current 0.7 host-theme surface for common consumer needs after
  the `site_config`, scoped CSS, duplicate fallback, and root-background binding options landed.
- The current product-neutral support is sufficient for common host theme flows:
  - official Mermaid theme selection and `themeVariables` through Rust `with_site_config(...)` or
    binding `options_json.site_config`,
  - Mermaid diagram-owned `themeCSS`,
  - host-owned scoped CSS with optional `!important` stripping,
  - `resvg-safe` fallback insertion / `foreignObject` stripping / CSS and attribute cleanup,
  - optional duplicate native/fallback text cleanup,
  - optional root canvas color replacement.
- Kept Zed-style exact editor palette cleanup as a host boundary. Rust hosts can write custom
  `SvgPostprocessor` passes for arbitrary element/inline-style rewrites; shared bindings expose
  only product-neutral controls and intentionally do not provide a generic XML rewrite DSL.
- Tightened docs so `docs/bindings/OPTIONS_JSON.md` shows
  `svg.drop_native_duplicate_fallbacks` in the full JSON shape, and
  `THEME_RENDERING_COVERAGE.md` now calls out the optional exact-text nature of fallback
  de-duplication instead of implying semantic/geometric equivalence.

Focused verification:

- `gh pr view 57967 --repo zed-industries/zed --comments --json title,url,state,mergeStateStatus,body,files,commits,comments,reviews`
- `gh api repos/zed-industries/zed/commits/c85f29cd2e78ec8a68b20349606d8298eecf37bb --jq '.files[] | {filename,patch}'`
- `$env:RUSTFLAGS='-C linker=rust-lld'; cargo nextest run -p merman-bindings-core`
- `$env:RUSTFLAGS='-C linker=rust-lld'; cargo nextest run -p merman-render drop_native_duplicate_fallbacks`
- `$env:RUSTFLAGS='-C linker=rust-lld'; cargo nextest run -p merman-render root_background`
- `$env:RUSTFLAGS='-C linker=rust-lld'; cargo nextest run -p merman --features render external_`

Residual note:

- Exact `neo/redux*` override derivation remains an honest follow-up only if a fixture or consumer
  proves direct `themeVariables` plus generated snapshots are insufficient.
- The duplicate fallback cleanup is exact-text based and optional. It is useful for Zed-like
  raster duplicate labels, but hosts with intentionally repeated labels may still prefer a custom
  geometry-aware cleanup pass.

Thirtieth slice outcome:

- Expanded the public API dark-theme renderability smoke in
  [crates/merman/tests/theme_renderability_smoke.rs](/F:/SourceCodes/Rust/merman/crates/merman/tests/theme_renderability_smoke.rs)
  from the earlier representative set to cover Class, State, Architecture, Block, Journey, Radar,
  Requirement, Timeline, Gantt, Treemap, and Pie as well.
- The smoke remains semantic rather than pixel-based: it verifies SVG output, rejects `NaN`,
  rejects unexpected `undefined` tokens, and checks source-backed visible labels plus theme colors
  that the current local renderer should emit.
- Requirement labels in the smoke use the renderer's visible source-backed text shape
  (`Risk: High`, `Verification: Analysis`) rather than raw parser token spelling.
- Timeline's `class="node-bkg node-undefined"` is narrowly allowed after checking Mermaid 11.15
  upstream SVG fixtures. This is the same class of upstream placeholder as the earlier Kanban
  `cluster undefined` / `node undefined` class shape, not a local visible rendering failure.

Focused verification:

- `rg -n "node-undefined|undefined" fixtures\upstream-svgs\timeline repo-ref\mermaid\packages\mermaid\src\diagrams\timeline -S`
- `$env:RUSTFLAGS='-C linker=rust-lld'; cargo nextest run -p merman --features render representative_dark_theme_diagrams_keep_visible_theme_signals`
- `$env:RUSTFLAGS='-C linker=rust-lld'; cargo nextest run -p merman --features render --test theme_renderability_smoke`
- `cargo fmt --check -p merman`
- JSONL validation for `CONTEXT.jsonl`, `TASKS.jsonl`, and `CAMPAIGNS.jsonl`
- `git diff --check`

Residual note:

- This is a public renderability contract gate, not a full visual parity metric. It should catch
  blank output, unreadable labels, missing emitted theme colors, and invalid visible tokens without
  pretending to measure browser font or pixel parity exactly.

Thirty-first slice outcome:

- Extended the same public API dark-theme renderability smoke to the remaining supported diagram
  families with compact source-backed theme/config signals: ER, Mindmap, C4, Packet, and Sankey.
- This closes the earlier public-smoke gap where those diagrams had renderer-level CSS/config
  coverage but no public `HeadlessRenderer` route proving labels and configured colors survived the
  full parse/layout/render path.
- No production rendering change was needed. The only calibration was Mindmap: `nodeBorder` is a
  root span color only for redux-style output, so the smoke uses `theme: "redux"` before asserting
  that color.
- C4 is intentionally checked through visible C4 config colors rather than broad Mermaid
  `themeVariables`, because Mermaid 11.15's C4 style provider is narrow and most visible C4 palette
  behavior is C4 config or per-element style rather than generic theme-variable CSS.

Focused verification:

- `$env:RUSTFLAGS='-C linker=rust-lld'; cargo nextest run -p merman --features render representative_dark_theme_diagrams_keep_visible_theme_signals`
- `$env:RUSTFLAGS='-C linker=rust-lld'; cargo nextest run -p merman --features render --test theme_renderability_smoke`
- `cargo fmt --check -p merman`
- JSONL validation for `CONTEXT.jsonl`, `TASKS.jsonl`, and `CAMPAIGNS.jsonl`
- `git diff --check`

Residual note:

- Info and Error still have no Mermaid 11.15 diagram-specific style provider. ZenUML remains an
  external plugin compatibility boundary. They should be audited only for concrete visible failures,
  not forced into the same theme smoke.

Thirty-second slice Mermaid source-checkout audit:

- Re-audited HPD-080 style-provider discovery against the workstream's source authority after a
  suspicious `railroad` / `cynefin` provider hit. The local `repo-ref/mermaid` checkout had drifted
  to `develop` at `9bae92cd3214f9ec99369ab314ef41ffb283f6b6`, while
  `tools/upstreams/REPOS.lock.json` pins Mermaid `11.15.0` to
  `41646dfd43ac83f001b03c70605feb036afae46d`.
- Verified the pinned commit directly with `git -C repo-ref/mermaid ls-tree ...`: `railroad` and
  `cynefin` are absent from the locked `packages/mermaid/src/diagrams` tree, while the unsupported
  family set `treeView`, `ishikawa`, `eventmodeling`, `venn`, and `wardley` remains present.
- Restored `repo-ref/mermaid` to detached HEAD at the lockfile commit. This was a reference-state
  repair, not a renderer code change.
- Re-ran style-provider discovery after the restore. The supported-family coverage in
  `THEME_RENDERING_COVERAGE.md` remains consistent with the pinned Mermaid 11.15 source; no new
  HPD-080 renderer defect was found in this scan.

Focused verification:

- `git -C repo-ref/mermaid rev-parse HEAD`
  returned `41646dfd43ac83f001b03c70605feb036afae46d` after the restore.
- `git -C repo-ref/mermaid ls-tree -d --name-only 41646dfd43ac83f001b03c70605feb036afae46d:packages/mermaid/src/diagrams`
  listed no `railroad` or `cynefin` directory.
- `rg --files repo-ref/mermaid/packages/mermaid/src/diagrams | rg "(style|styles|architectureStyles|ishikawaStyles|pieStyles)\.(ts|js)$"`
  matched the expected pinned-provider inventory.

Thirty-third slice outcome:

- Re-audited Zed PR `57967` and confirmed the current 0.7 theme surface supports common
  product-neutral host needs: Mermaid `theme` / `themeVariables` / `themeCSS`,
  binding `options_json.site_config`, host `svg.scoped_css` / `css_override_policy`,
  `resvg-safe`, duplicate native/fallback cleanup, and root background replacement.
- Kept Zed's exact background, edge-label, tag-label, and accent cleanup classified as host palette
  policy. No default Mermaid theme behavior changed for this part.
- Followed the latest Zed PR thread to PR `58325`, where Zed fixed a stack overflow in deeply
  nested Flowchart subgraphs on their merman fork. Local 0.7 still had the same class of unbounded
  recursive traversal in Flowchart cluster handling.
- Converted Flowchart cluster traversal seams in
  [crates/merman-render/src/flowchart/layout.rs](/F:/SourceCodes/Rust/merman/crates/merman-render/src/flowchart/layout.rs)
  from Rust call-stack recursion to explicit stacks:
  `compute_effective_dir_by_id`, `extract_descendants`, `flowchart_find_non_cluster_child`, and
  `copy_cluster`.
- Added a 512 KB stack-thread regression covering 10,000 nested subgraphs across those traversal
  seams. The bounded recursive cluster layout depth rule remains unchanged.

Focused verification:

- `gh pr view 57967 --repo zed-industries/zed --json title,url,commits,comments,reviews,files`
- `gh issue view 58325 --repo zed-industries/zed --json title,url,state,body,comments,labels`
- `gh api repos/zed-industries/merman/commits/1c765dcca2ef5092fcde7bebe8374819563623ef --jq '.files[] | {filename, patch}'`
- `npm run build:ts --prefix platforms/web`
- `$env:RUSTFLAGS='-C linker=rust-lld'; cargo nextest run -p merman-bindings-core`
- `$env:RUSTFLAGS='-C linker=rust-lld'; cargo nextest run -p merman-render drop_native_duplicate_fallbacks root_background scoped_css`
- `$env:RUSTFLAGS='-C linker=rust-lld'; cargo nextest run -p merman-render flowchart_cluster_traversals_handle_deep_subgraphs_with_small_stack`
- `cargo fmt --check -p merman-render`
- `$env:RUSTFLAGS='-C linker=rust-lld'; cargo nextest run -p merman-render --test flowchart_layout_test`
- `$env:RUSTFLAGS='-C linker=rust-lld'; cargo nextest run -p merman-render --test flowchart_svg_test`
- `$env:RUSTFLAGS='-C linker=rust-lld'; cargo nextest run -p merman --features render --test theme_renderability_smoke`
- `$env:RUSTFLAGS='-C linker=rust-lld'; cargo nextest run -p merman --features render --test zed_pr_57644_corpus`

Thirty-fourth slice outcome:

- Added a public API resvg-safe host-integration smoke in
  [crates/merman/tests/resvg_safe_fixture_smoke.rs](/F:/SourceCodes/Rust/merman/crates/merman/tests/resvg_safe_fixture_smoke.rs).
- The smoke covers the user-provided Kanban metadata and GitGraph merge samples, plus a compact
  dark-theme Flowchart case that proves visible theme colors survive
  `HeadlessRenderer::render_svg_resvg_safe_sync(...)`.
- It also samples supported fixture families deterministically: each available `basic.mmd`, any
  `zed_pr_57644_*.mmd` fixture, and a small sorted set of representative stress/upstream fixtures
  per family.
- The gate rejects host-visible/raster hazards: malformed XML, remaining `<foreignObject>`,
  unsupported CSS constructs, invalid visual values such as `NaN`, `Infinity`, and
  `fill="undefined"`, and empty style elements.
- When executed with the `raster` feature, the same gate converts each resvg-safe SVG to PNG bytes.
  This catches usvg/resvg-level failures that a string-only SVG check would miss.
- No production renderer defect was found in this slice. Treat this as a functional regression
  safety net, not a precise all-fixture or pixel parity metric.

Focused verification:

- `cargo fmt --check -p merman`
- `$env:RUSTFLAGS='-C linker=rust-lld'; cargo nextest run -p merman --features render --test resvg_safe_fixture_smoke`
- `$env:RUSTFLAGS='-C linker=rust-lld'; cargo nextest run -p merman --features raster --test resvg_safe_fixture_smoke`

## HPD-080 - All-Supported Resvg-Safe Audit And Treemap ClassDef

Outcome:

- Resolved the Flowchart `layout.rs` conflict between the Zed PR `58325` backport shape and the
  local explicit-stack cluster traversal follow-up. The merged file keeps the
  `MAX_DIAGRAM_NESTING_DEPTH` model guard and the 512 KB stack-thread regressions for deep helper
  traversals.
- Rechecked Zed PR `57967`: Zed's background/text/accent cleanup is still host policy, while the
  current merman surface covers common product-neutral host needs through `site_config`,
  `themeCSS`, scoped host CSS, optional `!important` cleanup, root-background replacement, and
  duplicate native/fallback cleanup.
- Extended the resvg-safe smoke with a manual ignored all-supported audit. The audit intentionally
  skips parser-only fixtures, upstream docs placeholders such as `...`, and pinned upstream-invalid
  examples, then renders the supported fixture set through
  `HeadlessRenderer::render_svg_resvg_safe_sync(...)`.
- Fixed an upstream-source-backed Treemap input compatibility gap found by that audit. Mermaid
  `TreeMapDB.addClass(...)` accepts bare label-style tokens such as `color`; local Treemap parsing
  no longer rejects those tokens as parse errors.
- Kept headless output valid rather than copying invalid CSS. Treemap SVG style compilation now
  drops empty-valued declarations, so a Mermaid-compatible bare token like `color` does not leak
  `color: !important` or `fill: !important` into final SVG output.
- Fixed section-less Pie root output for headless/raster safety. Mermaid's browser/capture path can
  serialize `viewBox="0 0 -Infinity 450"` for empty Pie input; local headless output now emits the
  finite `viewBox="0 0 450 450"` instead of preserving a raster-hostile invalid token.

Focused verification:

- `cargo fmt --check -p merman-core -p merman-render -p merman`
- `$env:RUSTFLAGS='-C linker=rust-lld'; cargo nextest run -p merman-render flowchart_cluster_traversals_handle_deep_subgraphs_with_small_stack extract_descendants_handles_deeply_nested_subgraphs`
- `$env:RUSTFLAGS='-C linker=rust-lld'; cargo nextest run -p merman --features render --test theme_renderability_smoke`
- `$env:RUSTFLAGS='-C linker=rust-lld'; cargo nextest run -p merman-render drop_native_duplicate_fallbacks root_background scoped_css`
- `$env:RUSTFLAGS='-C linker=rust-lld'; cargo nextest run -p merman-bindings-core supported_themes_exposes_core_theme_surface svg_options_can_drop_native_duplicate_fallbacks svg_options_can_inject_host_scoped_css svg_options_can_set_root_background_color`
- `$env:RUSTFLAGS='-C linker=rust-lld'; cargo nextest run -p merman-render pie --test pie_svg_test`
- `$env:RUSTFLAGS='-C linker=rust-lld'; cargo nextest run -p merman-core treemap_classdef_allows_bare_label_style_tokens_like_mermaid`
- `$env:RUSTFLAGS='-C linker=rust-lld'; cargo nextest run -p merman-render treemap --test treemap_svg_test`
- `$env:RUSTFLAGS='-C linker=rust-lld'; cargo nextest run -p merman --features render --test resvg_safe_fixture_smoke`
- `$env:RUSTFLAGS='-C linker=rust-lld'; cargo nextest run -p merman --features render --test resvg_safe_fixture_smoke --run-ignored ignored-only all_supported_fixtures_render_headless_resvg_safe_audit`
- `$env:RUSTFLAGS='-C linker=rust-lld'; cargo nextest run -p merman --features raster --test resvg_safe_fixture_smoke`
- `git diff --check`

## HPD-080 - Extended Theme Override Derivations

Outcome:

- Re-audited official Mermaid 11.15 `neo/redux*` theme behavior after the host-theme boundary work
  left exact extended-theme override derivation as an honest follow-up.
- Confirmed that no-override defaults should continue to come from the generated
  `theme_variables_11_15_0.json` snapshots, but user override handling cannot be a static snapshot
  merge. Mermaid theme modules run `calculate(overrides)`: copy user base keys, call
  `updateColors()`, then re-apply explicit user keys so direct derived-key overrides win.
- Added a bounded source-backed derivation seam in
  [crates/merman-core/src/theme.rs](/F:/SourceCodes/Rust/merman/crates/merman-core/src/theme.rs).
  It recomputes visible current-renderer derived keys for extended themes when users override
  source base keys such as `primaryColor`, `secondaryColor`, `background`, `lineColor`, and
  `mainBkg`.
- Kept the seam intentionally narrower than a full hand-port of every extended theme line. It
  covers keys consumed by current SVG renderers: Flowchart edge-label/icon surfaces, shared line and
  arrow colors, Architecture edge colors, Requirement relation colors, Sequence actor/label-box
  backgrounds, C4 person backgrounds, State backgrounds/transitions, and GitGraph tag-label
  background.
- Preserved explicit override precedence. If the user directly sets a derived key such as
  `nodeBkg` or `edgeLabelBackground`, local theme expansion leaves that value in place after
  derivation.
- Added a Flowchart SVG regression proving `theme: "redux"` plus
  `themeVariables.primaryColor = "#123456"` derives the Mermaid source secondary color into
  visible edge-label CSS, while correctly keeping Redux node fill on the `mainBkg` default
  `#ffffff`.

Source evidence:

- `repo-ref/mermaid/packages/mermaid/src/themes/index.js`
- `repo-ref/mermaid/packages/mermaid/src/themes/theme-neo.js`
- `repo-ref/mermaid/packages/mermaid/src/themes/theme-redux.js`
- `repo-ref/mermaid/packages/mermaid/src/themes/theme-redux-dark.js`
- `repo-ref/mermaid/packages/mermaid/src/themes/theme-redux-color.js`
- `repo-ref/mermaid/packages/mermaid/src/themes/theme-redux-dark-color.js`
- Installed Mermaid `11.15.0` dist probes through
  `tools/mermaid-cli/node_modules/mermaid/dist/mermaid.core.mjs`.

Focused verification:

- `cargo fmt --check -p merman-core -p merman-render`
- `$env:RUSTFLAGS='-C linker=rust-lld'; cargo nextest run -p merman-core theme`
- `$env:RUSTFLAGS='-C linker=rust-lld'; cargo nextest run -p merman-render --test flowchart_svg_test`
- `$env:RUSTFLAGS='-C linker=rust-lld'; cargo nextest run -p merman --features render --test theme_renderability_smoke`

Residual note:

- Future `neo/redux*` work should extend the source-backed derivation seam only when a fixture or
  consumer proves a currently emitted surface still misses Mermaid's override-derived value. Do not
  replace the generated default snapshots with fixture-keyed constants.

## HPD-080 - Extended Theme Dark Palette Derivations

Outcome:

- Continued the official `neo/redux*` override audit against Mermaid 11.15 dark extended themes.
- Confirmed from pinned source and installed Mermaid output that `neo-dark` / `redux-dark` /
  `redux-dark-color` with `themeVariables.primaryColor` derive visible
  `requirementBackground`, `pie1`, and `quadrant1Fill`.
- Confirmed `redux-dark` / `redux-dark-color` also derive GitGraph `git0..git7` plus
  `gitInv0..gitInv7` from the current palette, and that explicit `gitN` values should derive
  `gitInvN` unless the inverse key is explicit too.
- Extended [crates/merman-core/src/theme.rs](/F:/SourceCodes/Rust/merman/crates/merman-core/src/theme.rs)
  with HSL color parsing, dark extended-theme primary palette derivation, `redux-dark*` GitGraph
  palette/inverse derivation, and explicit `gitN` inverse derivation.
- Found and fixed a second production rendering gap while adding render-path coverage:
  [crates/merman-render/src/pie.rs](/F:/SourceCodes/Rust/merman/crates/merman-render/src/pie.rs)
  previously built slice/legend colors from a hardcoded default palette. Pie layout now reads
  `effective_config.themeVariables.pie1..pie12`, preserving the default color-domain behavior when
  theme variables are absent.
- Added Pie and QuadrantChart regressions proving `redux-dark` `primaryColor` overrides reach
  visible slice/quadrant fills.

Source evidence:

- `repo-ref/mermaid/packages/mermaid/src/themes/theme-neo-dark.js`
- `repo-ref/mermaid/packages/mermaid/src/themes/theme-redux-dark.js`
- `repo-ref/mermaid/packages/mermaid/src/themes/theme-redux-dark-color.js`
- Installed Mermaid `11.15.0` dist probe through
  `tools/mermaid-cli/node_modules/mermaid/dist/mermaid.core.mjs`.

Focused verification:

- `cargo fmt --check -p merman-core -p merman-render -p merman`
- `$env:RUSTFLAGS='-C linker=rust-lld'; cargo nextest run -p merman-core theme`
- `$env:RUSTFLAGS='-C linker=rust-lld'; cargo nextest run -p merman-render --test pie_svg_test`
- `$env:RUSTFLAGS='-C linker=rust-lld'; cargo nextest run -p merman-render --test quadrantchart_svg_test`
- `$env:RUSTFLAGS='-C linker=rust-lld'; cargo nextest run -p merman --features render --test theme_renderability_smoke`

Residual note:

- This still avoids broad snapshot replacement or browser-color overfitting. The seam should grow
  only from source-backed rules that affect currently emitted SVG surfaces.

## HPD-080 - Journey Arrowhead Visible Signal Audit

Outcome:

- Audited Journey `themeVariables.arrowheadColor` after the renderability smoke had counted it as a
  visible theme signal.
- Confirmed pinned Mermaid 11.15 `user-journey/styles.js` emits `.arrowheadPath`, but
  `user-journey/svgDraw.js` creates the marker path without that class. Local Journey output mirrors
  that marker DOM.
- Removed `arrowheadColor` from the Journey case in
  [crates/merman/tests/theme_renderability_smoke.rs](/F:/SourceCodes/Rust/merman/crates/merman/tests/theme_renderability_smoke.rs)
  so the smoke no longer treats an inert CSS token as visible coverage.
- Updated [docs/workstreams/headless-parity-deepening/THEME_RENDERING_COVERAGE.md](/F:/SourceCodes/Rust/merman/docs/workstreams/headless-parity-deepening/THEME_RENDERING_COVERAGE.md)
  to track Journey arrowhead color as an upstream-inert provider rule, not a renderability signal.

Source evidence:

- `repo-ref/mermaid/packages/mermaid/src/diagrams/user-journey/styles.js`
- `repo-ref/mermaid/packages/mermaid/src/diagrams/user-journey/svgDraw.js`
- `crates/merman-render/src/svg/parity/journey.rs`

Focused verification:

- `$env:RUSTFLAGS='-C linker=rust-lld'; cargo nextest run -p merman --features render --test theme_renderability_smoke`

Residual note:

- No renderer DOM class was added. If merman later chooses to make `.arrowheadPath` effective as a
  headless renderability improvement, it should be tracked explicitly as a deliberate DOM divergence
  from pinned Mermaid output.

## HPD-080 - Resvg-Safe Audit Filtering

Outcome:

- Ran the ignored all-supported `resvg-safe` audit with render-only features and confirmed it passes
  across the current supported fixture corpus.
- Attempted the unfiltered all-supported raster audit; it exceeded the command timeout before
  producing fixture-level signal. Representative raster `resvg-safe` smoke still passed.
- Added optional filters to
  [crates/merman/tests/resvg_safe_fixture_smoke.rs](/F:/SourceCodes/Rust/merman/crates/merman/tests/resvg_safe_fixture_smoke.rs):
  `MERMAN_RESVG_SAFE_AUDIT_FAMILY` and `MERMAN_RESVG_SAFE_AUDIT_FILTER`.
- Default ignored-audit behavior is unchanged. Filtered audits now accept non-empty filtered corpora,
  which makes PNG-level triage usable without turning the full all-supported raster pass into a
  blocking gate.

Focused verification:

- `cargo fmt --check -p merman`
- `$env:RUSTFLAGS='-C linker=rust-lld'; cargo nextest run -p merman --features render --test resvg_safe_fixture_smoke`
- `$env:RUSTFLAGS='-C linker=rust-lld'; $env:MERMAN_RESVG_SAFE_AUDIT_FAMILY='journey'; cargo nextest run -p merman --features raster --test resvg_safe_fixture_smoke --run-ignored ignored-only all_supported_fixtures_render_headless_resvg_safe_audit`

Residual note:

- Use filtered raster audits for failure triage and keep representative raster smoke as the routine
  gate. The unfiltered raster corpus is still useful manually, but it should not be treated as a
  normal fast-pass command.

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
