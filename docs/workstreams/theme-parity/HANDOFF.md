# Theme Parity Refactor - Handoff

Status: Complete
Last updated: 2026-06-01

## Current State

The lane is implemented and verified with targeted Rust, WASM, and frontend gates. TPR-020,
TPR-030, TPR-040, TPR-050, TPR-060, and closeout are complete. TPR-070 was split to follow-up
fixture/parity work.

## Key Findings

- Mermaid supports more theme presets than Merman currently exposes.
- Mermaid always expands default theme variables during initialization.
- Merman now expands `default`, `base`, `dark`, `forest`, and `neutral` in core and falls back to
  default for unknown theme names.
- Theme preset code now shares map extraction, default font-family construction, and `mkBorder`
  HSL derivation helpers.
- Class, Block, and Flowchart SVG CSS now use the shared `SvgTheme` resolver for common theme
  color/font values.
- Mermaid `themeCSS` is supported as scoped diagram-owned CSS after parity SVG rendering.
- Core, bindings, WASM, `@merman/web`, playground store, toolbar, share links, history, and Mermaid
  compare mode now agree on supported themes.
- Remaining diagram-specific theme reads, broad fixtures, and `neo/redux` theme families are
  intentionally split out.

## Follow-Ups

- Add representative theme parity fixtures for Flowchart, Class, Block, and ER across
  `default/base/dark/forest/neutral` plus overrides, then run:

```sh
cargo run -p xtask -- compare-all-svgs --check-dom --dom-decimals 3
```

- Continue migrating renderer-specific theme reads only where the shared resolver removes real
  duplication without changing SVG parity.
- Design `neo`/`redux` theme support separately.
