# Mermaid 11.15 Root Viewport Residuals - Evidence And Gates

Status: Active
Last updated: 2026-06-02

## Starting Evidence

Fresh gates from 2026-06-01:

- `cargo run -p xtask -- verify-generated`: passed.
- `cargo run -p xtask -- check-alignment`: passed.
- `cargo run -p xtask -- report-overrides --check-no-growth`: passed with root viewport
  overrides = 282, text metric lookup overrides = 495, SVG text metric table rows = 186, and
  Flowchart font metric table rows = 3774.
- `cargo run -p xtask -- compare-all-svgs --check-dom --dom-mode parity --dom-decimals 3`:
  passed for the implemented matrix.
- `cargo run -p xtask -- compare-all-svgs --check-dom --dom-mode parity-root --dom-decimals 3`:
  failed normally after the bounded-summary xtask fix.
- `cargo nextest run -p xtask root_parity`: passed, 5 tests.
- `cargo fmt --check`: passed.
- `git diff --check`: passed with a line-ending warning for the parent workstream
  `CONTEXT.jsonl` only.

Full `parity-root` accepted existing policy residuals:

- class: 2 accepted rows.
- sequence: 1 accepted row.
- gitgraph: 1 accepted row.
- mindmap: 4 accepted rows.

Fresh unaccepted residual summary from the full `parity-root` failure:

| Diagram | Unaccepted residuals | Report |
| --- | ---: | --- |
| Sequence | 168 | `target/compare/sequence_report_parity_root.md` |
| Flowchart | 61 | `target/compare/flowchart_report_parity_root.md` |
| Architecture | 32 | `target/compare/architecture_report_parity_root.md` |
| Class | 18 | `target/compare/class_report_parity_root.md` |
| C4 | 15 | `target/compare/c4_report_parity_root.md` |
| Timeline | 7 | `target/compare/timeline_report_parity_root.md` |
| ER | 3 | `target/compare/er_report_parity_root.md` |
| Sankey | 3 | `target/compare/sankey_report_parity_root.md` |
| Journey | 2 | `target/compare/journey_report_parity_root.md` |

Total unaccepted residuals: 309.

## M15RV-020 - Sequence Classification

Fresh evidence from 2026-06-01:

- `cargo nextest run -p merman-render sequence_root_overrides_can_be_disabled_per_render_options`:
  passed.
- `cargo run -p xtask -- compare-sequence-svgs --filter central_connection --check-dom --dom-mode parity-root --dom-decimals 3 --report-root-all --out target/compare/sequence_central_after_stale_pin_trim.md`:
  expected failure; all 6 remaining central-connection rows are root-only mismatches.
- `cargo run -p xtask -- compare-sequence-svgs --filter central_connection --check-dom --dom-mode parity-root --dom-decimals 3 --report-root-all --no-root-overrides --out target/compare/sequence_central_cli_no_root_overrides.md`:
  expected failure; verifies the new CLI diagnostic switch reaches the Sequence renderer.
- `cargo run -p xtask -- compare-sequence-svgs --check-dom --dom-mode parity-root --dom-decimals 3 --report-root-all --out target/compare/sequence_report_parity_root_after_stale_pin_trim.md`:
  expected failure with 168 raw Sequence root mismatches.
- `cargo run -p xtask -- compare-all-svgs --check-dom --dom-mode parity --dom-decimals 3`:
  passed for the implemented matrix.
- `cargo run -p xtask -- report-overrides --check-no-growth`: passed with root viewport
  overrides reduced from 282 to 279 total entries; Sequence root overrides reduced to 55 entries.
- `cargo run -p xtask -- compare-all-svgs --check-dom --dom-mode parity-root --dom-decimals 3`:
  expected failure with bounded summary and 308 unaccepted residuals.

Sequence classification:

- Three stale Sequence root pins were deleted:
  `upstream_cypress_sequencediagram_v2_spec_should_render_central_connection_with_normal_arrows_right_to_lef_033`,
  `upstream_cypress_sequencediagram_v2_spec_should_render_central_connections_with_bidirectional_arrows_and_045`,
  and `upstream_docs_directives_changing_sequence_diagram_config_via_directive_016`.
- The two stale central-connection pins had been inflating residuals to `+238` and `+941`.
  After deletion, the central-connection bucket is bounded to `+63`, `+54`, `+49`, `+32`,
  `+29`, and `+29`.
- Mermaid 11.15 source check: `repo-ref/mermaid/packages/mermaid/src/diagrams/sequence/sequenceRenderer.ts`
  defines `CENTRAL_CONNECTION_BASE_OFFSET = 4`, `CENTRAL_CONNECTION_BIDIRECTIONAL_OFFSET = 6`,
  and `CENTRAL_CONNECTION_CIRCLE_OFFSET = 16.5`; Rust layout/render code already mirrors those
  constants in `crates/merman-render/src/sequence/messages.rs` and
  `crates/merman-render/src/svg/parity/sequence/messages.rs`.
- Remaining Sequence root rows are therefore split as root-bounds/text-measurement/browser-lattice
  residuals, not missing central-connection semantics. Fresh full-root accepted policy removes
  `sequence/zed_pr_57644_sequence`, leaving 167 unaccepted Sequence rows.

Fresh unaccepted residual summary after M15RV-020:

| Diagram | Unaccepted residuals | Report |
| --- | ---: | --- |
| Sequence | 167 | `target/compare/sequence_report_parity_root.md` |
| Flowchart | 61 | `target/compare/flowchart_report_parity_root.md` |
| Architecture | 32 | `target/compare/architecture_report_parity_root.md` |
| Class | 18 | `target/compare/class_report_parity_root.md` |
| C4 | 15 | `target/compare/c4_report_parity_root.md` |
| Timeline | 7 | `target/compare/timeline_report_parity_root.md` |
| ER | 3 | `target/compare/er_report_parity_root.md` |
| Sankey | 3 | `target/compare/sankey_report_parity_root.md` |
| Journey | 2 | `target/compare/journey_report_parity_root.md` |

Total unaccepted residuals: 308.

## M15RV-030 - Flowchart Classification

Fresh evidence from 2026-06-01:

- `cargo run -p xtask -- compare-flowchart-svgs --check-dom --dom-mode parity-root --dom-decimals 3 --report-root-all --out target/compare/flowchart_report_parity_root_all.md`:
  expected failure with 61 Flowchart root mismatches.
- `cargo run -p xtask -- compare-flowchart-svgs --check-dom --dom-mode parity-root --dom-decimals 3 --report-root-all --no-root-overrides --out target/compare/flowchart_report_parity_root_no_overrides_all.md`:
  expected failure with 96 Flowchart root mismatches, proving the retained root pins are mostly
  still reducing root-only drift.
- `cargo run -p xtask -- compare-flowchart-svgs --filter upstream_cypress_newshapes_spec_newshapessets_newshapesset4_tb_md_html_false_030 --report-root-all --report-label-all --out target/compare/flowchart_newshape4_tb_label_deltas.md`:
  passed and showed the largest root residual has two SVG text node labels with `-0.602px` width
  deltas each.
- `cargo run -p xtask -- compare-flowchart-svgs --filter upstream_cypress_flowchart_spec_17_render_multiline_texts_017 --check-dom --dom-mode parity-root --dom-decimals 3 --report-root-all --out target/compare/flowchart_multiline_texts_after_stale_pin_trim.md`:
  expected failure; deleting the stale pin improved that fixture from `-1.000px` to `-0.883px`.
- `cargo run -p xtask -- compare-flowchart-svgs --check-dom --dom-mode parity-root --dom-decimals 3 --report-root-all --out target/compare/flowchart_report_parity_root_after_stale_pin_trim.md`:
  expected failure with 61 Flowchart root mismatches after pin deletion.
- `cargo run -p xtask -- compare-flowchart-svgs --check-dom --dom-mode parity --dom-decimals 3 --out target/compare/flowchart_report_parity_after_m15rv030.md`:
  passed.
- `cargo run -p xtask -- report-overrides --check-no-growth`: passed with root viewport
  overrides reduced from 279 to 278 total entries; Flowchart root overrides reduced to 38 entries.

Flowchart classification:

- One stale Flowchart root pin was deleted:
  `upstream_cypress_flowchart_spec_17_render_multiline_texts_017`.
- With root overrides enabled, Flowchart has 61 root mismatches: 60 style/max-width mismatches and
  1 viewBox mismatch.
- The enabled-root all-row table has no large source-rule bucket: maximum absolute root width
  delta is about `2.24px`; 835 rows are exact, 187 are within `0.25px`, and only 2 rows are over
  `2px`.
- Disabling Flowchart root overrides increases mismatch count to 96 and introduces larger
  residuals up to about `22.28px`, so broad pin deletion would be a regression.
- The dominant residual pattern is SVG text/BBox measurement drift across markdown/htmlLabels
  false new/old shape fixtures, arrow/line fixtures, and subgraph/title fixtures. These should be
  handled through shared measurement work or explicit diagnostic policy, not by hand-written root
  constants.

## M15RV-040 - Architecture, Class, And C4 Classification

Fresh evidence from 2026-06-01:

- `cargo run -p xtask -- compare-c4-svgs --check-dom --dom-mode parity-root --dom-decimals 3 --out target/compare/c4_report_parity_root_after_refresh.md`:
  passed after refreshing 15 existing C4 root viewport entries to the current Mermaid 11.15
  upstream SVG root `viewBox`/`max-width` values.
- `cargo run -p xtask -- compare-c4-svgs --check-dom --dom-mode parity --dom-decimals 3 --out target/compare/c4_report_parity_after_m15rv040.md`:
  passed.
- `MERMAN_DISABLE_ROOT_VIEWPORT_OVERRIDES=1 cargo run -p xtask -- compare-c4-svgs --check-dom --dom-mode parity-root --dom-decimals 3 --out target/compare/c4_report_parity_root_no_overrides.md`:
  expected failure with 35 raw C4 root mismatches, proving the retained C4 root table still
  reduces browser-root drift.
- `MERMAN_DISABLE_ROOT_VIEWPORT_OVERRIDES=1 cargo run -p xtask -- compare-architecture-svgs --check-dom --dom-mode parity-root --dom-decimals 3 --out target/compare/architecture_report_parity_root_no_overrides.md`:
  expected failure with 63 raw Architecture root mismatches; the enabled report has 32 raw
  mismatches, so retained Architecture root pins are still useful.
- `cargo run -p xtask -- compare-all-svgs --check-dom --dom-mode parity --dom-decimals 3`:
  passed for the implemented matrix.
- `cargo run -p xtask -- compare-all-svgs --check-dom --dom-mode parity-root --dom-decimals 3`:
  expected failure with bounded summary and 293 unaccepted residuals. C4 no longer appears in the
  unaccepted summary.
- `cargo run -p xtask -- report-overrides --check-no-growth`: passed with root viewport
  overrides still at 278 total entries; C4 still has 35 entries.
- `cargo fmt --check`: passed.

Architecture classification:

- With root overrides enabled, Architecture has 32 root mismatches: 26 `style=max-width`
  mismatches and 6 `viewBox` mismatches.
- Disabling Architecture root overrides increases raw mismatches to 63, including very large
  already-pinned layout drift rows such as `stress_architecture_deep_group_chain_027` and
  `stress_architecture_junction_fork_join_026`.
- The remaining enabled Architecture rows are not stale-root-pin candidates. The largest rows are
  group/port/disconnected-component layout-root differences, for example
  `stress_architecture_batch6_disconnected_components_with_titles_089` (`-247px`),
  `stress_architecture_mixed_service_forms_009` (`-183px`),
  `stress_architecture_batch3_port_pairs_corner_cases_058` (`-177.5px`), and
  `stress_architecture_many_small_groups_025` (`+151px`).
- These rows should be handled by Architecture layout/root-bound work, not by broad tolerances or
  new fixture pins.

Class classification:

- The Class focused report has 20 raw root mismatches, all `style=max-width`; the full root gate
  already accepts 2 existing policy rows, leaving 18 unaccepted Class residuals.
- Class has no root viewport override table, so this bucket is not a stale-pin cleanup bucket.
- The largest rows are namespace/layout-width residuals, for example
  `upstream_pkgtests_classdiagram_spec_003` and
  `upstream_html_demos_classchart_class_diagram_demos_010` (`+514.25px` each),
  `upstream_pkgtests_classdiagram_spec_006` (`+300px`), and the nested namespace fixtures
  (`+151.5px` each).

C4 classification:

- C4's 15 root residuals were all existing root-pin rows whose old fixture-derived values were
  off by 1-2px against the Mermaid 11.15 upstream SVG roots.
- Refreshing those existing rows closed C4 root parity without adding entries. This is a baseline
  refresh of governed fixture-derived data, not a new renderer-side measurement rule.

Fresh unaccepted residual summary after M15RV-040:

