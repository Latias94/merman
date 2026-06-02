# HPD-080 ER Theme CSS Visible Rendering Slice

## Context

Continue HPD-080's visible-rendering-defect audit after Journey. ER was selected because local
`er_css` still mirrored an older hardcoded stylesheet while Mermaid 11.15 has a dedicated
source-backed style provider for entity fill/stroke, labels, relationship lines, markers, and
edge-label backgrounds.

## Source Evidence

- Locked Mermaid source:
  `git -C repo-ref/mermaid show 41646dfd43ac83f001b03c70605feb036afae46d:packages/mermaid/src/diagrams/er/styles.ts`
- Local renderer:
  `crates/merman-render/src/svg/parity/css.rs`
  `crates/merman-render/src/svg/parity/er.rs`

## Finding

Local ER CSS hardcoded default-theme colors such as `#ECECFF`, `#9370DB`, and `#333333` for
entities, nodes, relationship lines, labels, and markers. A custom or non-default theme could
therefore render structurally valid ER diagrams with stale colors and wrong text/edge contrast.

Mermaid 11.15 instead derives those rules from `themeVariables.mainBkg`, `nodeBorder`,
`nodeTextColor`, `textColor`, `lineColor`, `tertiaryColor`, `edgeLabelBackground`, optional
`erEdgeLabelBackground`, and `strokeWidth` when `look: neo`.

## Implementation

- ER CSS now uses the shared `SvgTheme` access path for theme variables, font family, theme name,
  and look.
- Entity boxes, node shapes, relationship lines, markers, root text fill, error styles, edge-label
  backgrounds, edge-label text, and generic labels now consume Mermaid 11.15 theme variables.
- Added `css_rgba_fade(...)` as a narrow render-side utility for the ER `fade(tertiaryColor, 0.5)`
  rule. It uses the existing `svgtypes` dependency and returns `None` for unresolved CSS values
  rather than pretending to support browser/runtime expressions.
- Preserved the upstream exact default `tertiaryColor` fade string for default-theme ER output; for
  custom parseable CSS colors, the helper produces stable `rgba(...)` output.

## Boundary

- Mermaid's ER color-theme `genColor(...)` rules target `[data-look][data-color-id]` nodes. Current
  local ER SVGs emit `data-look="classic"` but do not emit `data-color-id`, so those rules were not
  added in this slice.
- Mermaid's `[data-look=neo].labelBkg` rule was also not emitted because current local ER label
  background elements do not carry `data-look`; adding it would be inert CSS rather than a visible
  rendering fix.

## Verification

- `cargo fmt -p merman-render`
- `cargo fmt --check -p merman-render`
- `cargo test -p merman-render er_css_honors_mermaid_11_15_theme_options`
- `cargo test -p merman-render css_rgba_fade_parses_css_colors`
- `cargo run -p xtask -- compare-er-svgs --check-dom --dom-mode parity --dom-decimals 3`
