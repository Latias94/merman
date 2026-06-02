# Theme Parity Refactor - TODO

Status: Complete
Last updated: 2026-06-02

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

## M6 - Post-11.15 Theme Surface Hardening

- [x] TPR-090 [owner=codex] [deps=TPR-060] [scope=crates/merman-core/src/theme.rs,crates/merman-render/src/svg/parity/flowchart/css.rs,platforms/web/src/index.ts]
  Goal: Re-align public supported themes with Mermaid 11.15's official config surface and make
  snapshot-only `neo/redux*` names fall back to the default theme.
  Validation: cargo nextest run -p merman-core theme &&
  cargo nextest run -p merman-bindings-core supported_themes_exposes_core_theme_surface &&
  cargo nextest run -p merman-render flowchart_svg &&
  npm run build:ts --prefix platforms/web &&
  cargo run -p xtask -- compare-all-svgs --check-dom --dom-mode parity-root --dom-decimals 3 --filter theme --diagram flowchart --diagram xychart --diagram gitgraph --diagram pie --diagram gantt --diagram architecture --diagram quadrantchart --diagram class --diagram sequence --diagram radar --diagram er --diagram timeline --diagram packet --diagram treemap
  Review: Do not expose snapshot-derived theme names unless Mermaid exposes them through config
  theme selection or Merman deliberately labels them experimental.
  Evidence: docs/workstreams/theme-parity/EVIDENCE_AND_GATES.md
  Context: docs/workstreams/theme-parity/CONTEXT.jsonl
  Handoff: DONE. Core, bindings, and `@merman/web` now expose only
  `default/base/dark/forest/neutral`; unsupported snapshot-only names use default theme variables.
  Flowchart neutral `edgeLabelBackground: white` now serializes to Mermaid's white label background.

- [x] TPR-100 [owner=codex] [deps=TPR-070,TPR-090] [scope=crates/merman,crates/merman-render]
  Goal: Add representative ordinary-source theme-selector parity coverage for supported external
  themes and unsupported snapshot-only fallback behavior without introducing a frontend test
  runner.
  Validation: cargo nextest run -p merman-core theme &&
  cargo nextest run -p merman-bindings-core supported_themes_exposes_core_theme_surface &&
  cargo nextest run -p merman-render flowchart_svg &&
  cargo nextest run -p merman-render neutral_named_white_edge_label_background_fades_to_white unknown_edge_label_background_keeps_mermaid_default_fade &&
  cargo nextest run -p merman --features render external_site_theme external_snapshot_only_theme
  Review: Coverage should prove actual external site-config behavior for plain sources, not only
  directive handling inside diagrams.
  Evidence: docs/workstreams/theme-parity/EVIDENCE_AND_GATES.md
  Context: docs/workstreams/theme-parity/CONTEXT.jsonl
  Handoff: DONE. Plain-source rendering now has high-level tests for external neutral theme
  application and external snapshot-only fallback; Flowchart CSS has direct tests for white and
  unknown label-background fade behavior.

- [ ] TPR-110 [owner=codex] [deps=TPR-100] [scope=crates/merman-render/src/svg/parity]
  Goal: Continue migrating remaining diagram-specific theme reads to shared resolver helpers only
  where the migration deletes real duplication without changing SVG parity.
  Validation: cargo nextest run -p merman-render svg &&
  cargo run -p xtask -- compare-all-svgs --check-dom --dom-mode parity-root --dom-decimals 3 --filter theme
  Review: Prefer local diagram semantics over premature abstraction when a value is layout-specific.
  Evidence: docs/workstreams/theme-parity/EVIDENCE_AND_GATES.md
  Context: docs/workstreams/theme-parity/CONTEXT.jsonl
  Handoff: SPLIT. Remaining resolver migrations are useful only with fixture evidence per diagram;
  this M6 closeout does not force abstraction without a parity failure.
