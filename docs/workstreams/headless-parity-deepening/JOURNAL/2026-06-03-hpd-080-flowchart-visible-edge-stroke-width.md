# HPD-080 - Flowchart Visible Edge Stroke Width

Task: HPD-080 visible rendering defect triage.

## Question

Does Flowchart `themeVariables.strokeWidth` affect the ordinary edge path users actually see, or
only a provider selector that current local DOM does not carry?

## Source Audit

- Pinned Mermaid 11.15 `packages/mermaid/src/styles.ts` sets `.edge-thickness-normal` to
  `options.strokeWidth ?? 1` with a `px` suffix.
- Pinned Mermaid 11.15 `packages/mermaid/src/diagrams/flowchart/styles.ts` also sets
  `.edgePath .path` from `options.strokeWidth`, but ordinary Flowchart paths in current output do
  not carry `.path`.
- Pinned Mermaid 11.15 `packages/mermaid/src/diagrams/flowchart/flowDb.ts` seeds ordinary edge
  classes as `edge-thickness-normal edge-pattern-solid flowchart-link`.
- A focused Mermaid CLI render with `themeVariables.strokeWidth = 4` confirmed the final SVG
  stylesheet contains `.edge-thickness-normal{stroke-width:4px;}` and the visible ordinary edge
  path carries `edge-thickness-normal ... flowchart-link`.

## Outcome

- Updated local Flowchart CSS so `.edge-thickness-normal` consumes `SvgTheme::css_value(
  "strokeWidth", "1")`.
- Kept `.edge-thickness-thick`, dashed/dotted patterns, and invisible-edge behavior unchanged.
- Added focused renderer coverage that asserts both the visible edge class rule and the visible
  edge path class tuple.
- Added an explicit `linkStyle` regression proving inline edge `stroke-width` still overrides the
  themed default class width.
- Tightened public dark-theme smoke so Flowchart `strokeWidth` is counted only with matching
  visible edge DOM, not only by the inert `.edgePath .path` provider rule.

## Verification

- `cargo fmt --check` - passed.
- `cargo nextest run -p merman-render --test flowchart_svg_test` - passed, `28` tests run.
- `cargo nextest run -p merman --features render --test theme_renderability_smoke` - passed, `10`
  tests run.
- `cargo run -p xtask -- compare-flowchart-svgs --check-dom --dom-mode parity --dom-decimals 3` -
  passed.
- `cargo run -p xtask -- compare-flowchart-svgs --check-dom --dom-mode parity-root --dom-decimals 3 --report-root-all --out target\compare\flowchart_report_parity_root_hpd080_edge_strokewidth.md` -
  expected-failed on the known Flowchart root/max-width diagnostic surface.
- `git diff --check` - passed.

## Residual

- Flowchart `parity-root` remains a diagnostic/root-bounds residual surface. This slice fixes a
  source-backed visible edge theme-consumption bug and does not tune layout/root dimensions.
