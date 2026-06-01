# M15C-070 Flowchart Bow Tie Rectangle

Date: 2026-06-01
Status: Done

## Summary

`upstream_cypress_flowchart_shape_alias_spec_shape_alias_aliasset36_036` exposed an old
bow-tie/stored-data width formula. Local layout and rendering still used
`text_width + nodePadding + 20px`; Mermaid 11.15 `bowTieRect.ts` uses classic horizontal label
padding of `2 * nodePadding` before the sampled arc bbox expands the outer path.

The fixture has three bow-tie aliases, so the old formula made each node about `5px` too wide and
produced a `+15.016px` root delta.

## Change

Updated `bow-rect`/`stored-data`/`bow-tie-rectangle` in:

- Flowchart layout sizing.
- Flowchart SVG shape rendering.
- Flowchart edge intersection geometry.

Added bow-tie rectangle coverage to `flowchart_node_shape_dimensions_follow_mermaid_rules`.

## Verification

- `cargo nextest run -p merman-render flowchart_node_shape_dimensions_follow_mermaid_rules`: passed.
- `cargo run -p xtask -- compare-flowchart-svgs --filter upstream_cypress_flowchart_shape_alias_spec_shape_alias_aliasset36_036 --check-dom --dom-mode parity-root --dom-decimals 3 --report-root-all --report-label-all --no-root-overrides`: passed.
- `cargo run -p xtask -- compare-flowchart-svgs --filter shape_alias --check-dom --dom-mode parity-root --dom-decimals 3 --report-root-all --report-label-all --no-root-overrides`: expected failure for remaining alias buckets; alias set 36 is gone from the failure list.
- `cargo run -p xtask -- compare-flowchart-svgs --check-dom --dom-mode parity-root --dom-decimals 3 --report-root-all`: expected failure, now 144 Flowchart strict root-only mismatches, down from 146.
- `cargo run -p xtask -- compare-all-svgs --check-dom --dom-mode parity --dom-decimals 3`: passed.
- `cargo fmt --check`: passed.
- `git diff --check`: passed.

## Next

Continue with the remaining top Flowchart strict-root residuals: shape-alias 27/20/21/12, delay
half-rounded rectangle, Unicode punctuation/text metrics, markdown subgraph root size, and
shape-family geometry/root clusters.
