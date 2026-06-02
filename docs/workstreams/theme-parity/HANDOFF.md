# Theme Parity Refactor - Handoff

Status: Complete
Last updated: 2026-06-02

## Current State

The lane was reopened and completed for Mermaid 11.15 theme hardening. TPR-090 and TPR-100 are
complete in code and evidence: public supported-theme lists are back to Mermaid's official config
surface, snapshot-only `neo/redux*` theme names fall back to default, Flowchart neutral label
backgrounds now match Mermaid's white `edgeLabelBackground`, and plain-source rendering has
high-level tests for external theme selection and unsupported fallback.

There is no current active task in this lane. TPR-110 remains split follow-up work for
diagram-specific resolver migrations once per-diagram fixture evidence justifies it.

## Key Findings

- Mermaid 11.15 config theme selection exposes only `default`, `base`, `dark`, `forest`, and
  `neutral`.
- Mermaid's repository contains snapshot/theme-variable files for `neo/redux*`, but those names are
  not official config theme names in 11.15.
- Mermaid always expands default theme variables during initialization.
- Merman now expands `default`, `base`, `dark`, `forest`, and `neutral` in core and falls back to
  default for unknown theme names.
- Merman previously exposed snapshot-only `neo/redux*` names in core and `@merman/web`; that made
  playground compare mode disagree with Mermaid fallback behavior.
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
- Remaining diagram-specific theme reads, broad fixture tooling, and experimental `neo/redux` support are
  intentionally split out.

## Follow-Ups

- Optional broad fixture/tooling follow-up: add explicit xtask support for external-theme injection
  if fixture-level playground theme-selector comparisons become necessary, then run:

```sh
cargo nextest run -p merman-core theme
cargo nextest run -p merman-bindings-core supported_themes_exposes_core_theme_surface
cargo nextest run -p merman-render flowchart_svg
cargo nextest run -p merman --features render external_site_theme external_snapshot_only_theme
cargo run -p xtask -- compare-all-svgs --check-dom --dom-mode parity-root --dom-decimals 3 --filter theme
```

- TPR-110: continue migrating renderer-specific theme reads only where the shared resolver removes real
  duplication without changing SVG parity.
- Design experimental `neo`/`redux` support separately if Merman wants a non-Mermaid-compatible
  extended theme surface.
