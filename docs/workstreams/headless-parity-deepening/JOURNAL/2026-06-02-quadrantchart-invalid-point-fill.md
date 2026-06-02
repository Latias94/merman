# HPD-080 - QuadrantChart Invalid Default Point Fill

Date: 2026-06-02
Task: HPD-080 visible rendering defect triage

## Context

The remaining dark-theme renderability scan found a supported diagram that could pass structural
DOM parity while still emitting invalid SVG colors. QuadrantChart default data points rendered as
`fill="hsl(240, 100%, NaN%)"` and `stroke="hsl(240, 100%, NaN%)"` when the diagram did not provide
an explicit point color.

This is a visible renderability defect, not a browser-measurement residual. A headless renderer
should not preserve invalid CSS tokens just because the pinned upstream fixture contains them.

## Source Checks

Pinned Mermaid source commit:

- `41646dfd43ac83f001b03c70605feb036afae46d`

Checked source files and dependencies:

- `packages/mermaid/src/themes/theme-default.js`
- `packages/mermaid/src/diagrams/quadrant-chart/quadrantRenderer.ts`
- `tools/mermaid-cli/node_modules/khroma/src/methods/lighten.ts`
- `tools/mermaid-cli/node_modules/khroma/src/methods/darken.ts`
- `tools/mermaid-cli/node_modules/khroma/src/methods/luminance.ts`

Important source-backed findings:

- Mermaid 11.15 intends `quadrantPointFill` to come from `lighten(quadrant1Fill)` or
  `darken(quadrant1Fill)`.
- The shipped source calls khroma `lighten` / `darken` without the required `amount` argument.
  Khroma then adjusts the lightness channel by `undefined`, which produces the saved
  `hsl(...NaN%)` fixture output.
- QuadrantChart has no CSS provider (`styles: () => ''`); theme behavior is inline renderer data,
  point styles, and class styles.

## Outcome

- QuadrantChart no longer emits upstream's invalid default point color. When no valid
  `themeVariables.quadrantPointFill` is available, merman derives a stable 10% lightness shift from
  `quadrant1Fill` and falls back to the border color only when the source color cannot be parsed.
- Valid `themeVariables.quadrantPointFill` still wins and is preserved as a raw CSS token, matching
  Mermaid's explicit override behavior.
- Added renderer regressions in `crates/merman-render/tests/quadrantchart_svg_test.rs` for:
  - valid default data point fill/stroke,
  - explicit `quadrantPointFill` / `quadrantPointTextFill` overrides.
- Extended the public API dark-theme smoke to include QuadrantChart inline theme variables.
- Updated xtask DOM parity normalization so only QuadrantChart default data-point circle
  `fill`/`stroke` treats upstream `hsl(...NaN%)` and the local fixed default as the same comparison
  slot. Strict DOM comparison still exposes the real difference.

## Verification

- `cargo fmt -p merman-render -p merman -p xtask`
- `cargo test -p merman-render --test quadrantchart_svg_test`
- `cargo test -p merman representative_dark_theme_diagrams_keep_visible_theme_signals --test theme_renderability_smoke --features render`
- `cargo test -p xtask parity_normalizes_quadrantchart_invalid_default_point_color`
- `cargo fmt --check -p merman-render -p merman -p xtask`
- `cargo run -p xtask -- compare-quadrantchart-svgs --check-dom --dom-mode parity --dom-decimals 3`
- `cargo run -p xtask -- compare-quadrantchart-svgs --check-dom --dom-mode parity-root --dom-decimals 3`

## Residual

This is an intentional renderability-over-byte-parity correction for a known upstream invalid CSS
token. Do not generalize it into broad color normalization. If Mermaid fixes the missing khroma
amount in a future baseline, revisit the local 10% lightness policy against the new source and
fixtures.
