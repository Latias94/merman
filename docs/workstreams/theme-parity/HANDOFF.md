# Theme Parity Refactor - Handoff

Status: Complete
Last updated: 2026-06-02

## Current State

The lane was reopened and completed for Mermaid 11.15 theme hardening, then corrected after a fresh
source check of the pinned 11.15 theme registry. Public supported-theme lists now include Mermaid's
official config surface:
`default/base/dark/forest/neutral/neo/neo-dark/redux/redux-dark/redux-color/redux-dark-color`.
Flowchart neutral label backgrounds match Mermaid's white `edgeLabelBackground`, and plain-source
rendering has high-level tests for external theme selection and unknown-theme fallback.

There is no current active task in this lane. TPR-110 remains split follow-up work for
diagram-specific resolver migrations once per-diagram fixture evidence justifies it.

## Key Findings

- Mermaid 11.15 config theme selection exposes `default`, `base`, `dark`, `forest`, `neutral`,
  `neo`, `neo-dark`, `redux`, `redux-dark`, `redux-color`, and `redux-dark-color`.
- Mermaid always expands default theme variables during initialization.
- Merman now expands all official Mermaid 11.15 theme names in core and falls back to default for
  unknown theme names.
- The extended `neo/redux*` theme names currently use the generated 11.15 upstream theme-variable
  snapshots as their default expansion. Explicit `themeVariables` still override direct keys, but
  full source-equivalent override derivation for those families remains a follow-up audit.
- Flowchart neutral label background needed named-color parsing for `white`.
- `HeadlessRenderer::with_site_config` now has regression coverage for plain source with external
  theme selection, matching the playground theme-selector path more closely than directive-only
  fixtures.
- Theme preset code now shares map extraction, default font-family construction, and `mkBorder`
  HSL derivation helpers.
- Class, Block, and Flowchart SVG CSS now use the shared `SvgTheme` resolver for common theme
  color/font values.
- Mermaid `themeCSS` is supported as scoped diagram-owned CSS after parity SVG rendering.
- Core, bindings, WASM, `@merman/web`, playground store, toolbar, share links, history, and Mermaid
  compare mode now agree on supported themes.
- Remaining diagram-specific theme reads, broad fixture tooling, and exact `neo/redux` override
  derivation are intentionally split out.

## Follow-Ups

- Optional broad fixture/tooling follow-up: add explicit xtask support for external-theme injection
  if fixture-level playground theme-selector comparisons become necessary, then run:

```sh
cargo nextest run -p merman-core theme
cargo nextest run -p merman-bindings-core supported_themes_exposes_core_theme_surface
cargo nextest run -p merman-render flowchart_svg
cargo test -p merman external_ --features render
cargo run -p xtask -- compare-all-svgs --check-dom --dom-mode parity-root --dom-decimals 3 --filter theme
```

- TPR-110: continue migrating renderer-specific theme reads only where the shared resolver removes real
  duplication without changing SVG parity.
- Audit `neo`/`redux` source-derived override behavior if default snapshot expansion is not enough
  for host theme customization.
