# HPD-080 Journey Arrowhead Visible Signal Audit

Date: 2026-06-02

## Problem

The public dark-theme renderability smoke counted Journey `themeVariables.arrowheadColor` as an
expected visible color. A source audit showed that this was too optimistic:

- Mermaid 11.15 `user-journey/styles.js` emits `.arrowheadPath { fill: arrowheadColor }`.
- Mermaid 11.15 `user-journey/svgDraw.js` creates the Journey marker path without an
  `arrowheadPath` class.
- Local Journey SVG mirrors that marker shape and also lacks the class.

That means the CSS token is source-backed but currently inert for Journey marker DOM. Counting it
as a visible smoke signal was a measurement bug.

## Change

- Removed `arrowheadColor` from `crates/merman/tests/theme_renderability_smoke.rs` Journey input and
  expected visible colors.
- Updated `THEME_RENDERING_COVERAGE.md` so Journey's arrowhead rule is tracked as an
  upstream-inert provider rule, not visible renderability coverage.

No renderer class was added. Adding `.arrowheadPath` would make the color visible, but it would also
introduce a DOM difference from pinned Mermaid output. This pass only tightens the evidence model.

## Source Evidence

- `repo-ref/mermaid/packages/mermaid/src/diagrams/user-journey/styles.js`
- `repo-ref/mermaid/packages/mermaid/src/diagrams/user-journey/svgDraw.js`
- `crates/merman-render/src/svg/parity/journey.rs`

## Verification

- `$env:RUSTFLAGS='-C linker=rust-lld'; cargo nextest run -p merman --features render --test theme_renderability_smoke`
