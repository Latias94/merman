# HPD-080 Requirement Visible Signal Audit

Date: 2026-06-03

## Question

The public dark-theme smoke still counted Requirement theme colors by checking that configured
tokens appeared somewhere in the SVG. That was too weak: several colors came only from
`requirement/styles.js` provider rules whose selectors do not match current Requirement DOM.

## Source And Render Evidence

- Pinned Mermaid 11.15 `requirement/styles.js` emits `.reqBox`, `.reqTitle`, `.reqLabel`,
  `.reqLabelBox`, `.relationshipLine`, `.relationshipLabel`, `.edgeLabel`, `.divider`, and
  `.labelBkg` rules.
- Pinned Mermaid 11.15 `requirementBox.ts` emits current node DOM through generic node surfaces:
  `data-look`, `basic label-container outer-path`, `label`, `divider`, and foreignObject label
  wrappers.
- A fresh Mermaid CLI render for a custom `look: neo` sample was generated in
  `target/compare/requirement_theme_audit_upstream.svg`. It confirmed that `.relationshipLine`,
  `.labelBkg`, `data-look="neo"`, and `outer-path` are consumed by current DOM, while `.reqBox`,
  `.reqTitle`, `.reqLabelBox`, and `.relationshipLabel` remain provider-only for the ordinary
  Requirement render path.

## Decision

Keep emitting Mermaid 11.15 provider CSS, but do not count provider-only colors as public visible
renderability signals.

The compact public Requirement smoke now uses a relationship so it can assert real line and edge
label surfaces. It counts:

- `relationColor` on relationship lines/markers,
- `edgeLabelBackground` on current edge label background DOM,
- `nodeBorder` / `strokeWidth` on `look: neo` node path CSS.

It no longer counts Requirement box/title/relationship-label colors when no matching current DOM is
emitted.

## Implementation

- Requirement CSS now emits the source-backed `look: neo` node path selector for current local DOM.
- Requirement SVG emits `data-look="neo"`, `outer-path`, and `divider` only for the `neo` path, so
  default Requirement fixture DOM remains unchanged.
- The public renderability smoke includes a focused Requirement test that proves `.reqBox` is still
  a provider rule but not a current DOM-visible signal.

## Verification

- `cargo nextest run -p merman-render requirement_css_honors_mermaid_11_15_theme_options`
- `cargo nextest run -p merman --features render --test theme_renderability_smoke`
- `cargo run -p xtask -- compare-requirement-svgs --check-dom --dom-mode parity --dom-decimals 3`
