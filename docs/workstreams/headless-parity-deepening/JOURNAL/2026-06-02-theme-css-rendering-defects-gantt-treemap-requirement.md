# 2026-06-02 - Gantt Treemap Requirement Theme CSS Defects

## Context

HPD-080 continues to prioritize functional renderability over fine root residuals. After the first
theme CSS slice fixed Kanban, Packet, Sankey, C4, and GitGraph, the next audit pass looked for
supported diagrams whose Mermaid 11.15 source has a diagram-specific style provider while the local
renderer still emitted fixed or stale CSS.

This pass found three source-backed defects:

- Gantt discarded `effective_config` during CSS emission and kept a fixed default stylesheet.
- Treemap emitted fixed black title/label/value CSS and ignored Mermaid 11.15 `treemap.*` style
  options.
- Requirement still mirrored an older hardcoded stylesheet and ignored Mermaid 11.15 requirement
  theme variables for node, relationship, and label colors.

## Source Checks

Pinned Mermaid source commit:

- `41646dfd43ac83f001b03c70605feb036afae46d`

Checked style providers:

- `packages/mermaid/src/diagrams/gantt/styles.js`
- `packages/mermaid/src/diagrams/treemap/styles.ts`
- `packages/mermaid/src/diagrams/requirement/styles.js`

## Outcome

- Gantt CSS now reads Mermaid 11.15 Gantt theme variables for section backgrounds, grid/today
  colors, task text, task bars, active/done/critical states, vertical markers, and title text.
- Gantt now emits the Mermaid 11.15 outside done/doneCrit text rules so labels that move outside
  task bars use the outside contrast color instead of the bar text color.
- Treemap CSS now maps Mermaid 11.15 `treemap.*` options and falls back through theme title/text
  colors for title, labels, values, leaf styles, and section styles.
- Requirement CSS now reads Mermaid 11.15 requirement theme variables for requirement boxes, title
  and body labels, relationship lines/labels, edge-label backgrounds, node text, and divider colors.
- The Requirement pass intentionally did not add the upstream `[data-look][data-color-id]` color
  scale rules because the current local renderer does not emit those attributes; adding inert CSS
  would not improve renderability.

## Verification

- `cargo test -p merman-render css_honors_mermaid_11_15`
- `cargo run -p xtask -- compare-gantt-svgs --check-dom --dom-mode parity --dom-decimals 3`
- `cargo run -p xtask -- compare-treemap-svgs --check-dom --dom-mode parity --dom-decimals 3`
- `cargo run -p xtask -- compare-requirement-svgs --check-dom --dom-mode parity --dom-decimals 3`

## Follow-up

Continue HPD-080 by auditing remaining supported style providers with visible-risk focus:

- Mindmap has more complex section, icon, span, and neo/redux theme behavior than the current local
  CSS.
- Journey still has several hardcoded task/actor colors that should be checked against Mermaid
  11.15 options before changing.
- ER and Pie still contain older hardcoded color paths; fix only if source evidence shows a visible
  readability gap rather than a harmless style parity tail.
