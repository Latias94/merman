# M15C-070 Flowchart Document And Delay Geometry

Date: 2026-06-01
Status: DONE_WITH_CONCERNS

## Scope

This slice closed the Flowchart strict-root buckets for:

- `doc` / `document`
- `delay` / `half-rounded-rectangle`
- shape-alias sets 20 and 21
- docs single-delay fixture 094

It did not attempt to close the remaining Flowchart strict-root residual set.

## Source References

- `repo-ref/mermaid/packages/mermaid/src/rendering-util/rendering-elements/shapes/waveEdgedRectangle.ts`
  - Mermaid 11.15 uses `minWidth = 14` for the wave-edged rectangle.
- `repo-ref/mermaid/packages/mermaid/src/rendering-util/rendering-elements/shapes/halfRoundedRectangle.ts`
  - Mermaid 11.15 uses `minWidth = 15` and `minHeight = 10`.

## Changes

- Updated Flowchart layout sizing, SVG rendering, edge intersection, and root-bounds reconstruction
  for `doc` / `document` to use the Mermaid 11.15 `minWidth=14` formula.
- Updated Flowchart layout sizing, SVG rendering, edge intersection, and root-bounds reconstruction
  for `delay` / `half-rounded-rectangle` to use the Mermaid 11.15 `15x10` minimum formula.
- Removed `delay` from the old curved-trapezoid root-bounds theoretical-width branch; the actual
  `halfRoundedRectangle.ts` path should be reconstructed by the delay-specific RoughJS bbox path.
- Added a narrow Flowchart HTML width override for the default-font
  `half-rounded-rectangle` label at `166.21875px`. This is upstream SVG/browser metric evidence,
  not a Mermaid shape-source formula.
- Extended `flowchart_node_shape_dimensions_follow_mermaid_rules` to cover document and delay
  layout dimensions.

## Validation

- `cargo run -p xtask -- compare-flowchart-svgs --filter upstream_cypress_flowchart_shape_alias_spec_shape_alias_aliasset20_020 --check-dom --dom-mode parity-root --dom-decimals 3 --report-root-all --report-label-all --no-root-overrides`: passed.
- `cargo run -p xtask -- compare-flowchart-svgs --filter upstream_docs_flowchart_delay_half_rounded_rectangle_094 --check-dom --dom-mode parity-root --dom-decimals 3 --report-root-all --report-label-all --no-root-overrides`: passed.
- `cargo run -p xtask -- compare-flowchart-svgs --filter upstream_cypress_flowchart_shape_alias_spec_shape_alias_aliasset21_021 --check-dom --dom-mode parity-root --dom-decimals 3 --report-root-all --report-label-all --no-root-overrides`: passed.
- `cargo nextest run -p merman-render flowchart_node_shape_dimensions_follow_mermaid_rules`: passed.
- `cargo run -p xtask -- compare-flowchart-svgs --filter shape_alias --check-dom --dom-mode parity-root --dom-decimals 3 --report-root-all --report-label-all --no-root-overrides`: still failed as expected, now only for alias buckets `12`, `29`, `34`, and `38`.
- `cargo run -p xtask -- compare-flowchart-svgs --check-dom --dom-mode parity-root --dom-decimals 3 --report-root-all`: still failed as expected with 124 Flowchart strict root-only mismatches, down from 129.
- `cargo run -p xtask -- compare-all-svgs --check-dom --dom-mode parity --dom-decimals 3`: passed.
- `cargo fmt --check`: passed.
- `git diff --check`: passed.
- `cargo run -p xtask -- check-alignment`: passed.

## Remaining Work

Flowchart strict-root is still red. The immediate next buckets are shape-alias `12`, `29`, `38`,
unpinned `34`, Unicode punctuation/text metrics, markdown subgraph root sizing, double-circle and
single-node shape roots, and broader shape-family root buckets.
