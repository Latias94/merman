# HPD-080 - Block Visible Edge Stroke Width

Task: HPD-080 visible rendering defect triage.

## Question

Does Block `themeVariables.strokeWidth` affect the ordinary edge path users actually see, or only a
provider selector that current local DOM does not carry?

## Source Audit

- Pinned Mermaid 11.15 `packages/mermaid/src/styles.ts` sets `.edge-thickness-normal` to
  `options.strokeWidth ?? 1` with a `px` suffix.
- Pinned Mermaid 11.15 `packages/mermaid/src/diagrams/block/renderHelpers.ts` assigns visible
  Block edge paths `edge-thickness-normal edge-pattern-solid flowchart-link LS-a1 LE-b1`.
- Pinned Mermaid 11.15 `packages/mermaid/src/diagrams/block/styles.ts` also emits
  `.edgePath .path`, but ordinary Block edge paths in current local output do not carry `.path`.
- A focused Mermaid CLI render with `themeVariables.strokeWidth = 4` confirmed upstream final SVG
  contains `.edge-thickness-normal{stroke-width:4px;}` and the visible Block path carries the
  matching edge-thickness class.

## Outcome

- Updated local Block CSS to emit Mermaid's shared edge thickness and pattern rules.
- Routed `.edge-thickness-normal` through `SvgTheme::css_value("strokeWidth", "1")`.
- Kept Block node stroke width, cluster fade behavior, and `.edgePath .path` provider width
  unchanged.
- Added focused renderer coverage for the final visible Block edge class and themed CSS rule.
- Expanded public dark-theme smoke to include a Block edge alongside the existing composite cluster
  color coverage.

## Verification

- `cargo nextest run -p merman-render --test block_svg_test` - passed, `4` tests run.
- `cargo run -p xtask -- compare-block-svgs --check-dom --dom-mode parity --dom-decimals 3` -
  passed.
- `cargo nextest run -p merman --features render --test theme_renderability_smoke` - passed, `10`
  tests run.
- `cargo fmt --check` - passed.
- `git diff --check` - passed.

## Residual

- This does not claim broader Block CSS parity beyond source-backed rules that current Block SVG
  actually consumes.
