# M15C-070 Flowchart Icon Root Pin

Date: 2026-06-01

## Scope

Refresh the stale root viewport override for the remaining Flowchart icon example strict-root row.

## Diagnosis

`upstream_cypress_flowchart_icon_spec_example_002` failed strict-root with root overrides enabled:

- Upstream Mermaid 11.15 root: `viewBox="0 0 98.046875 70"`, `max-width: 98.0469px`.
- Existing local root pin: `viewBox="0 0 92.046875 70"`, `max-width: 92.0469px`.

With root overrides disabled, the fixture passed strict-root, proving the renderer output already
matched the 11.15 baseline and the active failure was only the stale root pin.

## Changes

- Refreshed the existing Flowchart root override for
  `upstream_cypress_flowchart_icon_spec_example_002` to the Mermaid 11.15 root.
- Updated the override report text lookup budget to `495`. The previous Unicode text-metric slice
  added five fixture-derived 11.15 browser metrics with targeted tests and SVG evidence; the budget
  is explicit rather than implicit.

## Validation

- `cargo run -p xtask -- compare-flowchart-svgs --filter upstream_cypress_flowchart_icon_spec_example_002 --check-dom --dom-mode parity-root --dom-decimals 3 --report-root-all --report-label-all --no-root-overrides`:
  passed.
- `cargo run -p xtask -- compare-flowchart-svgs --filter upstream_cypress_flowchart_icon_spec_example_002 --check-dom --dom-mode parity-root --dom-decimals 3 --report-root-all --report-label-all`:
  passed.
- `cargo run -p xtask -- report-overrides --check-no-growth`:
  passed; root viewport overrides remain at `282` total entries and text lookup entries are now
  capped at `495`.
- `cargo run -p xtask -- compare-flowchart-svgs --check-dom --dom-mode parity-root --dom-decimals 3 --report-root-all`:
  still fails as expected with 66 Flowchart strict root-only mismatches, down from 67.

## Follow-Up

Continue M15C-070 by sampling the remaining leading Flowchart strict-root residuals. Current top
rows include styled-text/root size, Flowchart parameters, shape-mix, demo flowchart 010/049,
markdown/html=false new/old shape rows, and small subgraph/root rounding rows.
