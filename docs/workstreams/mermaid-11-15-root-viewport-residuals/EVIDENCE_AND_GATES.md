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
