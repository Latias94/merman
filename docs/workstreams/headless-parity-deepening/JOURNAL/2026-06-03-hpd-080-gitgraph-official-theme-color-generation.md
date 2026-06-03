# HPD-080 GitGraph Official Theme Color Generation

Date: 2026-06-03

## Summary

Audited the user-provided multi-branch GitGraph merge sample. The default local render was finite
and readable, but the audit exposed a more specific source-backed theme gap: local GitGraph CSS
always emitted classic/default per-branch `git0` rules, while Mermaid 11.15 switches official
`neo` / `redux*` themes into `genColor(...)`.

This is a visible theme/CSS parity issue, not a layout or root-bounds issue.

## Source Evidence

- `repo-ref/mermaid/packages/mermaid/src/diagrams/git/styles.js`
- `target/compare/gitgraph_redux_audit_upstream.svg`
- `target/compare/gitgraph_neo_audit_upstream.svg`

Fresh Mermaid CLI output showed:

- `redux` uses `nodeBorder`, `mainBkg`, `noteFontWeight`, `strokeWidth`, `4 2` branch dashes, and
  `mainBkg` merge/reverse/highlight-inner fills.
- `redux-color` / `redux-dark-color` use `borderColorArray` for non-zero branches, with dark themes
  using `mainBkg` for branch-label background fill.
- `neo` emits a scoped linear gradient def after the initial `<g/>` and uses
  `stroke:url(#<svg-id>-gradient)` on `.label*` branch-label backgrounds.

## Changes

- Updated `crates/merman-render/src/svg/parity/gitgraph.rs` so GitGraph CSS follows Mermaid 11.15's
  classic, redux, redux-color, and neo theme branches.
- Added scoped GitGraph gradient defs for `neo` / `neo-dark` when `themeVariables.useGradient` is
  enabled.
- Added renderer unit tests for classic/default, redux geometry, redux color, and neo gradient CSS.
- Added public `HeadlessRenderer` coverage proving `redux` and `neo` GitGraph theme signals reach
  final SVG output.

## Verification

- `cargo nextest run -p merman-render gitgraph_css`
- `cargo nextest run -p merman --features render --test theme_renderability_smoke gitgraph_official_themes_use_mermaid_11_15_color_generation`
- `cargo run -p xtask -- compare-gitgraph-svgs --check-dom --dom-mode parity --dom-decimals 3`

## Residual

This slice intentionally leaves GitGraph layout geometry, branch class indexing, commit-id
generation, font measurement, and root bounds unchanged. Any future changes to those surfaces need
separate source-backed evidence and family-level compare verification.
