# Mermaid 11.15 Root Viewport Residuals - TODO

Status: Active
Last updated: 2026-06-04

## M0 - Baseline Split

- [x] M15RV-010 [owner=codex] [deps=none] [scope=crates/xtask/src/cmd/compare/all.rs,target/compare,docs/workstreams/mermaid-11-15-root-viewport-residuals]
  Goal: Split root-only residual work out of the Mermaid 11.15 complete-adaptation campaign and
  make the full `parity-root` gate produce bounded, auditable failure summaries.
  Validation: `cargo nextest run -p xtask root_parity`;
  `cargo run -p xtask -- compare-all-svgs --check-dom --dom-mode parity-root --dom-decimals 3`.
  Review: The command may fail while residuals remain, but it must fail normally and point to
  per-diagram reports instead of crashing.
  Evidence: `EVIDENCE_AND_GATES.md`
  Handoff: DONE. Fresh full root evidence reports 309 unaccepted residuals after existing accepted
  root policy entries are applied.

## M1 - Largest Residual Buckets

- [x] M15RV-020 [owner=codex] [deps=M15RV-010] [scope=crates/merman-render/src/svg/parity/sequence,crates/merman-render/src/text,fixtures/upstream-svgs/sequence,target/compare/sequence_report_parity_root.md]
  Goal: Classify the Sequence root residual bucket and split source-derived lifecycle/frame/text
  rules from browser/root lattice tails.
  Validation: focused `compare-sequence-svgs` checks for any fixed bucket plus full structural
  `compare-all-svgs --dom-mode parity`.
  Review: Do not add broad root pins or per-string constants unless generated browser evidence
  proves a reusable measurement fact.
  Evidence: `EVIDENCE_AND_GATES.md`
  Handoff: DONE. Sequence now has a `--no-root-overrides` diagnostic path; 3 stale root pins were
  deleted. Fresh full root evidence has 168 raw Sequence mismatches and 167 unaccepted after the
  existing `zed_pr_57644_sequence` accepted residual. The central-connection source rules match
  Mermaid 11.15; remaining central rows are root-bounds/text-measurement residuals.

- [x] M15RV-030 [owner=codex] [deps=M15RV-010] [scope=crates/merman-render/src/flowchart,crates/merman-render/src/svg/parity/flowchart,target/compare/flowchart_report_parity_root.md]
  Goal: Classify the remaining Flowchart root residual bucket after the 11.15 shape-source slices.
  Validation: focused `compare-flowchart-svgs --dom-mode parity-root` checks, with
  `--no-root-overrides` where stale-pin diagnosis is relevant.
  Review: Source-derived Mermaid rules are allowed; new exact browser text constants should be
  generated or rejected as diagnostic residuals.
  Evidence: `EVIDENCE_AND_GATES.md`
  Handoff: DONE. Fresh report still has 61 Flowchart root mismatches after deleting 1 stale root
  pin. Disabling Flowchart root overrides increases mismatches to 96, so retained pins are mostly
  still useful. The remaining rows are small root/text-measurement tails: max absolute root width
  delta is about 2.24px, with 60 style mismatches and 1 viewBox mismatch.

- [x] M15RV-040 [owner=codex] [deps=M15RV-010] [scope=crates/merman-render/src/svg/parity/architecture,crates/merman-render/src/svg/parity/class,crates/merman-render/src/svg/parity/c4,target/compare]
  Goal: Classify Architecture, Class, and C4 root residuals into source-rule, root-pin, and
  diagnostic browser-root buckets.
  Validation: focused diagram root compares and `report-overrides --check-no-growth`.
  Review: Architecture has large group/port layout-root drifts; do not collapse them into broad
  root tolerances.
  Evidence: `EVIDENCE_AND_GATES.md`
  Handoff: DONE. C4 is green after refreshing 15 existing fixture-derived root pins to the
  Mermaid 11.15 upstream root values. Architecture remains at 32 unaccepted root residuals
  dominated by group/port/disconnected-component layout-root drift; disabling Architecture root
  pins increases raw mismatches from 32 to 63, so retained pins are still useful. Class has 18
  unaccepted root residuals after 2 existing accepted policy rows; there is no Class root pin table,
  and the largest rows are namespace/layout-width residuals rather than stale root pins.

## M2 - Smaller Residual Buckets

