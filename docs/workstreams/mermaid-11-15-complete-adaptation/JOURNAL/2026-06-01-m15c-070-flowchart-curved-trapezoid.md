# M15C-070 Flowchart Curved Trapezoid Geometry

Date: 2026-06-01

## Scope

Close the strict-root residuals for the no-label `newshapesset3` curved-trapezoid fixtures by
aligning local Flowchart curved-trapezoid sizing and root bounds with Mermaid 11.15.

## Source Reference

- `repo-ref/mermaid/packages/mermaid/src/rendering-util/rendering-elements/shapes/curvedTrapezoid.ts`
  - Mermaid 11.15 uses `minWidth = 20` and `minHeight = 5`.
  - The shape width is `max(minWidth, (bbox.width + labelPaddingX * 2) * 1.25, node?.width ?? 0)`.
  - The shape height is `max(minHeight, bbox.height + labelPaddingY * 2, node?.height ?? 0)`.

## Diagnosis

The no-label `newshapesset3` LR/TB rows still had a shared root width drift after the shape-alias
geometry sweep. Local curved-trapezoid sizing still used older minimum geometry constants
`80x20`, while Mermaid 11.15 uses `20x5`. This made the empty curved-trapezoid nodes wider than
upstream and shifted the graph root.

## Changes

- Updated Flowchart layout sizing for `curv-trap` / `display` / `curved-trapezoid`.
- Updated parity SVG shape rendering to use the same Mermaid 11.15 minimum constants.
- Updated root-bounds reconstruction for curved-trapezoid to use the 11.15 pre-width minimum.
- Updated curved-trapezoid edge-intersection geometry to keep layout, render, root-bounds, and
  intersection math consistent.
- Added curved-trapezoid coverage to
  `flowchart_node_shape_dimensions_follow_mermaid_rules`.

## Validation

- `cargo nextest run -p merman-render flowchart_node_shape_dimensions_follow_mermaid_rules`:
  passed.
- `cargo run -p xtask -- compare-flowchart-svgs --filter upstream_cypress_newshapes_spec_newshapessets_newshapesset3_lr_nolabel_065 --check-dom --dom-mode parity-root --dom-decimals 3 --report-root-all --report-label-all --no-root-overrides`:
  passed.
- `cargo run -p xtask -- compare-flowchart-svgs --filter upstream_cypress_newshapes_spec_newshapessets_newshapesset3_tb_nolabel_017 --check-dom --dom-mode parity-root --dom-decimals 3 --report-root-all --report-label-all --no-root-overrides`:
  passed.
- `cargo run -p xtask -- compare-flowchart-svgs --filter newshapesset3_ --check-dom --dom-mode parity-root --dom-decimals 3 --report-root-all --report-label-all --no-root-overrides`:
  still fails only for the remaining markdown/html=false rows
  `upstream_cypress_newshapes_spec_newshapessets_newshapesset3_lr_md_html_false_070` and
  `upstream_cypress_newshapes_spec_newshapessets_newshapesset3_tb_md_html_false_022`.
- `cargo run -p xtask -- compare-flowchart-svgs --check-dom --dom-mode parity-root --dom-decimals 3 --report-root-all`:
  still fails as expected with 69 Flowchart strict root-only mismatches, down from 71.
- `cargo run -p xtask -- compare-all-svgs --check-dom --dom-mode parity --dom-decimals 3`:
  passed.

## Follow-Up

Continue M15C-070 by sampling the remaining Flowchart strict-root residuals. The next leading
buckets are Unicode punctuation/text metrics, icon-only root metrics, markdown/html=false
new-shape rows, demo/root rounding rows, and small cross-family root viewport residuals.
