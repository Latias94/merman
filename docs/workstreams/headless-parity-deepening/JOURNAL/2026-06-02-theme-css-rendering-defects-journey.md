# HPD-080 Journey Theme CSS Visible Rendering Slice

Date: 2026-06-02

## Scope

Continue HPD-080 after Pie by auditing Journey's Mermaid 11.15 style provider. Journey is a high
value target because its visible task/section colors are controlled by CSS classes that can override
the SVG `fill` attributes emitted by the renderer.

## Source Checked

- Locked Mermaid source:
  `git -C repo-ref/mermaid show 41646dfd43ac83f001b03c70605feb036afae46d:packages/mermaid/src/diagrams/user-journey/styles.js`
- Local renderer:
  `crates/merman-render/src/svg/parity/journey.rs`

## Finding

The local Journey CSS still used fixed default colors for face fill, node fill/border, edge label
background, tooltip background/border, and all `.task-type-*` / `.section-type-*` fills. Because CSS
class rules override SVG presentation attributes, those fixed task/section rules can make configured
section colors appear not to work.

Journey also lacked the source-backed optional `.actor-N` theme rules. Local layout still emits
actor colors as presentation attributes, but Mermaid 11.15 can override them through
`themeVariables.actor0` through `actor5`.

## Change

- Journey CSS now reads Mermaid 11.15 theme variables:
  - `faceColor`
  - `mainBkg`
  - `nodeBorder`
  - `arrowheadColor`
  - `edgeLabelBackground`
  - `titleColor`
  - `tertiaryColor`
  - `border2`
  - `fillType0` through `fillType7`
  - optional `actor0` through `actor5`
- The generic `line` rule now follows upstream `textColor`; edge/flowchart link rules continue to
  use `lineColor`.
- Actor CSS is emitted only when a matching theme variable exists, preserving layout-derived default
  actor colors for ordinary Journey diagrams.

## Verification

- `cargo fmt -p merman-render`
- `cargo test -p merman-render journey_css_honors_mermaid_11_15_theme_options`
- `cargo run -p xtask -- compare-journey-svgs --check-dom --dom-mode parity --dom-decimals 3`

Journey structural SVG parity stayed green. This slice fixes source-backed theme/readability
behavior and does not claim root-bounds closure.
