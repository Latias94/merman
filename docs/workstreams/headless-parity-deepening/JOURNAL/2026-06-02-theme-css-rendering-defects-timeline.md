# HPD-080 Timeline Disabled Theme CSS

Date: 2026-06-02

## Source Evidence

- Pinned Mermaid 11.15 commit:
  `41646dfd43ac83f001b03c70605feb036afae46d`
- Audited source:
  `packages/mermaid/src/diagrams/timeline/styles.js`
- Mermaid 11.15 Timeline emits `.disabled` fills from `options.tertiaryColor` and disabled text
  fill from `options.clusterBorder`, with `lightgray` / `#efefef` only as missing-option fallbacks.

## Change

- Timeline CSS now reads `themeVariables.tertiaryColor` and `themeVariables.clusterBorder` through
  the existing theme lookup path instead of hardcoding fallback colors.
- Added a render-path regression for custom disabled node/text theme colors.
- Left redux/neo gradient and drop-shadow selectors out of this slice because local Timeline SVG
  does not currently emit the required `data-look` attributes or gradient/drop-shadow defs.

## Verification

- `cargo fmt -p merman-render`
- `cargo fmt --check -p merman-render`
- `cargo test -p merman-render timeline_svg_honors_mermaid_11_15_disabled_theme_colors --test timeline_svg_test`
- `cargo run -p xtask -- compare-timeline-svgs --check-dom --dom-mode parity --dom-decimals 3`
- `git diff --check`