| Diagram | Unaccepted residuals | Report |
| --- | ---: | --- |
| Sequence | 167 | `target/compare/sequence_report_parity_root.md` |
| Flowchart | 61 | `target/compare/flowchart_report_parity_root.md` |
| Architecture | 32 | `target/compare/architecture_report_parity_root.md` |
| Class | 18 | `target/compare/class_report_parity_root.md` |
| Timeline | 7 | `target/compare/timeline_report_parity_root.md` |
| ER | 3 | `target/compare/er_report_parity_root.md` |
| Sankey | 3 | `target/compare/sankey_report_parity_root.md` |
| Journey | 2 | `target/compare/journey_report_parity_root.md` |

Total unaccepted residuals: 293.

## M15RV-050 - Smaller Bucket Classification

Fresh evidence from 2026-06-01:

- `MERMAN_DISABLE_ROOT_VIEWPORT_OVERRIDES=1 cargo run -p xtask -- compare-er-svgs --check-dom --dom-mode parity-root --dom-decimals 3 --out target/compare/er_report_parity_root_no_overrides.md`:
  expected failure with 6 raw ER root mismatches.
- `cargo run -p xtask -- compare-er-svgs --check-dom --dom-mode parity-root --dom-decimals 3 --out target/compare/er_report_parity_root_after_m15rv050.md`:
  passed after refreshing 3 existing ER root viewport entries to Mermaid 11.15 upstream root
  values.
- `MERMAN_DISABLE_ROOT_VIEWPORT_OVERRIDES=1 cargo run -p xtask -- compare-sankey-svgs --check-dom --dom-mode parity-root --dom-decimals 3 --out target/compare/sankey_report_parity_root_no_overrides.md`:
  expected failure with 3 raw Sankey root mismatches.
- `cargo run -p xtask -- compare-sankey-svgs --check-dom --dom-mode parity-root --dom-decimals 3 --out target/compare/sankey_report_parity_root_after_m15rv050.md`:
  passed after refreshing 3 existing Sankey root viewport entries to Mermaid 11.15 upstream root
  values.
- `MERMAN_DISABLE_ROOT_VIEWPORT_OVERRIDES=1 cargo run -p xtask -- compare-timeline-svgs --check-dom --dom-mode parity-root --dom-decimals 3 --out target/compare/timeline_report_parity_root_no_overrides.md`:
  expected failure with 10 raw Timeline root mismatches.
- `cargo run -p xtask -- compare-timeline-svgs --check-dom --dom-mode parity-root --dom-decimals 3 --out target/compare/timeline_report_parity_root_after_m15rv050.md`:
  expected failure with 3 Timeline root mismatches after refreshing 4 existing root pins.
- `cargo run -p xtask -- compare-journey-svgs --check-dom --dom-mode parity-root --dom-decimals 3 --out target/compare/journey_report_parity_root_after_m15rv050.md`:
  expected failure with 2 Journey root mismatches.
- Focused structural gates passed:
  `compare-er-svgs --dom-mode parity`,
  `compare-sankey-svgs --dom-mode parity`,
  and `compare-timeline-svgs --dom-mode parity`.
- `cargo run -p xtask -- compare-all-svgs --check-dom --dom-mode parity --dom-decimals 3`:
  passed for the implemented matrix.
- `cargo run -p xtask -- compare-all-svgs --check-dom --dom-mode parity-root --dom-decimals 3`:
  expected failure with bounded summary and 283 unaccepted residuals. ER and Sankey no longer
  appear in the unaccepted summary.
- `cargo run -p xtask -- report-overrides --check-no-growth`: passed with root viewport
  overrides still at 278 total entries; ER has 6 entries, Sankey has 3 entries, and Timeline has
  8 entries.

ER classification:

- ER's 3 residuals were existing root-pin rows whose old fixture-derived values were stale against
  Mermaid 11.15. Refreshing those entries closed ER root parity without adding entries.

Sankey classification:

- Sankey's 3 residuals were existing root-pin rows whose viewBox heights were stale against
  Mermaid 11.15. Refreshing those entries closed Sankey root parity without adding entries.

Timeline classification:

- Four existing Timeline root pins were refreshed to Mermaid 11.15 upstream values:
  `timeline_stress_common_long_unbroken_words`,
  `timeline_stress_events_with_entities_and_ampersands`,
  `timeline_stress_unicode_cjk_and_emoji`, and
  `timeline_stress_very_long_unbroken_word`.
- Three Timeline rows remain and were intentionally not converted into new fixture pins:

## M15RV-089 - Architecture Source-Rule Follow-Up

Fresh evidence from 2026-06-02:

- `cargo test -p merman-render architecture::tests -- --nocapture`:
  passed after the Architecture compound-bbox padding helper extraction.
- `cargo test -p merman-render --test architecture_svg_test architecture_group_rect_uses_configured_padding_for_small_icons -- --nocapture`:
  passed.
- `cargo test -p merman-render --test architecture_svg_test architecture_icon_text_clamp_uses_architecture_font_size -- --nocapture`:
  passed.
- `cargo run -p xtask -- compare-architecture-svgs --filter upstream_architecture_docs_service_icon_text --check-dom --dom-mode parity-root --dom-decimals 3 --report-root-all --out target/compare/architecture_m15rv089_icontext_doc.md`:
  passed.
- `cargo run -p xtask -- compare-architecture-svgs --filter stress_architecture_batch6_init_fontsize_icon_size_wrap_093 --check-dom --dom-mode parity-root --dom-decimals 3 --report-root-all --out target/compare/architecture_m15rv089_icontext_093.md`:
  expected failure; this fixture has no `iconText`, so it is not evidence for the iconText clamp path.
- `cargo run -p xtask -- compare-architecture-svgs --filter upstream_architecture_cypress_fallback_icon --check-dom --dom-mode parity-root --dom-decimals 3 --report-root-all --out target/compare/architecture_m15rv089_fallback_icon.md`:
  expected failure; the `-0.25px` fallback-icon width tail is unaffected by the iconText clamp path.

Architecture follow-up notes:

- The Architecture compound/group bbox empirical `padding + 2.5` rule is now a shared helper in
  `crates/merman-render/src/architecture.rs`, reused by both layout and SVG parity geometry.
  This is a no-behavior-change refactor that reduces the chance of layout/parity drift.
- Mermaid 11.15 `svgDraw.ts` computes `iconText` line clamp from the DOM-applied `font-size`
  inside the `foreignObject`, not from the SVG text label font-size path. Rust now follows that
  policy by using `architecture.fontSize` (`arch_font_size_px`) for `iconText`
  `-webkit-line-clamp`, instead of the separate SVG text measurement size.
- The focused docs fixture `upstream_architecture_docs_service_icon_text` remains root-green after
  this change, so the policy alignment is source-backed and non-regressive.
- This is not the main explanation for the remaining Architecture root bucket. The current
  fallback-icon `-0.25px` tail and the larger disconnected/group/FCoSE residuals remain separate
  issues; do not over-claim this clamp fix as a bucket-wide residual reduction.
  `timeline_stress_accdescr_block_multiline` (`896px` upstream vs `895px` local),
  `timeline_stress_width_large_and_long_labels` (`896px` upstream vs `895px` local), and
  `upstream_long_word_wrap` (`961.5px` upstream vs `961px` local). These are small
  root-width/text-measurement tails.

Journey classification:

- Journey has no root viewport override table. The 2 remaining rows are unpinned small root-width
  tails: `upstream_cypress_journey_spec_should_maintain_sufficient_space_between_legend_and_diagram_when_007`
  (`2599.25px` upstream vs `2597.25px` local) and
  `upstream_cypress_journey_spec_should_wrap_text_on_whitespace_without_adding_hyphens_009`
  (`884.5px` upstream vs `883.25px` local).

Fresh unaccepted residual summary after M15RV-050:

| Diagram | Unaccepted residuals | Report |
| --- | ---: | --- |
| Sequence | 167 | `target/compare/sequence_report_parity_root.md` |
| Flowchart | 61 | `target/compare/flowchart_report_parity_root.md` |
| Architecture | 32 | `target/compare/architecture_report_parity_root.md` |
| Class | 18 | `target/compare/class_report_parity_root.md` |
| Timeline | 3 | `target/compare/timeline_report_parity_root.md` |
| Journey | 2 | `target/compare/journey_report_parity_root.md` |

Total unaccepted residuals: 283.

## M15RV-060 - Class Namespace Compound Layout Follow-Up

Fresh evidence from 2026-06-01:

- `cargo test -p merman-render --test class_layout_test`: passed, 12 tests.
- `cargo run -p xtask -- compare-class-svgs --filter basic --check-dom --dom-mode parity --dom-decimals 3 --out target/compare/class_basic_after_nodeborder_fix.md`:
  passed after the Class renderer switched its default node fill/stroke source to Mermaid's
  `mainBkg`/`nodeBorder` variables instead of `primaryColor`/`primaryBorderColor`.
- `cargo run -p xtask -- compare-class-svgs --check-dom --dom-mode parity --dom-decimals 3 --out target/compare/class_report_parity_after_namespace_compound.md`:
  passed for the current Class matrix.
- `cargo run -p xtask -- compare-class-svgs --filter upstream_pkgtests_classdiagram_spec_003 --check-dom --dom-mode parity-root --dom-decimals 3 --out target/compare/class_pkgtests_003_after_namespace_compound.md`:
  expected failure; the fixture is no longer the old wrong horizontal layout (`1014px` local
  max-width) and now renders as a vertical compound layout (`444.5px` local vs `499.75px`
  upstream).
- `cargo run -p xtask -- compare-class-svgs --filter stress_class_nested_namespaces_cross_edges_008 --check-dom --dom-mode parity-root --dom-decimals 3 --out target/compare/class_stress_nested_cross_edges_after_namespace_compound.md`:
  expected failure; the LR nested namespace fixture now renders near the upstream vertical stack
  (`277.75px` local vs `257.5px` upstream).
- `cargo run -p xtask -- compare-class-svgs --check-dom --dom-mode parity-root --dom-decimals 3 --out target/compare/class_report_parity_root_after_namespace_compound.md`:
  expected failure with 32 raw Class root mismatches in the current worktree.
- `cargo test -p merman-core theme`: passed, 9 tests, after the theme snapshot merge test was
  updated to require exact equality for snapshot-only themes and key coverage for hand-derived
  existing themes.
- `cargo run -p xtask -- compare-sequence-svgs --filter activation_explicit --check-dom --dom-mode parity --dom-decimals 3 --out target/compare/sequence_activation_after_theme_snapshot_merge.md`:
  passed after Sequence activation rect attributes were realigned with Mermaid
  `svgDraw.getNoteRect()`.
- `cargo run -p xtask -- compare-treemap-svgs --filter stress_treemap_font_size_precedence_001 --check-dom --dom-mode parity --dom-decimals 3 --out target/compare/treemap_font_size_after_theme_snapshot.md`:
  passed after the default-theme Treemap color-scale renderer path was kept on the SVG baseline
  defaults.
- `cargo run -p xtask -- compare-xychart-svgs --filter upstream_cypress_xychart_spec_should_render_a_single_bar_with_label_for_a_vertical_xy_chart_026 --check-dom --dom-mode parity --dom-decimals 3 --out target/compare/xychart_label_after_theme_snapshot.md`:
  passed after default-theme XYChart data labels were kept on the SVG baseline color.
- `cargo run -p xtask -- compare-all-svgs --check-dom --dom-mode parity --dom-decimals 3`:
  passed for the implemented matrix in the current worktree.
- `cargo run -p xtask -- report-overrides --check-no-growth`: passed with root viewport overrides
  still at 278 entries.
- `cargo run -p xtask -- compare-all-svgs --check-dom --dom-mode parity-root --dom-decimals 3`:
  expected failure with bounded summary and 293 unaccepted residuals.
- `cargo fmt --check`: passed.
- `git diff --check`: passed with a line-ending warning for this workstream's `CONTEXT.jsonl`.
- `cargo test -p merman-render --test class_layout_test`: passed, 14 tests, after adding
  regression coverage for LR namespace cross-edge extraction and child-before-parent namespace
  extraction.
- `cargo run -p xtask -- compare-class-svgs --filter upstream_namespaces_and_generics --check-dom --dom-mode parity-root --dom-decimals 3 --out target/compare/class_namespaces_generics_final.md`:
  passed after aligning the Class extractor with Mermaid 11.15's child-first copy order and
  recursive `ranksep + 25` behavior.
- `cargo run -p xtask -- compare-class-svgs --filter upstream_pkgtests_classdiagram_spec_006 --check-dom --dom-mode parity-root --dom-decimals 3 --out target/compare/class_pkgtests_006_final.md`:
  passed after moved child extractions were reparented under later parent extractions.
- `cargo run -p xtask -- compare-class-svgs --filter stress_class_nested_namespaces_many_levels_021 --check-dom --dom-mode parity-root --dom-decimals 3 --out target/compare/class_many_levels_nested_move.md`:
  passed.
