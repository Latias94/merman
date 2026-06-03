# HPD-080 - Packet And Sankey Visible Signal Boundary

Task: HPD-080 visible rendering defect triage.

## Question

Do Packet and Sankey public theme-smoke colors have matching current DOM, or are they only proving
stylesheet presence?

## Source Audit

- Pinned Mermaid 11.15 `packet/styles.ts` styles `.packetByte`, `.packetByte.start`,
  `.packetByte.end`, `.packetLabel`, `.packetTitle`, and `.packetBlock`.
- Pinned `packet/renderer.ts` emits those same classes on byte counters, labels, title, and block
  rectangles.
- Pinned `sankey/styles.js` styles `.sankey-label-bg`, `.sankey-label-fg`, `.node rect`, and
  `.link`.
- Pinned `sankeyRenderer.ts` emits outlined label foreground/background text classes when
  `labelStyle: "outlined"`, node rect fills, and link groups.

## Outcome

No production renderer defect was found. Added
`packet_and_sankey_theme_smoke_count_dom_consumed_selectors_as_visible` in
`crates/merman/tests/theme_renderability_smoke.rs`.

The test proves Packet configured colors only with matching `.packetBlock`, `.packetLabel`,
`.packetByte.start`, `.packetByte.end`, and `.packetTitle` DOM. It also proves Sankey outlined
label colors only with `.sankey-label-bg` / `.sankey-label-fg`, configured node colors with node
rect fills, and link styling with `.link` groups.

Updated the theme coverage ledger to record these DOM-consumed visible-signal boundaries.

## Verification

- `cargo fmt` - passed.
- `cargo fmt --check` - passed.
- `cargo nextest run -p merman --features render --test theme_renderability_smoke packet_and_sankey_theme_smoke_count_dom_consumed_selectors_as_visible` -
  passed, 1 test run.
- `cargo nextest run -p merman --features render --test theme_renderability_smoke` - passed, 8
  tests run.
- `Get-Content ... CONTEXT.jsonl | ConvertFrom-Json` - passed, 358 JSONL lines parsed.
- `git diff --check` - passed with only the existing `CONTEXT.jsonl` LF/CRLF working-copy warning.

## Residual

Packet and Sankey remain covered for the current implemented DOM. Future color assertions should
stay selector/DOM-backed rather than checking that a token merely appears somewhere in the SVG.