- [x] M15RV-050 [owner=codex] [deps=M15RV-010] [scope=crates/merman-render/src/svg/parity/er,crates/merman-render/src/svg/parity/sankey,crates/merman-render/src/svg/parity/timeline,crates/merman-render/src/svg/parity/journey,target/compare]
  Goal: Classify the smaller ER, Sankey, Timeline, and Journey residuals and close source-derived
  rows when cheap and defensible.
  Validation: focused diagram root compares and full structural parity.
  Review: Prefer deleting stale pins or accepting tiny browser-root residuals over adding new
  fixture-like string constants.
  Evidence: `EVIDENCE_AND_GATES.md`
  Handoff: DONE. ER and Sankey are root-green after refreshing existing fixture-derived root pins
  to Mermaid 11.15 upstream root values. Timeline was reduced from 7 to 3 by refreshing 4 existing
  root pins; the remaining rows are unpinned 0.5-1px root-width measurement tails. Journey remains
  at 2 unpinned 1.25-2px root-width measurement tails and has no root pin table.

## M2.5 - Source-Rule Residual Follow-Ups

- [x] M15RV-060 [owner=codex] [deps=M15RV-040] [scope=crates/merman-render/src/class.rs,crates/merman-render/src/svg/parity/class,crates/merman-render/tests/class_layout_test.rs,crates/merman-render/tests/class_svg_test.rs,target/compare/class_*]
  Goal: Reduce the Class namespace/layout-width root bucket with Mermaid source-derived compound
  graph rules instead of root viewport pins.
  Validation: focused Class namespace root compares, `cargo test -p merman-render --test
  class_layout_test`, and structural/root Class gates.
  Review: Keep changes in class graph construction/layout extraction. Do not add Class root
  override tables or per-fixture root constants.
  Evidence: `EVIDENCE_AND_GATES.md`
  Handoff: DONE_WITH_CONCERNS. Rust Class graph construction now mirrors the active Mermaid 11.15
  v3 unified path: `ClassDB.getData()` emits namespace group nodes before class/note/interface
  nodes, and the rendering-util Dagre extractor uses child-before-parent `copy(...)`, moved child
  extraction reparenting, and recursive `ranksep = parent.ranksep + 25`. SVG Class title wrapping
  now follows Mermaid's normal-weight `createText(...)` wrap before the outer bolder bbox, and
  numeric `themeVariables.fontSize` preserves raw CSS spelling without treating unitless CSS as a
  browser-effective px text size. `stress_class_nested_namespaces_cross_edges_008` is now
  root-green, Class structural parity is green, and full root evidence now leaves 12 unaccepted
  Class rows. Remaining Class rows are small SVG text/root tails plus known wider label residuals;
  do not close them by forcing browser font constants into headless layout.

- [x] M15RV-070 [owner=codex] [deps=M15RV-060] [scope=crates/merman-render/src/config.rs,crates/merman-render/src/class.rs,crates/merman-render/src/svg/parity/class/settings.rs,crates/merman-render/src/svg/parity/radar.rs]
  Goal: Extract shared font-size config helpers for Mermaid raw CSS interpolation and explicit-px
  SVG text measurement, then migrate the Class and Radar call sites that already had local copies.
  Validation: `cargo test -p merman-render config::tests`;
  `cargo test -p merman-render --test class_svg_test class_svg_preserves_numeric_theme_font_size_css_spelling`;
  focused Class/Radar SVG parity compares for numeric and px-string font-size fixtures.
  Review: This is a policy extraction, not a browser-exact measurement change. Do not broaden it
  to all diagrams until each diagram's Mermaid source path is checked.
  Evidence: `EVIDENCE_AND_GATES.md`
  Handoff: DONE. `config_css_number_or_string(...)` now captures Mermaid style interpolation where
  JSON numbers are emitted without adding `px`, while `config_f64_explicit_css_px(...)` captures
  the headless measurement rule where only explicit `px` strings are SVG-text effective for Class.
  Class and Radar fixtures stayed structurally green; full residual counts are unchanged.