- `cargo run -p xtask -- compare-class-svgs --filter upstream_pkgtests_classdiagram_spec_003 --check-dom --dom-mode parity-root --dom-decimals 3 --out target/compare/class_pkgtests_003_final.md`:
  expected failure; the fixture now has only a `0.25px` root width tail (`499.5px` local vs
  `499.75px` upstream).
- `cargo run -p xtask -- compare-class-svgs --check-dom --dom-mode parity-root --dom-decimals 3 --out target/compare/class_report_parity_root_after_v3_compound.md`:
  expected failure with 15 raw Class root mismatches.
- `cargo run -p xtask -- compare-class-svgs --check-dom --dom-mode parity --dom-decimals 3 --out target/compare/class_report_parity_after_v3_compound.md`:
  passed for the current Class matrix.
- `cargo run -p xtask -- compare-all-svgs --check-dom --dom-mode parity --dom-decimals 3`:
  passed for the implemented matrix.
- `cargo run -p xtask -- report-overrides --check-no-growth`: passed with root viewport overrides
  still at 278 entries.
- `cargo fmt --check`: passed.
- `git diff --check`: passed.
- `cargo run -p xtask -- compare-all-svgs --check-dom --dom-mode parity-root --dom-decimals 3`:
  expected failure with bounded summary and 278 unaccepted residuals.

Class source-rule findings:

- Early diagnosis against the old `classRenderer-v2.ts:addNamespaces(...)` path was superseded by
  Mermaid 11.15 source inspection of the active Class renderer. `classDiagram.ts` uses
  `classRenderer-v3-unified.ts`, which gets graph data through `ClassDB.getData()`. That method
  emits all namespace group nodes before class, note, and interface nodes. Rust now mirrors that
  namespace-first source order instead of inserting namespace-owned classes during namespace
  traversal.
- Mermaid's Class CSS uses `mainBkg` for node fill and `nodeBorder` for node border. Rust's
  renderer now uses the same variables; this avoids structural stroke mismatches after theme
  expansion makes `primaryBorderColor` differ from `nodeBorder`.
- Mermaid 11.15's default Class renderer uses the shared `rendering-util` Dagre path, not the old
  class `dagre-wrapper` behavior. Its `copy(...)` traversal copies child clusters before their
  parent cluster node, and `recursiveRender(...)` applies `ranksep: parent.ranksep + 25`. Rust now
  mirrors those rules, keeps the source eligibility rule (`children && !externalConnections`), and
  moves any already-extracted child cluster under a later extracted parent.
- Mermaid's Class SVG title path wraps text inside `createText(...)` while the inner tspans are
  still normal weight, then `shapeUtil.ts` applies outer `font-weight: bolder` and removes the
  inner normal-weight overrides for the final bbox. Rust now wraps Class titles with normal-weight
  measurement and measures the final label with the existing bolder style.
- Mermaid's generated Class CSS preserves raw `themeVariables.fontSize` spelling. A numeric
  `fontSize: 24` becomes `font-size:24`, not `font-size:24px`; browser SVG text sizing does not
  treat that unitless CSS as a 24px font. Rust now emits the raw CSS spelling while keeping headless
  text measurement tied to explicit px strings. This deliberately avoids a false exactness fix that
  would make numeric theme values behave unlike browser SVG text.
- This task is not root-green. The remaining Class rows include real namespace/layout-root
  differences plus smaller text/root tails. The largest resolved rows are root-green, while
  `upstream_pkgtests_classdiagram_spec_003`, `upstream_html_demos_classchart_class_diagram_demos_010`,
  `upstream_cypress_classdiagram_v2_spec_renders_a_class_diagram_with_nested_namespaces_and_relationships_035`,
  and `upstream_html_demos_classchart_class_diagram_demos_011` are now `0.25px` root-width tails.
  M15RV-090 must not close by accepting this whole current Class residual set.

Fresh unaccepted residual summary after M15RV-060:

| Diagram | Unaccepted residuals | Report |
| --- | ---: | --- |
| Sequence | 167 | `target/compare/sequence_report_parity_root.md` |
| Flowchart | 61 | `target/compare/flowchart_report_parity_root.md` |
| Architecture | 32 | `target/compare/architecture_report_parity_root.md` |
| Class | 13 | `target/compare/class_report_parity_root.md` |
| Timeline | 3 | `target/compare/timeline_report_parity_root.md` |
| Journey | 2 | `target/compare/journey_report_parity_root.md` |

Total unaccepted residuals: 278.

Fresh follow-up evidence after the v3 node-order, title-wrap, and CSS font-size correction:

- `cargo test -p merman-render --test class_layout_test`: passed, 16 tests.
- `cargo test -p merman-render --test class_svg_test`: passed, 18 tests.
- `cargo run -p xtask -- compare-class-svgs --filter stress_class_nested_namespaces_cross_edges_008 --check-dom --dom-mode parity-root --dom-decimals 3 --out target/compare/class_cross_edges_after_v3_order.md`:
  passed after mirroring `ClassDB.getData()` namespace-first group-node ordering.
- `cargo run -p xtask -- compare-class-svgs --filter stress_class_svg_font_size_precedence_025 --check-dom --dom-mode parity-root --dom-decimals 3 --out target/compare/class_font_size_025_after_title_wrap_normal.md`:
  expected failure; the row is now a `0.25px` root-width tail (`348px` upstream vs `347.75px`
  local) after the title-wrap and raw-CSS fixes.
- `cargo run -p xtask -- compare-class-svgs --filter stress_class_svg_font_size_px_string_precedence_026 --check-dom --dom-mode parity-root --dom-decimals 3 --out target/compare/class_font_size_026_after_title_wrap_normal.md`:
  expected failure; explicit `24px` stays layout-effective and the row is now a `0.25px`
  root-width tail (`367.25px` upstream vs `367px` local).
- `cargo run -p xtask -- compare-class-svgs --check-dom --dom-mode parity --dom-decimals 3 --out target/compare/class_report_parity_final_m15rv060.md`:
  passed for the current Class matrix.
- `cargo run -p xtask -- compare-class-svgs --check-dom --dom-mode parity-root --dom-decimals 3 --out target/compare/class_report_parity_root_final_m15rv060.md`:
  expected failure with 14 raw Class root mismatches. Full-root policy accepts 2 existing Class
  policy rows, leaving 12 unaccepted Class rows.
- `cargo run -p xtask -- compare-all-svgs --check-dom --dom-mode parity --dom-decimals 3`:
  passed for the implemented matrix.
- `cargo run -p xtask -- compare-all-svgs --check-dom --dom-mode parity-root --dom-decimals 3`:
  expected failure with bounded summary and 277 unaccepted residuals.
- `cargo run -p xtask -- report-overrides --check-no-growth`: passed with root viewport overrides
  still at 278 entries and no manual raw SVG/path bridges.
- `cargo fmt --check`: passed.
- `git diff --check`: passed with the existing CRLF warning for this workstream's
  `CONTEXT.jsonl`.

Fresh unaccepted residual summary after the M15RV-060 follow-up:

| Diagram | Unaccepted residuals | Report |
| --- | ---: | --- |
| Sequence | 167 | `target/compare/sequence_report_parity_root.md` |
| Flowchart | 61 | `target/compare/flowchart_report_parity_root.md` |
| Architecture | 32 | `target/compare/architecture_report_parity_root.md` |
| Class | 12 | `target/compare/class_report_parity_root.md` |
| Timeline | 3 | `target/compare/timeline_report_parity_root.md` |
| Journey | 2 | `target/compare/journey_report_parity_root.md` |

Total unaccepted residuals: 277.

## M15RV-070 - Shared Font-Size CSS/Measurement Policy Helper

Fresh evidence from 2026-06-01:

- `cargo test -p merman-render config::tests`: passed, including shared config helper coverage.
- `cargo test -p merman-render --test class_svg_test class_svg_preserves_numeric_theme_font_size_css_spelling`:
  passed.
- `cargo run -p xtask -- compare-radar-svgs --filter stress_radar_font_size_precedence_001 --check-dom --dom-mode parity --dom-decimals 3 --out target/compare/radar_font_size_after_css_font_helper.md`:
  passed for the Radar px-string theme font-size fixture after migrating to the shared raw-CSS
  helper.
- `cargo run -p xtask -- compare-radar-svgs --filter upstream_radar_theme_override_colon_header_spec --check-dom --dom-mode parity --dom-decimals 3 --out target/compare/radar_numeric_font_size_after_css_font_helper.md`:
  passed for the Radar numeric theme font-size fixture; numeric values still emit unitless CSS like
  upstream Mermaid.
- `cargo run -p xtask -- compare-class-svgs --filter stress_class_svg_font_size_precedence_025 --check-dom --dom-mode parity --dom-decimals 3 --out target/compare/class_font_size_025_after_css_font_helper.md`:
  passed for the Class numeric theme font-size structural fixture.
- `cargo run -p xtask -- compare-class-svgs --filter stress_class_svg_font_size_px_string_precedence_026 --check-dom --dom-mode parity --dom-decimals 3 --out target/compare/class_font_size_026_after_css_font_helper.md`:
  passed for the Class explicit px-string theme font-size structural fixture.
- `cargo run -p xtask -- report-overrides --check-no-growth`: passed with root viewport overrides
  still at 278 entries and no manual raw SVG/path bridges.
- `cargo fmt --check`: passed.
- `git diff --check`: passed with the existing CRLF warning for this workstream's
  `CONTEXT.jsonl`.

Policy extraction:

- `config_css_number_or_string(...)` represents Mermaid stylesheet interpolation: string values are
  emitted as trimmed CSS values, and JSON numbers are formatted as numbers without adding `px`.
- `config_f64_explicit_css_px(...)` represents the headless measurement rule needed by Class SVG
  text: only explicit `px` strings are treated as browser-effective SVG text sizes. JSON numbers and
  unitless strings remain CSS-output facts, not measurement facts.
- Radar already had a local helper with the raw CSS interpolation behavior; it now uses the shared
  config helper. Other diagrams still need source checks before migration because several Mermaid
  paths use `parseFontSize(...)`, diagram-specific config fields, or explicit `.style('font-size',
  value + 'px')` rules.

Full residual counts are unchanged from M15RV-060: total 277 unaccepted root residuals, with Class
at 12.

## M15RV-080 - Sequence Text Measurement Policy Audit

Fresh evidence from 2026-06-02:

- `cargo test -p merman-render sequence_central_connection_rtl -- --nocapture`: passed, 2 focused
  Sequence SVG tests. These tests cover the central-connection RTL fixture's default layout actor
  centers (`Alice=75`, `Bob=443`, `Charlie=820`) and verify the SVG renderer preserves the first
  message line coordinates (`442 -> 83`) instead of introducing a render-stage drift.
- `cargo run -p xtask -- compare-sequence-svgs --filter upstream_cypress_sequencediagram_v2_spec_should_render_central_connection_with_normal_arrows_right_to_lef_033 --check-dom --dom-mode parity-root --dom-decimals 3 --report-root-all --out target/compare/sequence_central_rtl_033_after_measurement_audit.md`:
  expected failure; the focused row remains a root-only residual at `965px` upstream vs `1028px`
  local (`+63px`).
- `cargo run -p xtask -- compare-sequence-svgs --check-dom --dom-mode parity-root --dom-decimals 3 --report-root-all --out target/compare/sequence_report_parity_root_m15rv080_measurement_audit.md`:
  expected failure with 168 Sequence mismatch lines, matching the prior raw Sequence bucket and
  confirming the compare-path cleanup did not create a new aggregate Sequence drift.
- `cargo run -p xtask -- gen-upstream-svgs --diagram sequence --filter upstream_cypress_sequencediagram_v2_spec_should_render_central_connection_with_normal_arrows_right_to_lef_033`:
  produced no upstream SVG diff, so the `965px` baseline is not stale under the current Mermaid
  export path.

Source findings:

- Mermaid 11.15 Sequence spacing still routes message labels through
  `getMaxMessageWidthPerActor(...)` in
  `repo-ref/mermaid/packages/mermaid/src/diagrams/sequence/sequenceRenderer.ts`. That path applies
  `utils.wrapLabel(...)` when needed, calls `utils.calculateTextDimensions(wrappedMessage,
  textFont)`, adds `2 * conf.wrapPadding`, then feeds the width into `calculateActorMargins(...)`.
- Mermaid's `utils.calculateTextDimensions(...)` in
  `repo-ref/mermaid/packages/mermaid/src/utils.ts` is browser-SVG-dependent: it inserts temporary
  text, measures `getBBox()`, rounds width and height, probes both `sans-serif` and the configured
  font family, and chooses the configured family unless the sans-serif dimensions are strictly
  larger.
- Rust's central-connection layout/render constants were already aligned with Mermaid source. The
  remaining central-connection mismatch is therefore a text measurement/root-bounds policy gap, not
  a missing central-connection semantic rule.

