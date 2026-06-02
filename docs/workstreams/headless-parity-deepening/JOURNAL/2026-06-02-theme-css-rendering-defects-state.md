# HPD-080 State Theme CSS Rendering Defect Slice

Date: 2026-06-02

## Source Evidence

- Pinned Mermaid 11.15 source:
  `repo-ref/mermaid@41646dfd43ac83f001b03c70605feb036afae46d:packages/mermaid/src/diagrams/state/styles.js`
- Local implementation:
  `crates/merman-render/src/svg/parity/state/style.rs`

## Diagnosis

State diagrams were still emitting a mostly hardcoded stylesheet even though the pinned Mermaid
11.15 State style provider is theme-option driven. DOM structural parity can stay green while state
nodes, clusters, transitions, edge labels, notes, start/end markers, special-state nodes, and title
text use stale default-theme colors under dark or custom themes.

The old local CSS also used exact `#statediagram-barbEnd` marker selectors, but local marker ids are
prefixed as `<diagram>_stateDiagram-barbEnd`. Those exact selectors did not affect current output.

## Implementation

- Routed State CSS through the shared `SvgTheme` seam for font family/size and theme variables.
- Mapped Mermaid 11.15 State style options for root text, error colors, `lineColor`,
  `transitionColor`, `nodeBorder`, `stateLabelColor`, `mainBkg`, `background`, `altBackground`,
  `strokeWidth`, note colors, label backgrounds, transition labels, special state colors,
  end-state colors, state fill/border, and composite cluster colors.
- Replaced the inert exact barbEnd marker selector with source-backed suffix selectors that match
  the prefixed local marker id.
- Removed local dependency marker CSS because current State SVG output does not emit
  `dependencyStart` / `dependencyEnd` marker ids.
- Added a CSS seam test and a render-path test proving init `themeVariables` reach the State SVG
  stylesheet.

## Boundary

The upstream State style provider has neo cluster gradient/drop-shadow rules. Current local State
SVG output carries `data-look`, but does not emit the corresponding gradient/drop-shadow defs for
those rules. This slice therefore does not emit those neo rules; adding them now would advertise a
visual parity path that the local SVG cannot actually realize.

This slice does not address State layout or root-bounds residuals.

## Validation

- `cargo fmt -p merman-render`
- `cargo fmt --check -p merman-render`
- `cargo test -p merman-render state_css_honors_mermaid_11_15_theme_options`
- `cargo test -p merman-render state_svg_honors_mermaid_11_15_theme_css_options --test state_svg_test`
- `cargo test -p merman-render --test state_svg_test`
- `cargo run -p xtask -- compare-state-svgs --check-dom --dom-mode parity --dom-decimals 3`

## Next

Continue HPD-080 by scanning remaining supported diagrams with old hardcoded style providers,
especially diagrams whose text/labels can become unreadable under non-default themes.
