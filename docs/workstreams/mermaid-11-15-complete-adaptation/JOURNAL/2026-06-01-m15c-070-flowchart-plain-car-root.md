# M15C-070 Flowchart Plain Car Root Slice

Date: 2026-06-01
Status: Done

## Summary

The leading handdrawn/demo hex-looking Flowchart strict-root bucket was not caused by hex geometry.
The hex polygon already matched the pinned Mermaid 11.15 SVG baseline. The root drift came from a
neighboring plain `F[Car]` label: upstream Mermaid 11.15 measured the plain DOM text width as
`24.203125px`, while the local vendored text path returned `45.015625px` and missed the existing
plain `Car` correction.

This slice updated the existing Flowchart label metric correction so it catches both the older
icon-probe width and the current vendored probe width. The correction remains guarded away from
`fa:` and inline `<i>` labels, so FontAwesome icon metrics keep their 11.15 inline icon box
behavior.

## Evidence Model

This fix is baseline/browser-metric anchored. Mermaid source does not carry per-word browser text
width constants such as `Car = 24.203125px`; those widths are produced by browser DOM measurement
and captured in the pinned Mermaid 11.15 SVG baselines.

Source-formula work remains separate. The previous shape-alias geometry slice was source anchored
to `hexagon.ts`, `linedCylinder.ts`, `waveRectangle.ts`, and `multiWaveEdgedRectangle.ts`.

## Verification

- `cargo nextest run -p merman-render flowchart_label_metrics_plain_car_uses_dom_text_width flowchart_label_metrics_for_layout_fontawesome_uses_nominal_boundary flowchart_html_fontawesome_icon_width_uses_nominal_boundary`: passed, 3 tests.
- `cargo run -p xtask -- compare-flowchart-svgs --filter upstream_cypress_flowchart_handdrawn_spec_fhd14_should_render_hexagons_014 --check-dom --dom-mode parity-root --dom-decimals 3 --report-root-all --report-label-all --no-root-overrides`: passed.
- `cargo run -p xtask -- compare-flowchart-svgs --filter upstream_html_demos_flowchart_flowchart_004 --check-dom --dom-mode parity-root --dom-decimals 3 --report-root-all --report-label-all --no-root-overrides`: passed.
- `cargo run -p xtask -- compare-flowchart-svgs --filter upstream_html_demos_flowchart_graph_003 --check-dom --dom-mode parity-root --dom-decimals 3 --report-root-all --report-label-all --no-root-overrides`: passed.
- `cargo run -p xtask -- compare-all-svgs --check-dom --dom-mode parity --dom-decimals 3`: passed after increasing the shell timeout; the first 4-minute run timed out without failure details.
- `cargo run -p xtask -- compare-flowchart-svgs --check-dom --dom-mode parity-root --dom-decimals 3 --report-root-all`: expected failure, now 148 Flowchart strict root-only mismatches, down from 160.
- `cargo fmt --check`: passed.
- `git diff --check`: passed.
- `cargo run -p xtask -- check-alignment`: passed.

## Next

Continue with the new largest Flowchart strict-root residuals: demo flowchart 016/052, shape-alias
36/27/20/21/12, delay half-rounded rectangle, Unicode punctuation/text metrics, markdown subgraph
root size, and remaining shape-family geometry/root clusters.
