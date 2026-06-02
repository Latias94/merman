# HPD-080 Class Note Theme CSS Rendering Defect

Date: 2026-06-02

## Source Evidence

- Pinned Mermaid 11.15 commit:
  `41646dfd43ac83f001b03c70605feb036afae46d`
- `packages/mermaid/src/diagrams/class/classDb.ts` creates note nodes with `cssStyles` containing:
  - `fill: ${config.themeVariables.noteBkgColor}`
  - `stroke: ${config.themeVariables.noteBorderColor}`
- `packages/mermaid/src/diagrams/class/styles.js` maps `.noteLabel` text to
  `options.noteTextColor`.

## Defect

Local Class CSS already read `themeVariables.noteTextColor`, but Class note shapes still hardcoded
the default note fill and border:

- `fill="#fff5ad"`
- `stroke="#aaaa33"`
- inline `style="fill:#fff5ad !important;stroke:#aaaa33 !important"`

That meant custom `themeVariables.noteBkgColor` and `noteBorderColor` were parsed and expanded in
`effective_config`, but ignored when the final SVG note body was emitted.

## Fix

- `crates/merman-render/src/svg/parity/class/note.rs`
  - Reads `noteBkgColor` and `noteBorderColor` through the shared theme lookup path.
  - Applies the configured colors to both note body paths and the inline `!important` style.
  - Covers both HTML-label and `htmlLabels:false` Class note render branches.
- `crates/merman-render/tests/class_svg_test.rs`
  - Added a render-path regression test for custom note fill, border, and text colors across both
    Class note label modes.

## Verification

- `cargo fmt -p merman-render`
- `cargo fmt --check -p merman-render`
- `cargo test -p merman-render class_svg_honors_configured_note_theme_colors --test class_svg_test`
- `cargo test -p merman-render --test class_svg_test`
- `cargo run -p xtask -- compare-class-svgs --check-dom --dom-mode parity --dom-decimals 3`

## Residuals

This slice does not alter Class layout, namespace cluster inline styles, root-bounds behavior, or
browser text measurement. It only fixes source-backed note color emission for the current local
Class SVG surface.