- [x] M15RV-080 [owner=codex] [deps=M15RV-020,M15RV-070] [scope=crates/merman-render/src/sequence,crates/merman-render/src/text,crates/xtask/src/cmd/overrides/svg.rs,crates/merman-render/tests/sequence_svg_test.rs,target/compare/sequence_*]
  Goal: Repair the Sequence SVG text-measurement policy path, or explicitly document the residual
  if browser-derived `calculateTextDimensions(...)` behavior cannot be approximated without
  harming broader headless parity.
  Validation: focused central-connection Sequence tests and `compare-sequence-svgs` root reports;
  full Sequence/root reports before accepting any measurement-policy change.
  Review: Do not replace Sequence message spacing with the deterministic measurer just because it
  improves one fixture; a diagnostic probe improved the focused central row but increased the
  overall Sequence root mismatch count. Do not refresh the full Sequence SVG override table until
  the generator itself is proven against fixture-derived browser evidence.
  Evidence: `EVIDENCE_AND_GATES.md`
  Handoff: DONE_WITH_CONCERNS. The Sequence SVG override generator now uses the same default
  Puppeteer browser path as `mmdc`, skips wrap-sensitive fixtures and non-endpoint message seeds,
  and regenerates 891 auditable SVG text rows. Central-connection RTL root parity is exact, full
  Sequence structural parity is green, and raw Sequence root mismatches dropped from 168 to 68
  (67 unaccepted after the existing `zed_pr_57644_sequence` policy row). Remaining rows are mostly
  HTML `<br>` / wrap / note / participant height tails, not central-connection semantics.

- [x] M15RV-085 [owner=codex] [deps=M15RV-080] [scope=crates/xtask/src/cmd/overrides/svg.rs,crates/merman-render/src/generated/svg_overrides_sequence_11_12_2.rs,crates/merman-render/src/text,target/compare/sequence_report_parity_root.md]
  Goal: Classify and reduce the remaining Sequence HTML `<br>` / wrap / note / participant root
  tails after the SVG override generator repair.
  Validation: focused Sequence `parity-root` compares for the largest remaining rows;
  `cargo run -p xtask -- compare-sequence-svgs --check-dom --dom-mode parity --dom-decimals 3`;
  full `compare-all-svgs --dom-mode parity`.
  Review: Do not add browser-exact font constants for every string. Prefer reusable wrap/HTML
  source rules, generator-backed evidence, or explicit diagnostic residual policy.
  Evidence: `EVIDENCE_AND_GATES.md`
  Handoff: DONE_WITH_CONCERNS. Mermaid source confirms `<br>` splitting and `wrapLabel(...)`
  short-circuit behavior, while message/note sizing still runs through browser-derived
  `calculateTextDimensions(...)`. The SVG override generator now collects final emitted
  `actor`, `messageText`, and `noteText` nodes from wrap fixtures while still filtering raw
  wrap seeds. The generated Sequence SVG text table grew from 891 to 1036 auditable rows, and the
  text wrap path now uses exact final SVG evidence only as a guarded single-line fit signal.
  The four largest HTML `<br>` / wrap rows named above are root-exact, Sequence structural parity
  is green, and raw Sequence root mismatches dropped from 68 to 64. Full all-diagram root policy
  accepts the existing `zed_pr_57644_sequence` row, leaving 63 unaccepted Sequence residuals.
  Remaining rows are long left-of note width/height tails, small line-break width tails, and
  participant/actor-type height tails.

- [x] M15RV-087 [owner=codex] [deps=M15RV-085] [scope=crates/merman-render/src/sequence,crates/merman-render/src/generated/sequence_root_overrides_11_12_2.rs,target/compare/sequence_report_parity_root.md]
  Goal: Classify and reduce the remaining Sequence long-note, small line-break width, and
  actor-type height tails after the M15RV-085 wrap evidence update.
  Validation: focused Sequence `parity-root` compares for
  `upstream_cypress_sequencediagram_spec_should_render_long_notes_wrapped_inline_left_of_actor_026`,
  `upstream_cypress_sequencediagram_v2_spec_should_render_wrapped_long_notes_left_of_control_019`,
  `participant_types`, and `stress_participant_types_006`; full Sequence structural/root reports
  after any source-rule change.
  Review: Prefer source-derived note/actor geometry rules or explicit diagnostic status. Do not
  add root pins or per-string constants for the remaining 1-7px tails unless they come from a
  reusable generated measurement source.
  Evidence: `EVIDENCE_AND_GATES.md`
  Handoff: DONE_WITH_CONCERNS. Mermaid 11.15 source-derived actor-type glyph rules are now applied
  for database, boundary, control, and entity participants. Created actor top placement now follows
  `adjustCreatedDestroyedData(...)`'s pre-render actor-height anchor, and root bounds account for
  Mermaid's footer-row max-height cursor bump. Six stale Sequence root pins that were exact with
  root overrides disabled were deleted. Raw Sequence root mismatches dropped from 64 to 28, and
  full all-diagram root policy leaves 27 unaccepted Sequence rows after the accepted
  `zed_pr_57644_sequence` residual. Remaining rows are long-note and line-break/text-measurement
  tails, not actor-type geometry.

