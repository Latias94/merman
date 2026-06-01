# M15C-070 Flowchart Double-Circle Geometry

Date: 2026-06-01
Status: DONE_WITH_CONCERNS

## Scope

This slice closed the Flowchart strict-root bucket for:

- `dbl-circ`
- `double-circle`
- `doublecircle`
- shape-alias set 12

It did not attempt to close the remaining Flowchart strict-root residual set.

## Source Reference

- `repo-ref/mermaid/packages/mermaid/src/rendering-util/rendering-elements/shapes/doubleCircle.ts`
  - Mermaid 11.15 computes `outerRadius = bbox.width / 2 + padding`.
  - The inner circle uses a fixed `5px` gap.

## Changes

- Updated Flowchart layout sizing for double-circle shapes from the old
  `label width + padding + 10px` diameter to Mermaid 11.15's `label width + 2 * padding`
  diameter.
- Updated `flowchart_node_shape_dimensions_follow_mermaid_rules` to cover the 11.15
  double-circle formula.
- SVG rendering already consumed the layout width/height, so the emitted outer/inner circle radii
  followed the corrected formula without a separate renderer patch.

## Validation

- `cargo run -p xtask -- compare-flowchart-svgs --filter upstream_cypress_flowchart_shape_alias_spec_shape_alias_aliasset12_012 --check-dom --dom-mode parity-root --dom-decimals 3 --report-root-all --report-label-all --no-root-overrides`: passed.
- `cargo nextest run -p merman-render flowchart_node_shape_dimensions_follow_mermaid_rules`: passed.
- `cargo run -p xtask -- compare-flowchart-svgs --filter shape_alias --check-dom --dom-mode parity-root --dom-decimals 3 --report-root-all --report-label-all --no-root-overrides`: still failed as expected, now only for alias buckets `29`, `34`, and `38`.
- `cargo run -p xtask -- compare-flowchart-svgs --check-dom --dom-mode parity-root --dom-decimals 3 --report-root-all`: still failed as expected with 101 Flowchart strict root-only mismatches, down from 124.
- `cargo run -p xtask -- compare-all-svgs --check-dom --dom-mode parity --dom-decimals 3`: passed.
- `cargo fmt --check`: passed.
- `git diff --check`: passed.
- `cargo run -p xtask -- check-alignment`: passed.

## Remaining Work

Flowchart strict-root is still red. The immediate shape-alias buckets are now `29`, `38`, and
unpinned `34`. Broader residuals remain in Unicode/text metrics, markdown subgraph root sizing,
and shape-family layout/root clusters.
