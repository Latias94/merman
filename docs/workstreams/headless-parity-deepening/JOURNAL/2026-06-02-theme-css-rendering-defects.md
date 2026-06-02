# 2026-06-02 - Mermaid 11.15 Theme CSS Rendering Defects

## Context

A user-provided Kanban example rendered structurally but looked visually broken because the SVG did
not emit Mermaid 11.15 Kanban diagram theme CSS. That exposed a broader risk class: diagrams can
pass DOM-oriented structural parity while still rendering unreadable output when diagram-specific
theme rules are missing.

This is a higher-priority defect class than fine-grained root viewport residuals. Invisible labels,
black cards, black branch label blocks, and missing branch colors are functional rendering failures,
not merely parity deltas.

## Source Checks

Pinned Mermaid source commit:

- `41646dfd43ac83f001b03c70605feb036afae46d`

Checked diagram style providers:

- `packages/mermaid/src/diagrams/kanban/styles.ts`
- `packages/mermaid/src/diagrams/packet/styles.ts`
- `packages/mermaid/src/diagrams/sankey/styles.js`
- `packages/mermaid/src/diagrams/c4/styles.js`
- `packages/mermaid/src/diagrams/git/styles.js`

## Outcome

- Kanban now emits Mermaid 11.15 section/ticket/icon/label theme CSS, fixing dark cards and hidden
  labels in the metadata example.
- Packet now maps Mermaid 11.15 `PacketStyleOptions` from `packet.*` config into the emitted CSS
  instead of hardcoding defaults.
- Sankey now uses config-aware info CSS and Mermaid 11.15 Sankey label/node/link style options for
  font, outlined label background, and foreground text color.
- C4 now emits `.person` from `themeVariables.personBorder/personBkg` and uses config-aware base
  CSS.
- GitGraph now emits Mermaid 11.15 classic/default per-branch theme rules:
  `.branch-labelN`, `.commitN`, `.commit-highlightN`, `.labelN`, `.arrowN`, plus merge/reverse/
  highlight inner colors. The user-provided three-branch merge graph now shows readable branch
  labels and colored branch/merge paths.

## Verification

- `cargo test -p merman-render kanban`
- `cargo test -p merman-render packet_css_honors_mermaid_11_15_packet_style_options`
- `cargo test -p merman-render sankey_css_honors_mermaid_11_15_theme_options`
- `cargo test -p merman-render c4_css_honors_mermaid_11_15_person_theme_options`
- `cargo test -p merman-render gitgraph_css_includes_mermaid_11_15_branch_theme_rules`
- `cargo run -p xtask -- compare-kanban-svgs --check-dom --dom-mode parity --dom-decimals 3`
- `cargo run -p xtask -- compare-packet-svgs --check-dom --dom-mode parity --dom-decimals 3`
- `cargo run -p xtask -- compare-sankey-svgs --check-dom --dom-mode parity --dom-decimals 3`
- `cargo run -p xtask -- compare-c4-svgs --check-dom --dom-mode parity --dom-decimals 3`
- `cargo run -p xtask -- compare-gitgraph-svgs --check-dom --dom-mode parity --dom-decimals 3`
- `cargo fmt --check -p merman-render`
- `git diff --check`

Additional manual rendering checks:

- `target/compare/kanban_user_metadata.fixed.png`
- `target/compare/gitgraph_user_merge.png`

## Follow-up Policy

Prioritize functional rendering defects over numeric parity residuals. If an output is blank,
unreadable, has hidden text, or loses semantic color cues, fix that first using source-backed
Mermaid style/rendering rules. Root viewport and browser-measurement residuals remain important but
should not consume attention ahead of visible rendering breakage.