Rejected probes:

- Replacing Sequence actor-spacing message measurement with `DeterministicTextMeasurer` improved
  the focused central RTL row from `1028px` to `995px`, but increased raw Sequence root mismatches
  from 168 to 169. This is not a safe fix because it trades one fixture for broader drift.
- Refreshing the full generated `svg_overrides_sequence_11_12_2.rs` table with
  `gen-svg-overrides --mode sequence` made the focused row worse (`1034px`) and produced a large
  generated-table churn. The current Sequence override generator's minimal-diagram inversion needs
  repair or replacement before a full refresh is defensible.

Fresh follow-up evidence after repairing the Sequence SVG override generator:

- `crates/xtask/src/cmd/overrides/svg.rs` now lets Sequence override generation follow the same
  default browser selection path as Mermaid CLI / Puppeteer. Passing an explicit system
  Chrome/Edge executable reproduced the bad `1034px` central row; leaving `executablePath`
  unset or using Puppeteer's bundled headless shell generated the expected central widths
  (`281px`, `292px`, `282px`, and `283px` for the RTL central variants).
- The generator now skips wrap-sensitive Sequence fixtures and skips raw message seeds that do not
  have actor endpoints. This avoids teaching final-layout SVG text overrides to incremental
  `wrapLabel(...)` probes and mirrors Mermaid's `getMaxMessageWidthPerActor(...)` input shape more
  closely.
- `crates/merman-render/src/text/measure.rs` now exposes a wrap-specific SVG bbox measurement seam.
  `VendoredFontMetricsTextMeasurer` uses that seam to keep exact final SVG text overrides out of
  wrap probing, while still using the generated table for final SVG text measurement.
- `crates/merman-render/src/svg/parity/sequence/block_text.rs` dropped the broad mid-width frame
  padding that falsely split `[Authentication check]`; the remaining narrow-frame pad is limited to
  frames at or below `160px`, where it preserves the observed Mermaid splits for nested-frame
  stress fixtures.
- `cargo run -p xtask -- report-overrides --check-no-growth`: passed with SVG text metric table
  rows increasing from 186 to 891 and the no-growth budget updated to 891.
- `cargo test -p merman-render sequence_central_connection_rtl -- --nocapture`: passed, 2 tests.
- `cargo run -p xtask -- compare-sequence-svgs --filter upstream_cypress_sequencediagram_v2_spec_should_render_central_connection_with_normal_arrows_right_to_lef_033 --check-dom --dom-mode parity-root --dom-decimals 3 --report-root-all --out target/compare/sequence_central_rtl_033_after_sequence_override_cleanup.md`:
  passed. The formerly `+63px` central RTL residual is now exact at `965px`.
- Focused Sequence structural checks passed for the wrap-sensitive regression rows:
  `upstream_cypress_sequencediagram_v2_spec_should_render_different_participant_types_with_notes_and_loops_015`,
  `stress_nested_frames_001`, and `stress_deep_nested_frames_018`.
- `cargo run -p xtask -- compare-sequence-svgs --check-dom --dom-mode parity --dom-decimals 3 --out target/compare/sequence_report_parity_after_sequence_override_cleanup.md`:
  passed for the full Sequence matrix.
- `cargo run -p xtask -- compare-sequence-svgs --check-dom --dom-mode parity-root --dom-decimals 3 --report-root-all --out target/compare/sequence_report_parity_root_after_sequence_override_cleanup.md`:
  expected failure with 68 raw Sequence root mismatches. Full `compare-all-svgs` accepts the
  existing `sequence/zed_pr_57644_sequence` policy row, leaving 67 unaccepted Sequence rows.
- Remaining Sequence root rows are now dominated by HTML `<br>` / wrap / note / participant height
  tails, for example `html_br_variants_and_wrap`,
  `stress_long_participant_labels_br_031`, `stress_br_in_messages_notes_011`, and
  `stress_sequence_batch5_wrap_html_br_spans_042`. These should be handled as M15RV-085, not by
  adding string-by-string browser constants.
- `cargo run -p xtask -- compare-all-svgs --check-dom --dom-mode parity --dom-decimals 3`:
  passed for the full implemented SVG matrix.
- `cargo run -p xtask -- compare-all-svgs --check-dom --dom-mode parity-root --dom-decimals 3`:
  expected failure with 175 unaccepted residuals: Flowchart 61, Sequence 67, Architecture 30,
  Class 12, Timeline 3, and Journey 2.

## M15RV-085 - Sequence HTML `<br>` / Wrap Evidence Follow-Up

Fresh source findings from 2026-06-02:

- Mermaid 11.15 defines `lineBreakRegex = /<br\s*\/?>/gi` in
  `repo-ref/mermaid/packages/mermaid/src/diagrams/common/common.ts`, so normal HTML `<br>`,
  `<br/>`, and `<br />` variants are explicit line breaks before generic word wrapping.
- `wrapLabel(...)` in `repo-ref/mermaid/packages/mermaid/src/utils.ts` short-circuits when the
  input contains `<br>`, otherwise it greedily wraps words using `calculateTextWidth(...)`.
- Sequence message spacing in
  `repo-ref/mermaid/packages/mermaid/src/diagrams/sequence/sequenceRenderer.ts` still computes
  widths from `wrapLabel(...)` plus browser-derived `calculateTextDimensions(...)`, while final
  rendering expands wrapped messages to at least Mermaid's configured width. Notes use the same
  `wrapLabel(...)` / `calculateTextDimensions(...)` policy when wrapping is enabled.

Generator and measurement changes:

- `crates/xtask/src/cmd/overrides/svg.rs` no longer skips whole Sequence wrap fixtures. It now
  collects final emitted SVG text samples from `actor`, `messageText`, and `noteText` nodes, so
  Mermaid's actual emitted labels can be used for final SVG text measurement.
- The raw model-derived extra seed path still skips wrapped actors/messages/boxes and messages
  without actor endpoints. That preserves the M15RV-080 separation between final SVG text evidence
  and incremental wrap probes.
- `crates/merman-render/src/text/wrap.rs` now has a guarded exact-single-line helper: exact final
  SVG evidence may suppress wrapping only when the full label fits the current wrap width with a
  small margin and the final exact width is materially narrower than the smooth wrap probe. This
  fixed false splits without turning exact final SVG rows into broad prefix-width constants.
- `crates/merman-render/src/generated/svg_overrides_sequence_11_12_2.rs` was regenerated from the
  upstream Sequence SVG corpus. The table grew from 891 to 1036 auditable rows, and
  `crates/xtask/src/cmd/overrides/report.rs` now records that budget explicitly.

Fresh validation:

- `cargo run -p xtask -- gen-svg-overrides --in fixtures\upstream-svgs\sequence --out crates\merman-render\src\generated\svg_overrides_sequence_11_12_2.rs --mode sequence`:
  passed; regenerated the Sequence SVG text table with final wrap-fixture text nodes included.
- `cargo test -p merman-render sequence_wrap_uses_exact_single_line_evidence_only_when_it_fits -- --nocapture`:
  passed.
- `cargo test -p merman-render sequence_svg_overrides_measure_final_simple_bbox_widths -- --nocapture`:
  passed.
- `cargo test -p merman-render sequence_svg_overrides_keep_literal_br_with_backslash_t_single_line -- --nocapture`:
  passed.
- Focused root compares passed for the four largest M15RV-085 rows:
  `stress_br_in_messages_notes_011`, `stress_long_participant_labels_br_031`,
  `stress_sequence_batch5_wrap_html_br_spans_042`, and `html_br_variants_and_wrap`.
- `cargo run -p xtask -- compare-sequence-svgs --check-dom --dom-mode parity --dom-decimals 3 --out target/compare/sequence_report_parity_after_m15rv085_wrap_samples.md`:
  passed for the full Sequence structural matrix.
- `cargo run -p xtask -- compare-sequence-svgs --check-dom --dom-mode parity-root --dom-decimals 3 --report-root-all --out target/compare/sequence_report_parity_root_after_m15rv085_wrap_samples.md`:
  expected failure with 64 raw Sequence root mismatches, down from 68 after M15RV-080.
- `cargo run -p xtask -- report-overrides --check-no-growth`: passed with the Sequence SVG text
  metric budget updated to 1036 rows.
- `cargo run -p xtask -- compare-all-svgs --check-dom --dom-mode parity --dom-decimals 3`:
  passed for the full implemented SVG matrix.
- `cargo run -p xtask -- compare-all-svgs --check-dom --dom-mode parity-root --dom-decimals 3`:
  expected failure with 171 unaccepted residuals: Flowchart 61, Sequence 63, Architecture 30,
  Class 12, Timeline 3, and Journey 2.

Outcome:

- The four largest Sequence HTML `<br>` / wrap rows are now root-exact.
- Raw Sequence root mismatches dropped from 68 to 64. In the full all-diagram policy run, the
  existing accepted `sequence/zed_pr_57644_sequence` row is still accepted, so Sequence has
  63 unaccepted residuals.
- Remaining Sequence rows are smaller and better classified: two long left-of note rows at `+7px`,
  several small line-break/text-measurement width tails, and participant/actor-type height-only
  tails. These are split into M15RV-087.

## M15RV-087 - Sequence Actor-Type And Stale Root Pin Follow-Up

Fresh source findings from 2026-06-02:

- Mermaid 11.15 `repo-ref/mermaid/packages/mermaid/src/diagrams/sequence/svgDraw.js` draws
  database participants with `rect.width / 3` cylinders and translates the cylinder by
  `(w, ry)`.
- The same source path draws boundary, control, and entity actor-man variants with 22px source
  radii and fixed source offsets. Rust previously carried stale approximations for several of
  those glyphs.
- Mermaid's `adjustCreatedDestroyedData(...)` positions created actor tops from
  `lineStartY - actor.height / 2` using the pre-render actor height. Type-specific SVG drawing can
  later update the rendered height, but that does not move the lifecycle anchor.
- Mermaid's footer actor draw pass bumps the shared bounds cursor by the maximum rendered footer
  actor height for the whole row. Individual destroyed actor `stopy` values do not replace that
  row cursor.

Renderer and root-policy changes:

- `crates/merman-render/src/svg/parity/sequence/actor_shapes.rs` now emits database actor geometry
  from the Mermaid 11.15 cylinder rule.
- `crates/merman-render/src/svg/parity/sequence/actor_man_glyphs.rs` now mirrors the source
  boundary/control/entity actor-man radii, transforms, circle positions, and text anchors.
- `crates/merman-render/src/sequence/actors.rs` now positions created top actors using the
  lifecycle height anchor instead of the type-specific visual height center.
- `crates/merman-render/src/sequence/root_bounds.rs` now includes Mermaid's footer-row
  max-height cursor bump in root bounds when `mirrorActors` is enabled.
- Six stale Sequence root viewport overrides were deleted after focused
  `--no-root-overrides` checks proved the computed roots were exact:
  `stress_quoted_participants_and_types_023`,
  `upstream_cypress_sequencediagram_spec_should_render_a_sequence_diagram_with_actor_creation_and_destruc_010`,
  `upstream_cypress_sequencediagram_v2_spec_should_render_different_participant_types_with_alternative_flows_016`,
  `upstream_cypress_sequencediagram_v2_spec_should_render_different_participant_types_with_notes_and_loops_015`,
  `upstream_cypress_sequencediagram_v2_spec_should_render_parallel_processes_with_different_participant_type_014`,
  and `upstream_cypress_sequencediagram_v2_spec_should_render_with_wrapped_messages_and_notes_011`.

Fresh validation:

- `cargo test -p merman-render sequence_text_and_frame_constants_match_mermaid -- --nocapture`:
  passed.
- `cargo run -p xtask -- compare-sequence-svgs --filter upstream_cypress_sequencediagram_v2_spec_should_render_participant_creation_and_destruction_with_differen_012 --check-dom --dom-mode parity-root --dom-decimals 3 --report-root-all --out target/compare/sequence_participant_creation_after_footer_cursor_rule.md`:
  passed after the lifecycle/footer cursor rule.
- Focused no-root stale-pin checks passed for
  `stress_quoted_participants_and_types_023`,
  `upstream_cypress_sequencediagram_v2_spec_should_render_parallel_processes_with_different_participant_type_014`,
  `upstream_cypress_sequencediagram_v2_spec_should_render_different_participant_types_with_alternative_flows_016`,
  `upstream_cypress_sequencediagram_v2_spec_should_render_different_participant_types_with_notes_and_loops_015`,
  `upstream_cypress_sequencediagram_v2_spec_should_render_with_wrapped_messages_and_notes_011`, and
  `upstream_cypress_sequencediagram_spec_should_render_a_sequence_diagram_with_actor_creation_and_destruc_010`.
- `cargo run -p xtask -- compare-sequence-svgs --check-dom --dom-mode parity --dom-decimals 3 --out target/compare/sequence_report_parity_after_m15rv087_stale_pin_drop.md`:
  passed for the full Sequence matrix.