- [x] M15RV-088 [owner=codex] [deps=M15RV-087,M15RV-040] [scope=crates/merman-render/src/svg/parity/architecture,crates/merman-render/src/generated/architecture_root_overrides_11_12_2.rs,fixtures/upstream-svgs/architecture,target/compare/architecture_report_parity_root.md,repo-ref/mermaid/packages/mermaid/src/diagrams/architecture]
  Goal: Classify and reduce the remaining Architecture root residual bucket with source-derived
  group/port/disconnected-component root-bound rules where justified.
  Validation: focused Architecture `parity-root` compares with `--report-root-all` and
  `--no-root-overrides`, followed by full Architecture structural/root reports and full
  all-diagram structural parity after any renderer change.
  Review: Do not turn the 30 Architecture rows into broad root pins or tolerances. Prefer Mermaid
  source rules, stale-pin deletion, or explicit diagnostic status for `foreignObject`/browser bbox
  tails.
  Evidence: `EVIDENCE_AND_GATES.md`
  Handoff: DONE_WITH_CONCERNS. Fresh Mermaid 11.15 Architecture upstream SVGs were regenerated
  for all 185 stored fixtures, proving the simple-service 170x165 root and bare-ID examples were
  stale baseline data rather than renderer bugs. Rust now emits Mermaid 11.15 diagram-scoped
  edge/service/node/group IDs and the current fallback service background path from
  `svgDraw.ts`; all 31 stale Architecture root viewport pins and the old groups-within-groups
  calibration were removed. Architecture structural parity is green, Architecture has zero root
  override entries, and full root evidence now reports 32 honest Architecture rows. The count
  increase from 30 to 32 reflects baseline cleanup exposing current 11.15 root tails; do not
  restore old pins or calibrations to recover the smaller number.

