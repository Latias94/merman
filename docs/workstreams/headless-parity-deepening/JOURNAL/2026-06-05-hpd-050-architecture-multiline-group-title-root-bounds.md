# HPD-050 - Architecture Multiline Group Title Root Bounds

Date: 2026-06-05
Task: HPD-050 Architecture-first layout engine audit

## Context

The previous long-group-title classification isolated
`stress_architecture_batch6_long_group_titles_wrapping_extreme_095` as a small root-only tail:
services and group rectangles were exact, while the root width was `-0.468750px` short because
Architecture group titles are emitted as SVG `<text>/<tspan>` and then observed through browser
`getBBox()`.

This row was a useful production candidate only if the rule stayed reusable and narrow: no root
pin, no single fixture constant, and no group padding or service-label scale change.

## Outcome

- Updated `push_architecture_groups(...)` so only wrapped, multi-line group titles round each
  measured SVG title row width up to the integer pixel boundary before unioning it into the
  synthetic root content bounds.
- Kept one-line group titles, service labels, child contribution bounds, group rectangles,
  FCoSE inputs, root overrides, and stored SVG baselines unchanged.
- This models the observed Chromium SVG `getBBox()` lattice for multi-`tspan` Architecture group
  titles without broadening to ordinary label measurement.

## Evidence

- Focused `stress_architecture_batch6_long_group_titles_wrapping_extreme_095` parity-root now
  passes with upstream/local `max-width: 533.000px`.
- `stress_architecture_long_group_titles_018` remains a separate existing residual at
  `+0.656px`; its title is one outer `tspan` row, so the multiline rule does not apply.
- Full Architecture parity-root remains an expected diagnostic failure, but the queue improves:
  DOM mismatches drop from `24` to `23`, non-zero root delta rows drop from `29` to `28`, and the
  absolute root-width residual sum drops from about `28.065px` to `27.596px`.
- `stress_architecture_batch6_long_group_titles_wrapping_extreme_095` is no longer in the
  mismatch list and appears as a `+0.000` root delta row.

## Verification

- `cargo fmt --check` - passed.
- `git diff --check` - passed.
- `cargo nextest run -p merman-render architecture` - passed, `31` tests run.
- `cargo run -p xtask -- report-overrides --check-no-growth` - passed; Architecture root
  overrides remain at `0`, and override-growth/root-usage checks are ok.
- `cargo run -p xtask -- compare-all-svgs --check-dom --dom-mode parity --dom-decimals 3` -
  passed across the implemented matrix.
- `cargo run -p xtask -- compare-architecture-svgs --filter stress_architecture_batch6_long_group_titles_wrapping_extreme_095 --check-dom --dom-mode parity-root --dom-decimals 3 --report-root-all --no-root-overrides --out target\compare\arch-focused-095-ceil` -
  passed.
- `cargo run -p xtask -- compare-architecture-svgs --filter stress_architecture_long_group_titles_018 --check-dom --dom-mode parity-root --dom-decimals 3 --report-root-all --no-root-overrides --out target\compare\arch-focused-018-ceil` -
  expected-failed with the existing `+0.656px` root-width tail.
- `cargo run -p xtask -- compare-architecture-svgs --check-dom --dom-mode parity-root --dom-decimals 3 --report-root-all --no-root-overrides --out target\compare\architecture-report-parity-root-multiline-title-ceil-final` -
  expected-failed with `23` Architecture root/style mismatches and `28` non-zero root delta rows.

## Residual Boundary

This closes only the multi-line group-title SVG root-union lattice tail represented by `095`.
It does not justify a global title-width ceiling, a service-label measurement change, group padding
change, root override, or baseline refresh. The remaining direct Architecture width tails still
belong to service child contribution / Cytoscape bbox phase residuals unless new source evidence
proves otherwise.
