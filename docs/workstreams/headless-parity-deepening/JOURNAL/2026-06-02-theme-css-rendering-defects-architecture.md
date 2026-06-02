# HPD-080 Architecture Theme CSS Rendering Defect

Date: 2026-06-02

## Source Evidence

- Pinned Mermaid 11.15 commit:
  `41646dfd43ac83f001b03c70605feb036afae46d`
- `packages/mermaid/src/diagrams/architecture/architectureStyles.ts` emits:
  - `.edge { stroke-width: ${options.archEdgeWidth}; stroke: ${options.archEdgeColor}; }`
  - `.arrow { fill: ${options.archEdgeArrowColor}; }`
  - `.node-bkg { stroke: ${options.archGroupBorderColor}; stroke-width: ${options.archGroupBorderWidth}; }`
- `packages/mermaid/src/diagrams/architecture/architectureTypes.ts` defines those values as
  `ArchitectureStyleOptions`.

## Defect

Local `architecture_css_with_config(...)` emitted Architecture edge/group CSS from generic
`lineColor`, `primaryBorderColor`, and hardcoded `3` / `2px` widths. Core theme expansion already
populated `themeVariables.archEdgeColor`, `archEdgeArrowColor`, `archEdgeWidth`,
`archGroupBorderColor`, and `archGroupBorderWidth`, so custom Architecture styling was parsed but
not reflected in final SVG CSS.

This is a visible HPD-080 defect, not a numeric root-bounds residual: DOM structure can remain
green while configured Architecture edge/group colors and widths are silently ignored.

## Fix

- `crates/merman-render/src/svg/parity/css.rs`
  - `architecture_css_with_config(...)` now reads `arch*` theme variables.
  - Width variables use `config_css_number_or_string(...)` so numeric and CSS-string tokens preserve
    Mermaid-style CSS spelling.
- `crates/merman-render/tests/architecture_svg_test.rs`
  - Added a render-path regression test proving custom `arch*` variables reach the emitted SVG
    stylesheet and generic fallback CSS is not emitted for that case.

## Verification

- `cargo fmt -p merman-render`
- `cargo fmt --check -p merman-render`
- `cargo test -p merman-render architecture_css_with_config_honors_font_and_theme_colors`
- `cargo test -p merman-render architecture_svg_honors_mermaid_11_15_style_theme_variables --test architecture_svg_test`
- `cargo test -p merman-render --test architecture_svg_test`
- `cargo run -p xtask -- compare-architecture-svgs --check-dom --dom-mode parity --dom-decimals 3`

## Residuals

This slice does not change Architecture layout, manatee/Cytoscape phase modeling, root-bounds
estimation, or the known 26 Architecture `parity-root` residuals. It only fixes source-backed
Mermaid 11.15 Architecture CSS emission for the current local SVG surface.
