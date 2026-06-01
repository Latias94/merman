# Mermaid 11.15 Root Viewport Residuals - Evidence And Gates

Status: Active
Last updated: 2026-06-01

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
