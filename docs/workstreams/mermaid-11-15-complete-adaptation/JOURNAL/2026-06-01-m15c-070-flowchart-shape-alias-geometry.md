# M15C-070 Flowchart Shape-Alias Geometry

Date: 2026-06-01
Status: Done for this slice

## Summary

Closed the largest Flowchart strict-root shape-alias buckets by replacing old shape formulas with
Mermaid 11.15 geometry from the upstream source:

- `hexagon.ts`: `hex`/`hexagon`/`prepare` shoulder is derived from padded height.
- `linedCylinder.ts`: `lin-cyl`/`disk`/`lined-cylinder` use two-sided padding and the 11.15
  cylinder cap formula.
- `waveRectangle.ts`: `paper-tape`/`flag` removed old `100x50` minimums and uses `h / 8` wave
  amplitude.
- `multiWaveEdgedRectangle.ts`: `docs`/`documents`/`st-doc`/`stacked-document` use
  `rectOffset=10`, `h = label + 3p`, `waveAmplitude = h / 8`, and the matching root-bounds
  calculation.

## Evidence

- `cargo nextest run -p merman-render flowchart_node_shape_dimensions_follow_mermaid_rules`:
  passed.
- `cargo run -p xtask -- compare-flowchart-svgs --filter upstream_cypress_flowchart_shape_alias_spec_shape_alias_aliasset7_007 --check-dom --dom-mode parity-root --dom-decimals 3 --report-root-all --report-label-all --no-root-overrides`:
  passed.
- `cargo run -p xtask -- compare-flowchart-svgs --filter upstream_cypress_flowchart_shape_alias_spec_shape_alias_aliasset23_023 --check-dom --dom-mode parity-root --dom-decimals 3 --report-root-all --report-label-all --no-root-overrides`:
  passed.
- `cargo run -p xtask -- compare-flowchart-svgs --filter upstream_cypress_flowchart_shape_alias_spec_shape_alias_aliasset35_035 --check-dom --dom-mode parity-root --dom-decimals 3 --report-root-all --report-label-all --no-root-overrides`:
  passed.
- `cargo run -p xtask -- compare-flowchart-svgs --filter upstream_cypress_flowchart_shape_alias_spec_shape_alias_aliasset33_033 --check-dom --dom-mode parity-root --dom-decimals 3 --report-root-all --report-label-all --no-root-overrides`:
  passed.
- `cargo run -p xtask -- compare-all-svgs --check-dom --dom-mode parity --dom-decimals 3`:
  passed.
- `cargo run -p xtask -- compare-flowchart-svgs --check-dom --dom-mode parity-root --dom-decimals 3 --report-root-all`:
  failed as expected with 160 Flowchart strict root-only mismatches, down from 202 before this
  slice.
- `cargo fmt --check`: passed.
- `git diff --check`: passed.

## Next

Continue M15C-070 with the current top Flowchart strict-root buckets: handdrawn/demo hex roots,
demo flowchart 016/052, remaining shape-alias buckets `36`, `27`, `20`, `21`, and `12`, delay
half-rounded rectangle, markdown-subgraph roots, and shape-family root clusters.
