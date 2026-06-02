# HPD-080 - XYChart Inline Theme Render Coverage

Date: 2026-06-02
Task: HPD-080 visible rendering defect triage

## Context

XYChart is an important negative case for the HPD-080 theme audit: Mermaid 11.15 does not expose a
diagram CSS provider for it. Theme behavior is inline. The DB merges
`themeVariables.xyChart` into `XYChartThemeConfig`, and the renderer writes those values directly
to SVG `fill` and `stroke` attributes.

That means merman should not invent XYChart CSS. It should prove that custom theme values reach the
final SVG render path.

## Source Checks

Pinned Mermaid source commit:

- `41646dfd43ac83f001b03c70605feb036afae46d`

Checked source files:

- `packages/mermaid/src/diagrams/xychart/xychartDb.ts`
- `packages/mermaid/src/diagrams/xychart/xychartRenderer.ts`
- `packages/mermaid/src/diagrams/xychart/chartBuilder/interfaces.ts`
- `packages/mermaid/src/themes/theme-default.js`
- `packages/mermaid/src/themes/theme-base.js`

Existing local fixture:

- `fixtures/xychart/upstream_cypress_xychart_spec_render_all_the_theme_color_018.mmd`

## Outcome

- Added a render-path regression in `crates/merman-render/tests/xychart_svg_test.rs`.
- The test renders the existing upstream fixture and asserts that custom XYChart theme values reach:
  - chart background,
  - chart title,
  - x-axis title and labels,
  - y-axis title and labels,
  - x/y axis lines and ticks,
  - bar and line plot palette colors.
- No production change was needed. The local renderer already follows Mermaid's inline XYChart
  theme model.

## Verification

- `cargo fmt -p merman-render`
- `cargo fmt --check -p merman-render`
- `cargo test -p merman-render xychart_svg_honors_mermaid_11_15_inline_theme_config --test xychart_svg_test`
- `cargo test -p merman-render --test xychart_svg_test`
- `cargo run -p xtask -- compare-xychart-svgs --check-dom --dom-mode parity --dom-decimals 3`
- `git diff --check`

## Residual

This slice does not claim full visual or pixel parity. It locks down the Mermaid 11.15 inline theme
contract for XYChart and keeps the HPD-080 boundary clear: no provider means no invented CSS, not
no theme responsibility.
