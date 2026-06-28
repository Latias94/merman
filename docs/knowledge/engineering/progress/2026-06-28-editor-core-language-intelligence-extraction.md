---
type: "Work Progress"
title: "Editor core language intelligence extraction"
description: "Progress note for extracting shared Mermaid editor intelligence into merman-editor-core and wiring LSP/WASM/Playground adapters."
timestamp: 2026-06-28T19:15:00Z
tags: ["merman", "editor-core", "lsp", "wasm", "playground"]
source_session: "019f09c5-f8d2-7441-9553-650dcb43a38c"
---

# Summary

`merman-editor-core` now owns protocol-neutral document snapshots, completions, diagnostics,
hover, symbols, navigation, rename, code-action metadata, and semantic-token selection. `merman-lsp`
is reduced to LSP projection and lifecycle code, while `merman-wasm` and `@mermanjs/web` expose
stateless browser editor queries that the playground Monaco editor consumes.

# Details

- Removed the `merman-analysis` LSP projection module and direct `lsp-types`/`tower-lsp` leakage.
- Added `crates/merman-editor-core` with focused tests for workspace snapshots, completion,
  diagnostics, structure/navigation/rename, and semantic tokens.
- Moved LSP diagnostics projection into `merman-lsp::diagnostics`; completion, structure, and
  semantic-token modules now adapt editor-core results.
- Added browser editor APIs in `merman-wasm` / `@mermanjs/web`:
  diagnostics, code actions, completions, hover, document/workspace symbols, definition,
  references, prepare/rename, semantic-token legend, and semantic tokens.
- Wired playground Monaco providers for diagnostics, completion, hover, code actions, document
  symbols, definition, references, rename, and semantic tokens with static completions as fallback.
- Fixed deprecated `flowchart.htmlLabels` quickfix behavior so init-to-frontmatter migration
  preserves effective config, while the dedicated deprecated-key quickfix also supports
  `config.flowchart.htmlLabels` wrapper directives.

# Verification

Passing checks included:

- `cargo test -p merman-editor-core`
- `cargo test -p merman-analysis`
- `cargo test -p merman-lsp --lib`
- `cargo test -p merman-lsp --test document_store --test completion --test diagnostics --test server_smoke`
- `cargo test -p merman-wasm`
- `cargo check -p merman-wasm --target wasm32-unknown-unknown`
- `npm run build --prefix platforms/web`
- `npm run smoke --prefix platforms/web`
- `npm run build --prefix playground`
- `npm run verify:dist --prefix playground`
- `npm run prepack --prefix platforms/web`
- `cargo fmt --all --check`
- `cargo test --no-run -p merman-editor-core -p merman-analysis -p merman-lsp -p merman-wasm`
- `git diff --check`

The published browser-full package now includes the new editor surface. The measured
`platforms/web/pkg/merman_wasm_bg.wasm` size is:

- raw actual `6649826`
- gzip actual `2532845`, budget `2850000`
- brotli actual `1874082`, budget `2100000`

The release package budget in `docs/release/WASM_SIZE_BUDGETS.json` was raised as a regression guard
for this intentional default API expansion.

Current-session Rust test reruns hit a macOS binary loading/signing-layer stall before the Rust test
harness started: `cargo test -p merman-editor-core --test completion ...` left the test binary parked
at `_dyld_start`, and `codesign -dv target/debug/deps/completion-...` also hung. Earlier same-session
Rust package checks listed above had already passed before this host-level stall. A later
`cargo test --no-run` compile-only pass for the touched Rust crates completed successfully, so the
remaining limitation is test binary execution on this host session rather than compilation.

# Next Action

Complete the final handoff. If more Rust test confidence is needed on a fresh host session, rerun
`cargo nextest run -p merman-editor-core -p merman-analysis -p merman-lsp` or the equivalent
`cargo test` package split.

# Citations

- `docs/plans/2026-06-28-001-refactor-editor-core-language-intelligence-plan.md`