- `cargo run -p xtask -- compare-sequence-svgs --check-dom --dom-mode parity-root --dom-decimals 3 --report-root-all --out target/compare/sequence_report_parity_root_after_m15rv087_stale_pin_drop.md`:
  expected failure with 28 raw Sequence root mismatches, down from 64 after M15RV-085.
- `cargo run -p xtask -- report-overrides --check-no-growth`: passed with Sequence root viewport
  overrides reduced from 55 to 49 entries and total root viewport overrides reduced to 272.
- `cargo run -p xtask -- compare-all-svgs --check-dom --dom-mode parity --dom-decimals 3`:
  passed for the full implemented SVG matrix.
- `cargo run -p xtask -- compare-all-svgs --check-dom --dom-mode parity-root --dom-decimals 3`:
  expected failure with 135 unaccepted residuals: Flowchart 61, Architecture 30, Sequence 27,
  Class 12, Timeline 3, and Journey 2.

Outcome:

- Sequence actor-type height rows and stale-root-pin rows are no longer part of the active
  residual bucket.
- Raw Sequence root mismatches dropped from 64 to 28. In the full all-diagram policy run, the
  existing accepted `sequence/zed_pr_57644_sequence` row is still accepted, so Sequence has
  27 unaccepted residuals.
- Remaining Sequence rows are long-note width tails, line-break/text-measurement width tails, and
  small math/root-measurement tails. They should not be forced with hand-written constants.

## M15RV-040 Follow-Up - Architecture Root Diagnostics Parity

Fresh evidence from 2026-06-01:

- `cargo run -p xtask -- compare-architecture-svgs --filter stress_architecture_batch5_services_outside_groups_crosslinks_078 --check-dom --dom-mode parity-root --dom-decimals 3 --report-root-all --no-root-overrides --out target/compare/architecture_crosslinks_no_overrides.md`:
  expected failure; confirms the new explicit Architecture CLI switch reaches the renderer and
  emits an all-row root delta table with root overrides disabled.
- `cargo run -p xtask -- compare-architecture-svgs --check-dom --dom-mode parity-root --dom-decimals 3 --report-root-all --out target/compare/architecture_report_parity_root_all.md`:
  expected failure with the existing 32 raw Architecture root mismatches, now accompanied by a full
  `Root Viewport Deltas` section for Architecture itself.
- `cargo run -p xtask -- compare-all-svgs --diagram architecture --check-dom --dom-mode parity-root --dom-decimals 3 --report-root-all`:
  expected failure with a bounded summary pointing at
  `target/compare/architecture_report_parity_root.md`; this proves `compare-all-svgs` now routes
  Architecture through the same root-report path as Flowchart, Sequence, State, GitGraph, and
  Mindmap.
- `cargo test -p xtask root_parity_failure_summary_keeps_final_error_bounded -- --nocapture`:
  passed.
- `cargo test -p xtask parses_root_report_limits -- --nocapture`:
  passed.

Architecture diagnostics findings:

- Before this follow-up, Architecture parity-root diagnosis was weaker than other root-heavy
  diagrams: `compare-architecture-svgs` lacked `--report-root*` support and did not expose an
  explicit `--no-root-overrides` switch even though the lane was already reasoning about pinned vs
  unpinned Architecture residuals.
- Architecture now respects `SvgRenderOptions.apply_root_overrides` in the final root viewport
  emission path instead of relying only on the process-wide
  `MERMAN_DISABLE_ROOT_VIEWPORT_OVERRIDES` environment variable.
- This is a diagnostics/deepening change, not a renderer-parity claim. The raw Architecture root
  bucket remains `32`, and the leading rows are unchanged large layout-root differences such as
  disconnected components, mixed service forms, and grouped port cases.
- The immediate benefit is traceability: Architecture root residuals now produce the same sortable
  `max-width/viewBox` evidence table as the other audited diagrams, which makes later measurement
  policy extraction or source-rule work less guessy.

Fresh follow-up evidence from 2026-06-02:

- `cargo run -p xtask -- compare-architecture-svgs --filter upstream_architecture_docs_service_icon_text --check-dom --dom-mode parity-root --dom-decimals 3 --report-root-all --out target/compare/architecture_docs_service_icon_text_fresh.md`:
  initially failed with a `+120.204px` root width delta (`343.884px` upstream vs `464.089px`
  local), which was too large to be a plausible headless text-measurement tail for a three-service
  docs example.
- `cargo run -p xtask -- gen-upstream-svgs --diagram architecture --filter upstream_architecture_docs_service_icon_text`:
  refreshed the pinned Mermaid baseline for that single Architecture fixture.
- Fresh file inspection of
  `fixtures/upstream-svgs/architecture/upstream_architecture_docs_service_icon_text.svg`
  now shows Mermaid's current baseline root at `453.9440612792969px` instead of the stale
  `343.88421630859375px`.
- `cargo run -p xtask -- compare-all-svgs --diagram architecture --check-dom --dom-mode parity-root --dom-decimals 3 --report-root-all`:
  still fails at 32 unaccepted Architecture residuals, but the refreshed
  `target/compare/architecture_report_parity_root.md` now reports
  `upstream_architecture_docs_service_icon_text` as a much smaller `+10.145px` residual
  (`453.944px` upstream vs `464.089px` local).

Architecture stale-baseline finding:

- `upstream_architecture_docs_service_icon_text` was not a renderer-side `+120px` bug. The large
  delta came from a stale pinned upstream SVG baseline that predated the current Mermaid 11.15
  Architecture iconText output.
- After refreshing only that upstream SVG, the residual collapsed into a smaller iconText
  `foreignObject` / root-bounds tail. This is still a real parity gap, but it is now in the right
  category: browser-driven iconText bbox approximation, not a gross Architecture layout failure.
- Because Mermaid's `svgDraw.ts` computes Architecture service size from
  `serviceElem.node().getBBox()` after inserting `foreignObject` HTML, any future fix here should
  be treated as an explicit headless approximation problem and justified with iconText-specific
  probe evidence rather than with broad Architecture layout changes.

Fresh follow-up evidence from 2026-06-02 for source-owned Architecture calibration cleanup:

- `cargo run -p xtask -- compare-architecture-svgs --filter upstream_architecture_cypress_reasonable_height --check-dom --dom-mode parity-root --dom-decimals 3 --report-root-all --no-root-overrides --out target/compare/architecture_reasonable_height_no_overrides.md`:
  expected failure before the cleanup, and it matched the enabled-root output exactly
  (`1859.75px` upstream vs `1860.25px` local). This proved the residual came from computed local
  geometry, not from a retained root pin.
- `cargo run -p xtask -- compare-architecture-svgs --filter upstream_architecture_cypress_reasonable_height --check-dom --dom-mode parity-root --dom-decimals 3 --report-root-all --out target/compare/architecture_reasonable_height_enabled.md`:
  same pre-cleanup `+0.380px` residual as the no-root-overrides run.
- Removed the local `is_reasonable_height_profile(...)` width calibration
  (`vb_w += 0.380126953125`) from `crates/merman-render/src/svg/parity/architecture/viewport.rs`.
- After the cleanup, these three focused gates all passed with exact zero root delta:
  `target/compare/architecture_reasonable_height_after_drop.md`,
  `target/compare/architecture_layout_reasonable_height_after_drop.md`, and
  `target/compare/architecture_reasonable_height_spec_after_drop.md`.
- `cargo run -p xtask -- compare-all-svgs --diagram architecture --check-dom --dom-mode parity-root --dom-decimals 3 --report-root-all`:
  still fails overall, but the bounded summary dropped from 32 to 29 unaccepted Architecture
  residuals.

Architecture calibration cleanup finding:

- The old `reasonable_height` width bump was no longer paying its way under Mermaid 11.15.
  It created a deterministic `+0.380px` overshoot on all three corresponding upstream fixtures.
- Removing it is a true source-owned cleanup, not a tolerance change: no browser-only approximation
  was introduced, and the affected fixtures became exact root matches.

Fresh structural-hygiene evidence from 2026-06-02:

- After refreshing `upstream_architecture_docs_service_icon_text`, the full structural gate exposed
  Architecture fixture-corpus inconsistency rather than a renderer layout defect: most Architecture
  fixtures still use bare `service-*` / `node-*` / `group-*` IDs and the older fallback background
  path spelling, while the refreshed fixture follows Mermaid 11.15's scoped IDs and absolute
  fallback path from `repo-ref/mermaid/packages/mermaid/src/diagrams/architecture/svgDraw.ts`.
- The renderer now emits the Mermaid 11.15 absolute fallback background path for services with no
  icon or iconText. The emitted-bounds tests were updated to the 11.15 `80x80` bbox.
- `crates/xtask/src/svgdom.rs` normalizes Architecture diagram-scoped service/node/group IDs and
  fallback service background path spelling in parity modes only. This keeps structural gates from
  encoding stale fixture-generation details while root gates still compare the actual root
  viewport output.
- `cargo test -p xtask parity_normalizes_architecture -- --nocapture`: passed, 2 tests.
- `cargo test -p merman-render svg_path_bounds_architecture_service_node_bkg_matches_mermaid_bbox -- --nocapture`:
  passed.
- `cargo test -p merman-render svg_emitted_bounds_attr_lookup_d_does_not_match_id -- --nocapture`:
  passed.
- `cargo run -p xtask -- compare-architecture-svgs --check-dom --dom-mode parity --dom-decimals 3 --out target/compare/architecture_report_parity_after_path_and_id_normalization.md`:
  passed.
- `cargo run -p xtask -- compare-all-svgs --check-dom --dom-mode parity --dom-decimals 3`:
  passed for the full implemented SVG matrix.

## M15RV-088 - Architecture 11.15 Baseline And Scoped IDs

Fresh source and fixture evidence from 2026-06-02:

- `repo-ref/mermaid/packages/mermaid/package.json` is Mermaid `11.15.0`; the local reference repo
  is at revision `9bae92cd3`.
- `repo-ref/mermaid/packages/mermaid/src/diagrams/architecture/svgDraw.ts` is the authority for
  the emitted Architecture DOM IDs and fallback service background path:
  `drawEdges(...)` writes edge path IDs as
  `` `${diagramId}-${getEdgeId(source, target, { prefix: 'L' })}` ``,
  `drawServices(...)` writes service group IDs as `` `${diagramId}-service-${service.id}` `` and
  fallback path IDs as `` `${diagramId}-node-${service.id}` ``, `drawJunctions(...)` writes
  junction rect IDs as `` `${diagramId}-node-${junction.id}` ``, and `drawGroups(...)` writes
  group rect IDs as `` `${diagramId}-group-${data.id}` ``.
- The same source path uses the current fallback service background path
  `` M0,${iconSize} V5 Q0,0 5,0 H${iconSize - 5} Q${iconSize},0 ${iconSize},5 V${iconSize} Z ``.
  This is source-owned Architecture SVG syntax, not a comparator tolerance.
- A focused fresh upstream generation for `upstream_architecture_simple_service_spec` produced a
  `160x160` root and the current scoped IDs/path, while the stored fixture still had the older
  `170x165` root and bare IDs. That proved the apparent simple-service delta was stale upstream
  fixture data rather than a Rust layout defect.
- `cargo run -p xtask -- gen-upstream-svgs --diagram architecture --out target/upstream-svgs-11-15-architecture-m15rv088-full`:
  regenerated all 185 Architecture upstream SVGs from the current Mermaid 11.15 reference.

Renderer and baseline changes:

- `crates/merman-render/src/svg/parity/architecture/edges.rs` now carries `diagram_id` into the
  edge renderer and prefixes Architecture edge path IDs the same way as Mermaid `svgDraw.ts`.
- `crates/merman-render/src/svg/parity/architecture/nodes.rs` now carries `diagram_id` into the
  node renderer and prefixes service, fallback-node, junction, and group IDs the same way as
  Mermaid `svgDraw.ts`.
- The stored `fixtures/upstream-svgs/architecture` corpus was refreshed from the fresh Mermaid
  11.15 output for all 185 Architecture fixtures. This removes mixed fixture vintages from the
  Architecture baseline instead of relying on old bare-ID/path variants.
- `crates/merman-render/src/generated/architecture_root_overrides_11_12_2.rs` no longer contains
  Architecture root viewport entries. The legacy module filename remains, but the lookup now
  intentionally returns `None` for Architecture.
- `crates/merman-render/src/svg/parity/architecture/viewport.rs` no longer applies the old
  groups-within-groups root viewport calibration. Fresh 11.15 root evidence showed that local
  calibration introduced deterministic errors; removing it made the affected rows exact to the
  configured root comparison precision.

Fresh validation:

- `cargo test -p merman-render architecture_diagonal_arrows_follow_the_actual_edge_segment -- --nocapture`:
  passed.