- [ ] M15RV-089 [owner=codex] [deps=M15RV-088] [scope=crates/merman-render/src/architecture.rs,crates/merman-render/tests/architecture_svg_test.rs,crates/merman-render/src/svg/parity/architecture,tools/debug/arch_fcose_browser_probe_fixture_025.js,target/compare/architecture_report_parity_root_after_m15rv089_group_padding_metric_refactor_only.md,repo-ref/mermaid/packages/mermaid/src/diagrams/architecture]
  Goal: Investigate the top Architecture FCoSE/group-port root residuals now that the 11.15
  upstream baseline and root-pin table are honest.
  Validation: focused Architecture `parity-root` compares for
  `stress_architecture_junction_fork_join_026`, `stress_architecture_fan_in_out_021`,
  `stress_architecture_deep_nesting_013`, and
  `stress_architecture_batch6_junctions_multi_split_with_group_edges_087`, followed by full
  Architecture structural/root reports and full all-diagram structural parity after any renderer
  change.
  Review: Implement only source-derived layout/root-bound rules or reusable headless
  approximations. Do not add broad Architecture root pins, root tolerances, or browser-dependent
  font/foreignObject hacks just to reduce the count.
  Evidence: `EVIDENCE_AND_GATES.md`
  Handoff: IN_PROGRESS. Mermaid 11.15 source checks closed two Architecture FCoSE input bugs:
  junction Cytoscape parents now come only from `junction.in`, and group alignment overwrites now
  follow `ArchitectureDB.getDataStructures()`'s `this.nodes -> service.edges` endpoint traversal
  instead of a single global edge pass. Architecture structural parity and full all-diagram
  structural parity remain green. Architecture root residuals dropped from 32 to 29:
  `stress_architecture_fan_in_out_021`,
  `stress_architecture_batch6_junctions_multi_split_with_group_edges_087`, and
  `stress_architecture_deep_nesting_013` are exact, while
  `stress_architecture_junction_fork_join_026` remains a smaller `+14px` tail. A follow-up source
  check showed Architecture group style uses configured `padding`, not `iconSize / 2`; Rust group
  rect sizing now follows `padding + 2.5`, and the duplicate Cytoscape canvas-label width
  approximation was extracted into a shared helper. The custom-init row
  `stress_architecture_batch6_init_fontsize_icon_size_wrap_093` improved from about `-22.5px` to
  about `-2.5px` without changing the 29-row Architecture failure set. A subsequent source check
  matched Mermaid's relative-constraint BFS duplicate-pop behavior, so `junction_fork_join` now
  feeds 9 relative constraints like the browser probe instead of Rust's previous 7; this did not
  change the `+13.976px` viewport tail. Layout-side pre-FCoSE group bbox inflation now also uses
  configured `padding + 2.5` instead of the old `iconSize / 2 + 2.5` proxy; this aligns layout and
  SVG group sizing policy but does not change the current 29-row residual set. The committed
  piecewise long-label canvas approximation (`measured width >= 200px -> scale 1.01`) reduces
  `stress_architecture_batch5_long_titles_and_punct_076` from `+10px` to `+5px` without moving the
  nearby diagnostic matrix rows; do not tune one global label scale from that row. Fresh
  2026-06-03 Architecture reports after the HPD-050 isolated-service seam keep structural parity
  green and show `25` root mismatches, not the older `29` queue. Do not reopen rows that are now
  exact, including `stress_architecture_batch4_init_small_icons_061`,
  `stress_architecture_batch4_init_fontsize_wrap_063`,
  `stress_architecture_edge_label_corner_cases_012`,
  `stress_architecture_fan_in_out_021`, `stress_architecture_deep_nesting_013`,
  `stress_architecture_batch6_junctions_multi_split_with_group_edges_087`, and
  `stress_architecture_disconnected_islands_046`. Continue from the remaining larger tails:
  `stress_architecture_junction_fork_join_026` (`+13.976px`),
  `stress_architecture_batch5_long_titles_and_punct_076` (`+5px`),
  `stress_architecture_html_titles_and_escapes_041` (`+5px`),
  `stress_architecture_unicode_and_xml_escapes_019` (`+3px`),
  `stress_architecture_batch6_init_fontsize_icon_size_wrap_093` (`-2.5px`),
  `stress_architecture_nested_groups_002` (`+2.5px`), and
  `stress_architecture_group_port_edges_017` (`+1.468px`), treating the smaller
  browser/Cytoscape bbox lattice as diagnostic unless a reusable generated rule is found.
  Additional focused checks classified `stress_architecture_html_titles_and_escapes_041` as a
  group/service Cytoscape bbox tail (not an HTML/entity or edge-label source bug), and
  `stress_architecture_group_port_edges_017` as source-input-matched manatee vs cytoscape-fcose
  solver/compound-bound drift. Do not tune group-edge shifts, root pins, or one-off metric
  constants from those rows. Further focused diagnostics classify
  `stress_architecture_unicode_and_xml_escapes_019` as the same group/service bbox class,
  `stress_architecture_edge_label_corner_cases_012` and
  `stress_architecture_batch4_init_fontsize_wrap_063` as edge-label browser `getBBox()` tails, but
  those two rows are now root-green after the later HPD-050 `createText()` root-bounds fix. Keep
  `stress_architecture_nested_groups_002` classified as a nested-compound/FCoSE residual after
  source inputs match. Latest HPD-050 FCoSE geometry evidence closes
  `stress_architecture_batch6_junctions_multi_split_with_group_edges_087` again by treating
  near-touch rectangles and near-equal overlap centers with a `1e-9` geometry epsilon; full
  Architecture structural parity stays green and Architecture `parity-root` now has `24` mismatch
  rows led by the existing `+5px` long-title/HTML-title tails.

## M3 - Policy Closeout

- [ ] M15RV-090 [owner=planner] [deps=M15RV-020,M15RV-030,M15RV-040,M15RV-050,M15RV-060,M15RV-070,M15RV-080,M15RV-085,M15RV-087,M15RV-088,M15RV-089] [scope=docs/workstreams/mermaid-11-15-root-viewport-residuals,crates/xtask/src/cmd/compare/all.rs]
  Goal: Close the root residual lane by either making `parity-root` green or accepting only
  documented diagnostic residuals with fresh evidence.
  Validation: `cargo run -p xtask -- compare-all-svgs --check-dom --dom-mode parity --dom-decimals 3`;
  `cargo run -p xtask -- compare-all-svgs --check-dom --dom-mode parity-root --dom-decimals 3`;
  `cargo run -p xtask -- report-overrides --check-no-growth`; `cargo fmt --check`;
  `git diff --check`.
  Review: Run workstream review and verification before closing.
  Evidence: `EVIDENCE_AND_GATES.md`
  Handoff: Blocked by source-rule follow-ups. Do not close by accepting the last-known full-root
  residual set; rerun fresh all-diagram `parity-root` evidence first. Flowchart, Sequence,
  Architecture, and Class still contain real text/layout/root-bounds differences.
