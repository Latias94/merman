# HPD-080 - Timeline Redux Visible DOM Theme Consumption

Task: HPD-080 visible rendering defect triage.

## Question

Does Timeline `theme: "redux"` consume `nodeBorder`, `mainBkg`, `strokeWidth`, and `fontWeight` on
the node and activity-line DOM users actually see, or does local output keep the classic Timeline
CSS/DOM branch?

## Source Audit

- Pinned Mermaid 11.15 `packages/mermaid/src/diagrams/timeline/styles.js` switches `redux*`
  themes into `genReduxSections(...)`.
- That branch styles current section node paths with `mainBkg` / `nodeBorder` / `strokeWidth` and
  styles `.lineWrapper line` with `nodeBorder` / `strokeWidth`.
- Pinned `svgDraw.js` also changes redux node geometry to sharp-corner paths and does not emit the
  classic bottom divider line.
- Focused Mermaid CLI evidence with `theme: "redux"`, `nodeBorder: #38bdf8`, and
  `strokeWidth: 5` showed upstream final CSS:
  `.section--1 ... { fill:#111827; stroke:#38bdf8; stroke-width:5; ... }` and
  `.lineWrapper line { stroke:#38bdf8; stroke-width:5; }`, while the line DOM still carries
  presentational `stroke="black"` / `stroke-width="2"` attributes that CSS overrides.

## Outcome

- Updated local Timeline CSS generation to choose the Mermaid 11.15 redux branch when the active
  theme contains `redux`.
- Routed redux node path fill/stroke/stroke-width, label fill/font-weight, and `lineWrapper line`
  stroke/stroke-width through `SvgTheme`.
- Updated redux node rendering to emit sharp-corner paths, omit the classic `node-line-*` divider
  DOM, and use the upstream text offsets for normal and event nodes.
- Added focused renderer coverage and public `HeadlessRenderer` smoke coverage for the current
  node and line DOM.

## Verification

- `cargo nextest run -p merman-render --test timeline_svg_test` - passed, `2` tests run.
- `cargo nextest run -p merman --features render --test theme_renderability_smoke` - passed, `11`
  tests run.
- `cargo run -p xtask -- compare-timeline-svgs --check-dom --dom-mode parity --dom-decimals 3 --out target\compare\timeline_report_parity_after_hpd080_redux_theme.md` -
  passed, structural Timeline DOM parity stayed green.
- `cargo run -p xtask -- compare-timeline-svgs --check-dom --dom-mode parity-root --dom-decimals 3 --out target\compare\timeline_report_parity_root_after_hpd080_redux_theme.md` -
  expected-failed on the known `3` Timeline max-width/root residual rows only.
- `cargo fmt -p merman-render -p merman --check` - passed.

## Residual

- Timeline arrowhead marker path remains unstyled exactly like the current pinned Mermaid 11.15
  output for this branch; this slice fixes the visible node/line CSS branch, not marker-color
  policy.
- Timeline root residuals remain the existing `3` max-width rows documented by HPD-030/HPD-080.