- `cargo test -p merman-render architecture_text_constants_match_mermaid -- --nocapture`: passed.
- `cargo run -p xtask -- compare-svg-xml --check --diagram architecture --upstream-root target/upstream-svgs-11-15-architecture-m15rv088-full --dom-mode parity --dom-decimals 3`:
  passed after the scoped ID changes.
- `cargo run -p xtask -- compare-architecture-svgs --check-dom --dom-mode parity --dom-decimals 3 --out target/compare/architecture_report_parity_after_m15rv088_cleanup.md`:
  passed.
- `cargo run -p xtask -- compare-architecture-svgs --check-dom --dom-mode parity-root --dom-decimals 3 --report-root-all --out target/compare/architecture_report_parity_root_after_m15rv088_cleanup.md`:
  expected failure with 32 Architecture root residuals.
- `cargo run -p xtask -- report-overrides --check-no-growth`: passed. Architecture root overrides
  are now `0`, and total root viewport override entries dropped to `241`.
- `cargo fmt -p merman-render --check`: passed.
- `cargo run -p xtask -- compare-all-svgs --check-dom --dom-mode parity --dom-decimals 3`:
  passed for the full implemented SVG matrix.
- `cargo run -p xtask -- compare-all-svgs --check-dom --dom-mode parity-root --dom-decimals 3`:
  expected failure with 137 unaccepted residuals: Flowchart 61, Architecture 32, Sequence 27,
  Class 12, Timeline 3, and Journey 2.

Outcome:

- Architecture structural parity is green against a fully refreshed Mermaid 11.15 Architecture
  fixture corpus.
- The Architecture root override table is intentionally empty. The previous 31 entries were stale
  Mermaid 11.12-era pins under the current 11.15 baseline.
- The Architecture root count moved from the pre-refresh 30-row summary to 32 honest rows. This is
  not a renderer regression; it removes stale fixture/pin/calibration noise and exposes the
  current 11.15 FCoSE/group-port root-bound residuals.
- The largest remaining Architecture rows are
  `stress_architecture_junction_fork_join_026` at about `-1551px`,
  `stress_architecture_fan_in_out_021` at about `-108px`,
  `stress_architecture_deep_nesting_013` at about `+106px`, and
  `stress_architecture_batch6_junctions_multi_split_with_group_edges_087` at about `+89px`.
  These should be investigated as source/layout/root-bound issues before any diagnostic residual
  policy is considered.

## M15RV-089 - Architecture Junction Parent Source Rule

Fresh source evidence from 2026-06-02:

- `repo-ref/mermaid/packages/mermaid/src/diagrams/architecture/architectureRenderer.ts`
  `addJunctions(...)` adds Cytoscape junction nodes with `parent: junction.in`.
- The same source uses `idealEdgeLength(...)` and `edgeElasticity(...)` callbacks that switch on
  whether `nodeData(nodeA).parent === nodeData(nodeB).parent`. A wrong junction parent therefore
  changes the FCoSE spring model, not just a final SVG attribute.
- Rust previously inferred a junction's group from neighboring non-junction services when
  `junction.in` was absent. Mermaid 11.15 does not do this. In
  `stress_architecture_junction_fork_join_026`, this put `fork` inside `left`, turning the
  `fork -> auth` edge into a same-parent strong spring and collapsing the expected wide layout.

Renderer change:

- `crates/merman-render/src/architecture.rs` no longer infers group parents for ungrouped
  Architecture junctions. Junction parentage now comes only from the parsed/render model, matching
  Mermaid's `junction.in` source rule.

Fresh validation:

- `cargo run -p xtask -- compare-architecture-svgs --filter stress_architecture_junction_fork_join_026 --check-dom --dom-mode parity-root --dom-decimals 3 --report-root-all --out target/compare/architecture_junction_fork_join_after_junction_parent_source_rule.md`:
  expected failure, but the row improved from about `-1551px` to about `+14px`
  (`2808.127px` upstream vs `2822.102px` local).
- `cargo run -p xtask -- compare-architecture-svgs --check-dom --dom-mode parity --dom-decimals 3 --out target/compare/architecture_report_parity_after_m15rv089_junction_parent_source_rule.md`:
  passed for the full Architecture matrix.
- `cargo run -p xtask -- compare-architecture-svgs --check-dom --dom-mode parity-root --dom-decimals 3 --report-root-all --out target/compare/architecture_report_parity_root_after_m15rv089_junction_parent_source_rule.md`:
  expected failure with 30 Architecture root residuals, down from 32.
- `cargo run -p xtask -- compare-all-svgs --check-dom --dom-mode parity --dom-decimals 3`:
  passed for the full implemented SVG matrix.
- `cargo run -p xtask -- compare-all-svgs --check-dom --dom-mode parity-root --dom-decimals 3`:
  expected failure with 135 unaccepted residuals: Flowchart 61, Architecture 30, Sequence 27,
  Class 12, Timeline 3, and Journey 2.
- `cargo run -p xtask -- report-overrides --check-no-growth`: passed; Architecture root overrides
  remain `0`.
- `cargo fmt -p merman-render --check`: passed.

Outcome:

- `stress_architecture_fan_in_out_021` and
  `stress_architecture_batch6_junctions_multi_split_with_group_edges_087` are now root-exact.
- `stress_architecture_junction_fork_join_026` is no longer a large FCoSE collapse; its remaining
  root delta is about `+14px` and should be treated separately from the removed parent-inference
  bug.
- The largest remaining Architecture row is now `stress_architecture_deep_nesting_013` at about
  `+106px`. Continue source investigation there before considering residual policy.

## M15RV-089 - Architecture Group Alignment Source Traversal

Fresh source and probe evidence from 2026-06-02:

- `repo-ref/mermaid/packages/mermaid/src/diagrams/architecture/architectureDb.ts`
  `getDataStructures()` updates `groupAlignments` while reducing `this.nodes` and each node's
  `service.edges` list. This means the same edge can update the group alignment map once per
  endpoint, and later endpoint traversal overwrites earlier values for the same group pair.
- A focused browser probe for `stress_architecture_deep_nesting_013` showed Mermaid's effective
  constraints as `horizontal=[[lb, api]]` and `vertical=[[lb, ext], [api, cache]]`.
- Rust had collapsed `groupAlignments` to a single global edge pass. For the same fixture, the
  focused debug output before the fix produced `horizontal=[[0, 1, 1, 3]]` and
  `vertical=[[0, 5]]`, incorrectly preserving the `core`/`data` alignment as horizontal.

Renderer and test changes:

- `crates/merman-render/src/architecture.rs` now builds Architecture `group_alignments` by
  walking `node_order`, then each node's incident edge list, matching Mermaid's
  `this.nodes -> service.edges` traversal and overwrite behavior.
- `crates/merman-render/tests/architecture_svg_test.rs` now has a regression test for
  `stress_architecture_deep_nesting_013` that asserts the source-derived visible alignments:
  `lb/api` share a row, `lb/ext` share a column, and `api/cache` share a column.

Fresh validation:

- `cargo run -p xtask -- compare-architecture-svgs --filter stress_architecture_deep_nesting_013 --check-dom --dom-mode parity-root --dom-decimals 3 --report-root-all --out target/compare/architecture_deep_nesting_m15rv089_group_alignment_source_order_debug.md`:
  passed; debug constraints are now `horizontal=[[0, 1]]`,
  `vertical=[[0, 5], [1, 4]]`, and the fixture is root-exact.
- `cargo run -p xtask -- compare-architecture-svgs --check-dom --dom-mode parity --dom-decimals 3 --out target/compare/architecture_report_parity_after_m15rv089_group_alignment_source_order.md`:
  passed for the full Architecture matrix.
- `cargo run -p xtask -- compare-architecture-svgs --check-dom --dom-mode parity-root --dom-decimals 3 --report-root-all --out target/compare/architecture_report_parity_root_after_m15rv089_group_alignment_source_order.md`:
  expected failure with 29 Architecture root residuals, down from 30 after the junction-parent
  source rule.
- `cargo test -p merman-render --test architecture_svg_test -- --nocapture`: passed, 2 tests.
- `cargo test -p merman-render --test architecture_layout_test -- --nocapture`: passed, 5 tests.
- `cargo run -p xtask -- compare-all-svgs --check-dom --dom-mode parity --dom-decimals 3`:
  passed for the full implemented SVG matrix.
- `cargo run -p xtask -- compare-all-svgs --check-dom --dom-mode parity-root --dom-decimals 3`:
  expected failure with 134 unaccepted residuals: Flowchart 61, Architecture 29, Sequence 27,
  Class 12, Timeline 3, and Journey 2.
- `cargo run -p xtask -- report-overrides --check-no-growth`: passed; Architecture root overrides
  remain `0`.
- `cargo fmt -p merman-render --check`: passed.
- `git diff --check`: passed.

Outcome:

- `stress_architecture_deep_nesting_013` is root-exact after matching Mermaid's source traversal
  order for group alignment overwrites.
- M15RV-089 has now reduced Architecture root residuals from 32 to 29 without adding root pins,
  tolerances, or browser-dependent font constants.
- The largest remaining Architecture row is now
  `stress_architecture_batch6_init_fontsize_icon_size_wrap_093` at about `-22.5px`; investigate it
  as an iconSize/fontSize/wrap/root-bounds issue before considering residual policy.

## M15RV-089 - Architecture Group Padding And Canvas Label Metrics

Fresh source and probe evidence from 2026-06-02:

- `repo-ref/mermaid/packages/mermaid/src/diagrams/architecture/architectureRenderer.ts` styles
  `.node-group` with `padding: ${db.getConfigField('padding')}px`; the group padding source is the
  configured Architecture `padding`, not `iconSize / 2`.
- `repo-ref/mermaid/packages/mermaid/src/diagrams/architecture/svgDraw.ts` `drawGroups(...)`
  reads the Cytoscape `node.boundingBox()` result and emits the group rect from
  `x1 + iconSize / 2`, `y1 + iconSize / 2`, `width = w`, and `height = h`. The browser/Cytoscape
  bbox still owns the final 0.5-2.5px lattice tail, but the input style padding is source-owned.
- The diagnostic browser probe
  `tools/debug/arch_fcose_browser_probe_fixture_025.js` now parses fixture `%%{init: ...}%%`
  directives before calling `mermaid.initialize(...)` and reports the effective Architecture
  `iconSize`, `fontSize`, and `padding`. The previous probe accidentally measured default
  `iconSize=80` for custom-init fixtures, which made the `padding` question look less clear than
  it was.
- Focused browser probe evidence for
  `stress_architecture_batch6_init_fontsize_icon_size_wrap_093` now reports config
  `{ iconSize: 40, fontSize: 18, padding: 30 }`, service pre-layout bboxes of about
  `API Service=101x62`, `Database=84x62`, and `Logs=42x62`, and group pre-layout bboxes of about
  `left=162x124` and `right=145x124`.

Renderer and test changes:

- `crates/merman-render/src/architecture.rs` now exposes
  `architecture_cytoscape_canvas_label_metrics(...)` so the layout and SVG root-bound code share
  the same deterministic Cytoscape canvas-label width approximation instead of carrying duplicate
  inline scale/rounding logic.
- `crates/merman-render/src/svg/parity/architecture/geometry.rs` now sizes group rectangles from
  `architecture.padding + 2.5` instead of the legacy `iconSize / 2 + 2.5` proxy. This preserves the
  existing headless bbox tail while removing the source-inaccurate `iconSize` dependency.
- `crates/merman-render/tests/architecture_svg_test.rs` now guards custom-padding behavior with
  `architecture_group_rect_uses_configured_padding_for_small_icons`, asserting that the custom
  `padding=30`, `iconSize=40` fixture does not regress to the old `iconSize / 2` group width.

Fresh validation:

- `cargo test -p merman-render --test architecture_svg_test -- --nocapture`: passed, 3 tests.
- `cargo test -p merman-render --test architecture_layout_test -- --nocapture`: passed, 5 tests.
- `cargo run -p xtask -- compare-architecture-svgs --filter stress_architecture_batch6_init_fontsize_icon_size_wrap_093 --check-dom --dom-mode parity-root --dom-decimals 3 --report-root-all --out target/compare/architecture_batch6_init_fontsize_icon_size_wrap_m15rv089_padding_metric_refactor_only.md`:
  expected failure, but the focused row improved from about `-22.5px` to about `-2.5px`
  (`325.105px` upstream vs `322.605px` local). Height is effectively aligned
  (`380.479px` upstream vs `380.604px` local).
- `cargo run -p xtask -- compare-architecture-svgs --check-dom --dom-mode parity-root --dom-decimals 3 --report-root-all --out target/compare/architecture_report_parity_root_after_m15rv089_group_padding_metric_refactor_only.md`:
  expected failure with 29 Architecture root residuals and the same failure set as the previous
  group-alignment report.
