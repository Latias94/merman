# HPD-080 Pie Theme CSS Visible Rendering Slice

Date: 2026-06-02

## Scope

Continue HPD-080's supported-diagram style audit after Mindmap. Pie was selected because local SVG
rendering still called `pie_css(diagram_id)` without `effective_config`, while Mermaid 11.15 has a
dedicated `pieStyles.ts` provider for visible text and stroke styling.

## Source Checked

- Locked Mermaid source:
  `git -C repo-ref/mermaid show 41646dfd43ac83f001b03c70605feb036afae46d:packages/mermaid/src/diagrams/pie/pieStyles.ts`
- Locked Mermaid theme defaults:
  `git -C repo-ref/mermaid show 41646dfd43ac83f001b03c70605feb036afae46d:packages/mermaid/src/themes/theme-default.js`
- Local renderer:
  `crates/merman-render/src/svg/parity/css.rs`
  `crates/merman-render/src/svg/parity/pie.rs`

## Finding

The local Pie CSS used fixed default colors and sizes:

- black slice stroke and outer stroke,
- `0.7` slice opacity,
- black title/legend text,
- `#333` slice labels,
- default font family.

That ignored Mermaid 11.15 theme variables such as `pieTitleTextColor`,
`pieSectionTextColor`, and `pieLegendTextColor`. A dark/custom theme could therefore produce
unreadable labels even when the SVG DOM structure matched.

## Change

- `pie_css` now accepts `effective_config`.
- Pie CSS now reads Mermaid 11.15 `themeVariables` for stroke, opacity, title text, slice labels,
  and legend text.
- Default values remain aligned with the upstream default theme (`25px` title, `17px` labels,
  black strokes, `0.7` opacity).
- Removed the unused fixed `info_css(...)` wrapper after Pie moved to config-aware CSS parts.

## Verification

- `cargo fmt -p merman-render`
- `cargo test -p merman-render pie_css_honors_mermaid_11_15_theme_options`
- `cargo run -p xtask -- compare-pie-svgs --check-dom --dom-mode parity --dom-decimals 3`

Pie structural SVG parity stayed green. This is a theme/readability fix, not a root-bounds claim.
