# M15C-070 Flowchart Lined/Tagged Document Geometry

Date: 2026-06-01
Status: DONE_WITH_CONCERNS

## Scope

This slice closed the remaining Flowchart shape-alias strict-root buckets:

- `lin-doc` / `lined-document`
- `tag-doc` / `tagged-document`
- shape-alias sets 29, 34, and 38

It did not attempt to close the broader Flowchart strict-root residual set.

## Source Reference

- `repo-ref/mermaid/packages/mermaid/src/rendering-util/rendering-elements/shapes/linedWaveEdgedRect.ts`
  - Mermaid 11.15 uses `waveAmplitude = node.look === 'neo' ? h / 4 : h / 8`.
- `repo-ref/mermaid/packages/mermaid/src/rendering-util/rendering-elements/shapes/taggedWaveEdgedRectangle.ts`
  - Mermaid 11.15 uses `waveAmplitude = h / 8`.
  - The tag sine path uses `(y + h) * 1.3`, `(y + h) * 1.25`, `(y + h) * 1.3`, and `-h * 0.02`.

## Changes

- Updated Flowchart layout sizing for lined/tagged document shapes to use Mermaid 11.15 classic
  wave amplitudes.
- Updated SVG rendering for lined/tagged document shapes with the same source formulas.
- Updated tagged-document root-bounds reconstruction to union the rendered rough wave path and tag
  path instead of using the stale asymmetric-height shortcut.
- Updated tagged-document edge intersection to use the 11.15 wave amplitude.
- Added regression coverage for lined/tagged document shape dimensions.
- Added a narrow Flowchart HTML width override for `stacked-rectangle` at `128.578125px`. This is
  upstream SVG/browser metric evidence, not a Mermaid source-formula value.

## Validation

- `cargo run -p xtask -- compare-flowchart-svgs --filter upstream_cypress_flowchart_shape_alias_spec_shape_alias_aliasset29_029 --check-dom --dom-mode parity-root --dom-decimals 3 --report-root-all --report-label-all --no-root-overrides`: passed.
- `cargo run -p xtask -- compare-flowchart-svgs --filter upstream_cypress_flowchart_shape_alias_spec_shape_alias_aliasset38_038 --check-dom --dom-mode parity-root --dom-decimals 3 --report-root-all --report-label-all --no-root-overrides`: passed.
- `cargo run -p xtask -- compare-flowchart-svgs --filter upstream_cypress_flowchart_shape_alias_spec_shape_alias_aliasset34_034 --check-dom --dom-mode parity-root --dom-decimals 3 --report-root-all --report-label-all --no-root-overrides`: passed.
- `cargo run -p xtask -- compare-flowchart-svgs --filter shape_alias --check-dom --dom-mode parity-root --dom-decimals 3 --report-root-all --report-label-all --no-root-overrides`: passed.
- `cargo run -p xtask -- compare-flowchart-svgs --filter shape_alias --check-dom --dom-mode parity --dom-decimals 3`: passed.
- `cargo nextest run -p merman-render default_font_flowchart_html_width_overrides_match_upstream flowchart_node_shape_dimensions_follow_mermaid_rules`: passed.
- `cargo run -p xtask -- compare-flowchart-svgs --check-dom --dom-mode parity-root --dom-decimals 3 --report-root-all`: still failed as expected with 71 Flowchart strict root-only mismatches, down from 101.
- `cargo run -p xtask -- compare-all-svgs --check-dom --dom-mode parity --dom-decimals 3`: passed.
- `cargo fmt --check`: passed.
- `git diff --check`: passed.
- `cargo run -p xtask -- check-alignment`: passed.

## Remaining Work

Flowchart strict-root is still red. The full shape-alias bucket is now closed. The next residuals
are no-label new shape geometry/root buckets, Unicode/text metrics, markdown edge or subgraph root
sizing, icon-only roots, and small root-rounding rows.