- `cargo run -p xtask -- compare-architecture-svgs --check-dom --dom-mode parity --dom-decimals 3 --out target/compare/architecture_report_parity_after_m15rv089_group_padding_metric_refactor_only.md`:
  passed for the full Architecture matrix.
- `cargo run -p xtask -- compare-all-svgs --check-dom --dom-mode parity --dom-decimals 3`:
  passed for the full implemented SVG matrix.
- `cargo run -p xtask -- compare-all-svgs --check-dom --dom-mode parity-root --dom-decimals 3`:
  expected failure with 134 unaccepted residuals: Flowchart 61, Architecture 29, Sequence 27,
  Class 12, Timeline 3, and Journey 2.
- `cargo run -p xtask -- report-overrides --check-no-growth`: passed; Architecture root overrides
  remain `0`, total root viewport overrides remain `241`.
- `cargo fmt -p merman-render --check`: passed.
- `git diff --check`: passed with only the existing LF/CRLF warnings for workstream JSONL files.

Outcome:

- The custom-init padding row is no longer a `-22.5px` source-rule defect; it is a remaining
  `-2.5px` browser/Cytoscape bbox quantization and headless text-measurement tail.
- An attempted `+1px` browser-bbox edge adjustment was rejected because it increased the
  Architecture root mismatch count from 29 to 31 by adding two new residuals. Do not reintroduce
  that kind of exact-browser tweak without broad generated evidence.
- Architecture remains at 29 root residuals with no root pins, no tolerances, and no
  browser-dependent renderer path.

## M15RV-089 - Architecture Relative Constraint Duplicate BFS Source Order

Fresh source and probe evidence from 2026-06-02:

- `repo-ref/mermaid/packages/mermaid/src/diagrams/architecture/architectureRenderer.ts`
  `getRelativeConstraints(...)` performs BFS over the spatial map by assigning
  `visited[curr] = 1` on pop, but it does not skip an already-queued duplicate current position.
  It only checks `!visited[newPos]` before pushing a neighbor and emitting a relative placement
  constraint.
- The fixed browser probe for `stress_architecture_junction_fork_join_026` reports 9 relative
  placement constraints. The duplicated queued `join` position emits `join -> db` and
  `join -> cache` twice before those neighbor positions are visited.
- Rust previously used `if !visited_pos.insert(curr) { continue; }`, which skipped duplicate
  queued positions on pop and emitted only 7 relative placement constraints for the same fixture.

Renderer and test changes:

- `crates/merman-render/src/architecture.rs` now extracts
  `architecture_relative_placement_constraints(...)` and preserves Mermaid's duplicate-pop BFS
  behavior.
- A focused unit test,
  `architecture_relative_constraints_preserve_mermaid_duplicate_bfs_pops`, constructs the
  fork/join diamond grid and asserts that `join -> db` and `join -> cache` each appear twice.

Fresh validation:

- `cargo test -p merman-render architecture_relative_constraints_preserve_mermaid_duplicate_bfs_pops -- --nocapture`:
  passed.
- `cargo test -p merman-render --test architecture_layout_test -- --nocapture`: passed, 5 tests.
- `cargo test -p merman-render --test architecture_svg_test -- --nocapture`: passed, 3 tests.
- `cargo run -p xtask -- compare-architecture-svgs --filter stress_architecture_junction_fork_join_026 --check-dom --dom-mode parity-root --dom-decimals 3 --report-root-all --out target/compare/architecture_junction_fork_join_after_relative_bfs_duplicate_source_order.md`:
  expected failure; the row remains about `+13.976px`, but Rust now feeds the same duplicate
  relative constraints that Mermaid's source/probe emits.
- `cargo run -p xtask -- compare-architecture-svgs --check-dom --dom-mode parity --dom-decimals 3 --out target/compare/architecture_report_parity_after_m15rv089_relative_bfs_duplicate_source_order.md`:
  passed for the full Architecture matrix.
- `cargo run -p xtask -- compare-architecture-svgs --check-dom --dom-mode parity-root --dom-decimals 3 --report-root-all --out target/compare/architecture_report_parity_root_after_m15rv089_relative_bfs_duplicate_source_order.md`:
  expected failure with 29 Architecture root residuals.
- `cargo run -p xtask -- compare-all-svgs --check-dom --dom-mode parity --dom-decimals 3`:
  passed for the full implemented SVG matrix.
- `cargo run -p xtask -- compare-all-svgs --check-dom --dom-mode parity-root --dom-decimals 3`:
  expected failure with 134 unaccepted residuals: Flowchart 61, Architecture 29, Sequence 27,
  Class 12, Timeline 3, and Journey 2.
- `cargo run -p xtask -- report-overrides --check-no-growth`: passed; Architecture root overrides
  remain `0`, total root viewport overrides remain `241`.
- `cargo fmt -p merman-render --check`: passed.
- `git diff --check`: passed with only the existing LF/CRLF warnings for workstream JSONL files.

Outcome:

- Architecture FCoSE relative-placement input now matches Mermaid's duplicate queued-position BFS
  behavior for the fork/join residual. This is a source-consistency fix, not a viewport-count
  reduction.
- `stress_architecture_junction_fork_join_026` remains the largest Architecture row at about
  `+13.976px`; treat its remaining delta as solver/headless layout drift unless a new source rule
  is found.

## M15RV-089 - Architecture Pre-Layout Group Padding Source Rule

Fresh source evidence from 2026-06-02:

- `repo-ref/mermaid/packages/mermaid/src/diagrams/architecture/architectureRenderer.ts` styles
  `.node-group` with `padding: ${db.getConfigField('padding')}px` before FCoSE runs.
- Rust had already switched final SVG group rectangle sizing to `architecture.padding + 2.5`, but
  the pre-layout compound bbox used for the FCoSE relocation/input approximation still used the old
  default-only proxy `iconSize / 2 + 2.5`.

Renderer change:

- `crates/merman-render/src/architecture.rs` now uses `padding_px + 2.5` for pre-layout group
  bbox inflation as well as final SVG group rectangle sizing. This removes a source-inconsistent
  split between layout input and SVG root-bound input.

Fresh validation:

- `cargo run -p xtask -- compare-architecture-svgs --filter stress_architecture_batch6_init_fontsize_icon_size_wrap_093 --check-dom --dom-mode parity-root --dom-decimals 3 --report-root-all --out target/compare/architecture_batch6_init_fontsize_icon_size_wrap_m15rv089_pre_layout_padding.md`:
  expected failure; the row remains about `-2.5px`.
- `cargo run -p xtask -- compare-architecture-svgs --filter stress_architecture_batch4_init_small_icons_061 --check-dom --dom-mode parity-root --dom-decimals 3 --report-root-all --out target/compare/architecture_batch4_init_small_icons_m15rv089_pre_layout_padding.md`:
  expected failure; the row remains about `-9.288px`.
- `cargo test -p merman-render --test architecture_layout_test -- --nocapture`: passed, 5 tests.
- `cargo test -p merman-render architecture_relative_constraints_preserve_mermaid_duplicate_bfs_pops -- --nocapture`:
  passed.
- `cargo run -p xtask -- compare-architecture-svgs --check-dom --dom-mode parity --dom-decimals 3 --out target/compare/architecture_report_parity_after_m15rv089_pre_layout_padding.md`:
  passed for the full Architecture matrix.
- `cargo run -p xtask -- compare-architecture-svgs --check-dom --dom-mode parity-root --dom-decimals 3 --report-root-all --out target/compare/architecture_report_parity_root_after_m15rv089_pre_layout_padding.md`:
  expected failure with 29 Architecture root residuals.
- `cargo run -p xtask -- compare-all-svgs --check-dom --dom-mode parity --dom-decimals 3`:
  passed for the full implemented SVG matrix.
- `cargo run -p xtask -- compare-all-svgs --check-dom --dom-mode parity-root --dom-decimals 3`:
  expected failure with 134 unaccepted residuals: Flowchart 61, Architecture 29, Sequence 27,
  Class 12, Timeline 3, and Journey 2.
- `cargo run -p xtask -- report-overrides --check-no-growth`: passed; Architecture root overrides
  remain `0`, total root viewport overrides remain `241`.
- `cargo fmt -p merman-render --check`: passed.
- `git diff --check`: passed with only the existing LF/CRLF warnings for workstream JSONL files.

Outcome:

- Layout-side and SVG-side Architecture group padding now use the same source-derived config field.
- This does not reduce the current Architecture residual count; remaining custom-padding rows are
  now dominated by canvas label / Cytoscape bbox measurement tails rather than the old
  `iconSize / 2` padding proxy.

## M15RV-089 - Architecture Canvas Label Residual Diagnostics

Fresh diagnostic evidence from 2026-06-02:

- `node tools/debug/arch_fcose_browser_probe_fixture_025.js stress_architecture_batch5_long_titles_and_punct_076`
  reports browser pre-layout service bboxes of `Runner Linux amd64=155x100`,
  `Container Registry=139x100`, `Artifacts Storage retention 30d=223x100`, and
  `Production=83x100`. The upstream SVG places the group rect at about
  `x=-233.463,width=462.926`; local output is `x=-244.463,width=472.926`.
- Local `MERMAN_ARCH_DEBUG_CY_BBOX=1` for the same fixture shows the shared canvas-label helper
  estimating `Artifacts Storage retention 30d` at `half_w=118.5`, while Chromium/Cytoscape's
  browser bbox is about `111.5` half-width. That over-wide long label explains most of the
  `+10px` root-width tail.
- The same debug output shows shorter labels need different behavior: `Runner Linux amd64` and
  `Container Registry` are closer to the current scaled estimate, while `Production` is dominated
  by the icon/border floor. A single global scale tweak would improve one row and risk regressing
  others.
- `node tools/debug/arch_fcose_browser_probe_fixture_025.js stress_architecture_batch4_init_small_icons_061`
  reports effective config `{ iconSize: 40, fontSize: 12, padding: 10 }`, browser service bboxes
  of `42x56`, group pre-layout bbox `65x78`, and final positions
  `a=(-38.036,59.786)`, `b=(55.536,-33.786)`, `c=(55.536,59.786)`.
- Local debug for `stress_architecture_batch4_init_small_icons_061` is icon-floor dominated
  (`half_w=21`, `bottom=14` for labels `A`, `B`, and `C`), so the remaining `-9.288px` root tail is
  not the same long-label over-scale issue as the batch5 row.

Outcome:

- No renderer change was made for these label residuals. The evidence supports treating them as
  browser/Cytoscape canvas bbox measurement tails until we have a generated Architecture
  canvas-label metric source or a better deterministic canvas measurer.
- Do not replace `ARCHITECTURE_CYTOSCAPE_CANVAS_LABEL_WIDTH_SCALE` with a new one-off constant to
  fix `stress_architecture_batch5_long_titles_and_punct_076`; the probe evidence is mixed by label
  length and icon floor.

Superseded note from HPD-050:

- The batch5 long-label conclusion still stands, but
  `stress_architecture_batch4_init_small_icons_061` was later closed without a label-scale tweak.
  The reusable source-backed fix was to transform the local `createText()` y-range when estimating
  rotated Architecture edge-label root bounds and to use `fontSize + 1px` for compound label
  bottom.

## M15RV-089 - Architecture Group/Port Residual Diagnostics

Fresh diagnostic evidence from 2026-06-02:

- `cargo run -p xtask -- compare-architecture-svgs --filter stress_architecture_html_titles_and_escapes_041 --check-dom --dom-mode parity --dom-decimals 3 --out target/compare/architecture_html_titles_and_escapes_parity_probe_m15rv089.md`:
  passed, confirming the focused row is root-only after parity geometry normalization.
- A focused browser probe for `stress_architecture_html_titles_and_escapes_041` reports the same
  alignment and relative-placement constraints as Rust, with browser pre-layout service bboxes of
  `Web Front Line 2=129x100`, `CDN Cache=92x100`, and `Origin primary=107x100`.
- Comparing upstream and local SVG output for `stress_architecture_html_titles_and_escapes_041`
  shows the root width is controlled by the group rectangle, not by edge labels: upstream group
  rect is about `x=-170.963,width=399.926`, while local is about `x=-172.463,width=404.926`.
  Service positions and edge-label transforms are only shifted by about `0.5px` in X.
- Mermaid source inspection of `svgDraw.ts` confirms edge labels are rendered after layout from
  source/target endpoints and do not feed a separate final group-rect rule. The row is therefore
  another group/service Cytoscape bbox approximation tail, not an HTML/entity parsing or edge-label
  source-rule bug.
- `stress_architecture_unicode_and_xml_escapes_019` follows the same pattern despite the fixture
  name: the authored fixture intentionally avoids XML/entity grammar pitfalls, the browser probe
  constraints match Rust, and the root width is controlled by `group-i`. Browser reports
  `Metrics Exporter=123x100`, while the current headless compound estimate is about `125px`; the
  local group rect is about `3px` wider.
