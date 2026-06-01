# Theme Parity Refactor - TODO

Status: Complete
Last updated: 2026-06-01

## M0 - Scope And Evidence Freeze

- [x] TPR-010 [owner=planner] [deps=none] [scope=docs/workstreams/theme-parity]
  Goal: Freeze the theme parity problem, deletion policy, non-goals, and validation gates.
  Validation: DESIGN.md, TODO.md, TASKS.jsonl, CAMPAIGNS.jsonl, WORKSTREAM.json, and
  CONTEXT.jsonl exist and agree.
  Review: Confirm this lane is separate from generic renderer parity and Web/WASM playground work.
  Evidence: docs/workstreams/theme-parity/DESIGN.md
  Context: docs/workstreams/theme-parity/CONTEXT.jsonl
  Handoff: DONE. First executable slice is core default-theme expansion.

## M1 - Core Theme Expansion

- [x] TPR-020 [owner=codex] [deps=TPR-010] [scope=crates/merman-core/src/theme.rs]
  Goal: Expand `theme: default` through the same core theme pipeline as supported explicit themes.
  Validation: cargo fmt && cargo nextest run -p merman-core theme
  Review: User `themeVariables` overrides must keep precedence over derived defaults.
  Evidence: docs/workstreams/theme-parity/EVIDENCE_AND_GATES.md
  Context: docs/workstreams/theme-parity/CONTEXT.jsonl
  Handoff: DONE. Default theme expansion now populates Mermaid-compatible core variables, preserves
  user overrides, and falls back to default for unknown theme names. Render smoke for class/block/
  flowchart passed after updating scoped-color expectations to Mermaid's default `#131300`.

- [x] TPR-030 [owner=codex] [deps=TPR-020] [scope=crates/merman-core/src/theme.rs]
  Goal: Refactor duplicated theme preset helpers into shared derivation helpers without changing
  existing `base/dark/forest/neutral` behavior.
  Validation: cargo fmt && cargo nextest run -p merman-core theme
  Review: Keep exact upstream-derived string serialization for existing assertions.
  Evidence: docs/workstreams/theme-parity/EVIDENCE_AND_GATES.md
  Context: docs/workstreams/theme-parity/CONTEXT.jsonl
  Handoff: DONE. Shared theme variable map extraction, default Mermaid font-family value, and
  `mkBorder` HSL derivation helper now remove duplicated preset boilerplate while existing theme
  assertions remain unchanged.

## M2 - Render Resolver And CSS Cleanup

- [x] TPR-040 [owner=codex] [deps=TPR-020,TPR-030] [scope=crates/merman-render/src/svg/parity]
  Goal: Add a shared SVG theme resolver and migrate class/block/flowchart CSS callers first.
  Validation: cargo fmt && cargo nextest run -p merman-render block_svg class_svg flowchart_svg
  Review: Delete fallback code only when the resolver covers the same behavior.
  Evidence: docs/workstreams/theme-parity/EVIDENCE_AND_GATES.md
  Context: docs/workstreams/theme-parity/CONTEXT.jsonl
  Handoff: DONE. `SvgTheme` now centralizes common color/font reads for Class, Block, and
  Flowchart CSS; remaining diagram-specific reads stay in follow-up scope.

- [x] TPR-050 [owner=codex] [deps=TPR-040] [scope=crates/merman-render/src/svg/parity]
  Goal: Decide and implement the `themeCSS` contract: explicitly unsupported or scoped and
  sanitized.
  Validation: cargo fmt && cargo nextest run -p merman-render svg
  Review: Do not pass user CSS through unsafely.
  Evidence: docs/workstreams/theme-parity/EVIDENCE_AND_GATES.md
  Context: docs/workstreams/theme-parity/CONTEXT.jsonl
  Handoff: DONE. Mermaid `themeCSS` is supported as diagram-owned CSS, scoped to the root SVG id,
  with unsupported top-level at-rules dropped and Mermaid hash placeholders restored for CSS color
  values. `resvg_safe` remains responsible for raster-safety cleanup.

## M3 - API And Playground Theme Source

- [x] TPR-060 [owner=codex] [deps=TPR-020] [scope=crates/merman-wasm,platforms/web,playground]
  Goal: Make the supported theme list single-source across WASM, TypeScript, playground, and
  Mermaid compare mode.
  Validation: cargo check -p merman-wasm --target wasm32-unknown-unknown &&
  npm run build --prefix platforms/web && npm run build --prefix playground
  Review: Unknown shared-url themes should degrade to `default`.
  Evidence: docs/workstreams/theme-parity/EVIDENCE_AND_GATES.md
  Context: docs/workstreams/theme-parity/CONTEXT.jsonl
  Handoff: DONE. Core, bindings, WASM, `@merman/web`, playground store, toolbar, history, share
  URLs, and Mermaid compare mode now agree on `default/base/dark/forest/neutral`; unknown themes
  normalize to `default`.

## M4 - Follow-Up Split

- [ ] TPR-070 [owner=codex] [deps=TPR-040,TPR-060] [scope=fixtures,crates/merman-render]
  Goal: Add representative theme fixtures for flowchart, class, block, and ER covering
  `default/base/dark/forest/neutral` plus overrides.
  Validation: cargo fmt && cargo run -p xtask -- compare-all-svgs --check-dom --dom-decimals 3
  Review: Fixtures should prove behavior rather than merely update snapshots.
  Evidence: docs/workstreams/theme-parity/EVIDENCE_AND_GATES.md
  Context: docs/workstreams/theme-parity/CONTEXT.jsonl
  Handoff: SPLIT. Broad SVG fixture expansion and full ER/other-diagram theme resolver migration
  are follow-up parity work. This lane closes on targeted Rust, WASM, and frontend gates.

## M5 - Closeout

- [x] TPR-080 [owner=planner] [deps=TPR-040,TPR-050,TPR-060] [scope=docs/workstreams/theme-parity,CHANGELOG.md]
  Goal: Close the lane, update changelog, and split `neo/redux` theme support as a separate
  follow-on if still out of scope.
  Validation: documented narrower equivalent covering core theme tests, renderer SVG tests,
  WASM check, and frontend builds.
  Review: Workstream review has no blocking findings.
  Evidence: docs/workstreams/theme-parity/EVIDENCE_AND_GATES.md
  Context: docs/workstreams/theme-parity/CONTEXT.jsonl
  Handoff: DONE. `neo/redux` theme families, full fixture expansion, and remaining diagram-specific
  resolver migrations are explicit follow-ups.