- `cargo run -p xtask -- compare-architecture-svgs --filter stress_architecture_group_port_edges_017 --check-dom --dom-mode parity-root --dom-decimals 3 --report-root-all --out target/compare/architecture_group_port_edges_017_m15rv089_start.md`:
  expected failure; upstream root is about `707.769x542.448`, while local is about
  `709.238x524.603`.
- `node tools/debug/arch_fcose_browser_probe_fixture_025.js stress_architecture_group_port_edges_017`
  reports Mermaid constraints matching Rust debug output: horizontal `[[in1,in2],[out1,ext]]`,
  vertical `[[in1,out1]]`, and relative constraints `in1 -> in2`, `out1 top -> in1 bottom`, and
  `ext -> out1`, each with `gap=120`.
- Local `MERMAN_ARCH_DEBUG_CY_BBOX=1` shows all four service labels in
  `stress_architecture_group_port_edges_017` are icon-floor dominated (`half_w=41`, `bottom=18`),
  matching the browser probe's `82x100` service bboxes. The remaining height delta comes from the
  final FCoSE solution: browser separates the `out1` and `in1` top-left Y positions by about
  `238.948px`, while local separates them by about `221.103px`.
- Mermaid source and Rust implementation agree on the group-boundary edge force policy:
  same-parent edges use `idealEdgeLengthMultiplier * iconSize` and configured `edgeElasticity`;
  cross-parent edges use `0.5 * iconSize` and `0.001` elasticity. Manatee debug also shows the
  same alignment/relative input and the expected intergraph edge-length adjustment path.
- `stress_architecture_nested_groups_002` also has matching browser/Rust alignment and relative
  constraints. The remaining root width delta is from nested compound bbox/layout approximation:
  local service positions are shifted about `+1.25px` in X, and the outer platform group right
  edge lands about `3.75px` farther right even though its width is only about `0.5px` narrower.

Additional edge-label evidence:

- `stress_architecture_edge_label_corner_cases_012` has matching service icon-floor bboxes,
  matching constraints, and matching final SVG edge-label text splitting/transforms. Its
  `-1.788px` root width tail is controlled by browser `getBBox()` for the horizontal edge label
  `path api v1 items id 42`, not by routing or parser behavior.
- `stress_architecture_batch4_init_fontsize_wrap_063` has effective browser config
  `{ iconSize: 80, fontSize: 20, padding: 40 }`, matching local debug. Its service bboxes are
  icon-floor dominated and its two vertical edge labels split the same way upstream and locally;
  the `-1.788px` width tail is another rotated edge-label browser bbox residual.

Outcome:

- No renderer change was made for these rows. `stress_architecture_html_titles_and_escapes_041`
  and `stress_architecture_unicode_and_xml_escapes_019` are classified as group/service Cytoscape
  bbox measurement tails. `stress_architecture_edge_label_corner_cases_012` and
  `stress_architecture_batch4_init_fontsize_wrap_063` are classified as edge-label browser
  `getBBox()` tails. `stress_architecture_group_port_edges_017` and
  `stress_architecture_nested_groups_002` are classified as source-input-matched
  FCoSE/compound-bound residuals unless future evidence finds a reusable `cytoscape-fcose` rule
  missing in manatee.
- The existing Architecture diagonal-arrow behavior remains an intentional merman visual
  improvement: Mermaid emits translated port-direction polygons, while merman rotates diagonal
  arrowheads to the routed edge segment. The parity comparator already treats that transform as
  geometry noise, and root gates still compare the rendered viewport.

Superseded note from HPD-050:

- `stress_architecture_edge_label_corner_cases_012` and
  `stress_architecture_batch4_init_fontsize_wrap_063` were later closed by the source-backed
  Architecture edge-label root-bounds fix. Their earlier text-splitting evidence remains valid;
  the missing piece was bbox placement after `createText()` and rotation, not label wrapping.

## 2026-06-02 - Class HTML Label Width Investigation

Fresh focused evidence from 2026-06-02:

- `cargo run -p xtask -- compare-class-svgs --filter upstream_cypress_classdiagram_elk_v3_spec_elk_should_render_classes_with_different_text_labels_037 --check-dom --dom-mode parity-root --dom-decimals 3 --out target/compare/class_diff_labels_037_m15rv091_rendered_widths.md`:
  failed as expected before any retained change; the row is root-only.
- `cargo run -p xtask -- compare-class-svgs --filter upstream_cypress_classdiagram_handdrawn_v3_spec_hd_should_render_classes_with_different_text_labels_037 --check-dom --dom-mode parity-root --dom-decimals 3 --out target/compare/class_diff_labels_handdrawn_037_m15rv091_rendered_widths.md`:
  failed as expected before any retained change; the row is root-only.
- A direct upstream/local SVG scrape for the ELK fixture shows the residual is concentrated in
  HTML title `foreignObject` widths, especially `C12` and `C13`:
  upstream `183.421875 / 210.578125` vs local `175.375 / 208.6875`, which compounds into the
  root `max-width` tail (`2355.75px` upstream vs `2344.92px` local).
- A temporary experiment adding 12 rendered-width entries to
  `crates/merman-render/src/generated/class_text_overrides_11_12_2.rs` made both
  `different_text_labels_037` fixtures root-exact, confirming the diagnosis that these are
  browser-derived HTML label width facts rather than structure/viewBox bugs.
- `cargo run -p xtask -- report-overrides --check-no-growth` then failed with:
  `Text metric lookup overrides grew to 507 lookup entries, budget 495`.
- The experimental lookup additions were removed immediately; no retained renderer/generated-data
  change remains from this probe.

Outcome:

- The `different_text_labels_037` pair is now classified: it is a real Class HTML title width
  gap, but the current hand-curated override budget forbids solving it by simply appending 12 more
  lookup rows.
- The next aligned follow-up is not another local width constant. It is either:
  1. a stale-table cleanup that frees override budget while preserving current gates, or
  2. a generated/auditable Class HTML width evidence path that can replace older manual rows.
- Until that exists, do not claim Class is closer by hand-growing
  `class_text_overrides_11_12_2.rs`; that is self-defeating under the repo's no-growth gate.
- A same-day stale-table reconnaissance did not find safe bulk deletions. The obvious short-label
  candidates are still present in current fixture/test coverage (`Docs`, `Cool`, `uses`, `API`,
  `DB`, `Server`), and historical fearless-refactor evidence already records that multiple
  "small" Class lookup removals caused focused geometry drift or widespread golden churn. Treat
  future Class table cleanup as a delete-one-verify-one campaign, not a grep-based sweep.

## 2026-06-02 - Architecture Long-Label Scale Observability

Fresh focused evidence from 2026-06-02:

- `crates/merman-render/src/architecture_metrics.rs` now records the applied Cytoscape canvas-label
  width scale inside `ArchitectureCytoscapeCanvasLabelMetrics`, and the debug output reports that
  scale explicitly. This is an observability-only seam; no width math changed in this pass.
- `cargo test -p merman-render architecture_text_constants_match_mermaid -- --nocapture`:
  passed.
- `cargo test -p merman-render architecture_canvas_label_metrics_report_applied_scale -- --nocapture`:
  passed, covering the new metrics seam directly.
- `cargo run -p xtask -- compare-architecture-svgs --filter stress_architecture_batch5_long_titles_and_punct_076 --check-dom --dom-mode parity-root --dom-decimals 3 --out target/compare/architecture_batch5_long_titles_probe_after_scale_observability.md`:
  still fails with the same focused root tail as before this pass:
  upstream `543px`, local `548px` (`+5px`).
- `cargo run -p xtask -- compare-architecture-svgs --filter stress_architecture_batch4_init_small_icons_061 --check-dom --dom-mode parity-root --dom-decimals 3 --out target/compare/architecture_batch4_small_icons_probe_after_scale_observability.md`:
  still fails with the same focused small-icon tail as before this pass:
  upstream `187.75px`, local `178.5px` (`-9.25px`).

Outcome:

- This pass intentionally did not move the residual counts. It makes the current headless
  approximation easier to inspect and reason about before any future attempt to narrow the
  long-label branch.
- The stable `+5px` / `-9.25px` focused rows confirm that the new seam did not silently perturb the
  Architecture bbox policy.
- Future changes to the long-label branch should be framed as narrowing or replacing the current
  `>=200px -> 1.01` approximation, not as another opaque global constant tweak.
- A same-day upstream source recheck clarified that this branch approximates the Cytoscape layout
  canvas label path (`node[label]` + `compound-sizing-wrt-labels: include`), not the final SVG
  `createText(...).getBBox()` service-label path. The metric helper and constants were then renamed
  from generic `cytoscape/long-label` wording to explicit `layout_canvas_*` semantics without
  changing behavior. Focused Architecture residuals and the new metrics-seam tests stayed stable.

## 2026-06-02 - Architecture FCoSE Prelayout Adapter Boundary

Fresh implementation evidence:

- `crates/merman-render/src/architecture.rs` now isolates the Architecture-specific Cytoscape
  pre-layout bbox approximation in `architecture_fcose_prelayout_bounds(...)`.
- The helper returns the FCoSE `initial_center` and node `BoundsExtras`, keeping Mermaid/Cytoscape
  adapter policy in `merman-render` instead of pushing diagram-specific behavior into `manatee`.
- The layout view no longer stores group title state. Current source-backed evidence says group
  titles are rendered inside compound bounds and do not affect the pre-layout
  `eles.boundingBox()` center.

Fresh validation:

- `cargo test -p merman-render architecture_prelayout_bounds_feed_label_extras_without_group_title_state --lib`:
  passed.
- `cargo test -p merman-render architecture_relative_constraints_preserve_mermaid_duplicate_bfs_pops --lib`:
  passed.
- `cargo test -p merman-render --test architecture_layout_test`:
  passed.
- `cargo run -p xtask -- report-overrides --check-no-growth`:
  passed; Architecture root overrides remain `0`.
- `cargo run -p xtask -- compare-architecture-svgs --filter stress_architecture_batch5_long_titles_and_punct_076 --check-dom --dom-mode parity-root --dom-decimals 3 --report-root-all --out target/compare/architecture_batch5_after_prelayout_adapter.md`:
  expected failure with unchanged root-only residual, upstream `542.926px` vs local `547.926px`.

Outcome:

- This is a boundary cleanup and auditability improvement, not a residual-count reduction.
- Continue using this seam to audit which remaining Architecture rows are input-model mismatches,
  generated/bbox measurement tails, or source-input-matched FCoSE/compound residuals.

## Gate Set

Run after any code or generated-data change:

```bash
cargo run -p xtask -- compare-all-svgs --check-dom --dom-mode parity --dom-decimals 3
cargo run -p xtask -- report-overrides --check-no-growth
cargo fmt --check
git diff --check
```

Run when changing root policy, root overrides, or emitted bounds:

```bash
cargo run -p xtask -- compare-all-svgs --check-dom --dom-mode parity-root --dom-decimals 3
```

The `parity-root` command is allowed to fail while this lane is active, but it must fail with
bounded summaries and fresh per-diagram reports.

## 2026-06-02 - Baseline Naming Deconfusion And Override Inventory Recheck

Fresh focused evidence from 2026-06-02:

- `cargo test -p xtask overrides::report -- --nocapture`:
  passed after a small xtask refactor that centralizes the pinned Mermaid baseline label lookup.
- `cargo test -p xtask root_override_audit -- --nocapture`:
  passed after switching the audit report header from a stale hard-coded baseline string to the
  same shared pinned-baseline helper.
- `cargo run -p xtask -- report-overrides`:
  now prints `Mermaid baseline: @11.15.0`, proving the reporting surface no longer advertises the
  old `11.12.3` baseline while the repository is pinned to Mermaid 11.15.
- The same inventory run also gives a fresh honest override footprint for the active 11.15 lane:
  `241` root viewport entries, `488` text lookup entries, `1036` Sequence SVG text rows, and
  `3774` Flowchart font-metric rows.
- `crates/merman-render/src/generated/mod.rs` now documents that the retained
  `*_11_12_2.rs` generated filenames are storage-era artifacts rather than the active semantic
  contract. No generated file was renamed in this slice.
- `crates/merman-core/src/lib.rs` no longer claims headless parity is pinned to
  `mermaid@11.12.3`; the top-level crate docs now describe parity against the repository's pinned
  Mermaid baseline instead.

Outcome:

- No diagram renderer behavior changed in this slice. This was a deliberate deconfusion pass:
  remove stale baseline language from the active toolchain and make the remaining historical
  filename suffix explicit rather than silently misleading.
- The current 11.15 gap discussion should use the live override inventory above instead of a fake
  percentage-complete estimate. The next aligned work is to classify that inventory into:
  1. historical naming only,
  2. still-justified headless approximation debt,
  3. highest-value deletion or source-rule replacement targets.
- Do not treat the surviving `11_12_2` suffixes themselves as proof of a rendering gap. They are
  now explicitly documented as naming debt until a controlled regeneration/rename migration is
  worth the churn.
