---
type: Current State
status: active
---

# Current State

- Goal: 推进 `docs/plans/2026-06-24-003-refactor-mature-mermaid-lsp-roadmap-plan.md`，把 merman 做成成熟的全功能 Mermaid LSP 与 lint/analysis 产品级基建；允许破坏性内部重构，持续移除启发式解析路径，完成 parser-backed semantic facts、诊断、补全、hover、symbols、definition/references、rename、code action、semantic tokens、配置/包装、测试与文档门槛。
- Branch: feat/diagnostics-analysis-contract
- Last verified: 2026-06-25 (`cargo fmt --all --check`, `cargo nextest run -p merman-analysis -p merman-lsp --no-fail-fast`)
- Done: `workspace/symbol` now reuses tracked outline projections from document snapshots, is advertised in `ServerCapabilities`, and is covered by focused capability and smoke regressions that verify cross-document lookup without a second parser path; `merman-analysis` now also carries stable rule descriptors for core parse/resource/compatibility/internal diagnostics, and the shared rule config can disable `no_diagram` and source-byte-limit diagnostics without breaking the rest of the analysis pipeline; the shared lint/config contract now rejects unknown or internal rule ids on JSON and CLI surfaces instead of silently ignoring them, and exposes a public configurable-rule registry view for future completion surfaces; CLI lint now proves the same shared rule config contract at the command surface; LSP initialization now proves the same disablement contract at the server surface, and LSP smoke now also proves the shared severity override contract for source-byte-limit diagnostics on initialize and configuration-change paths; semantic tokens now have range and delta handlers over the same parser-backed snapshot tokens, configuration changes now trigger `workspace/semanticTokens/refresh` when the client advertises refresh support, and the server now caches previous semantic-token state so delta responses can reuse prior result ids; the text-scan fallback now also treats `init`, `initialize`, and `wrap` directive prefixes as non-symbol directives so they do not leak into node IDs or outline items; directive-oriented completion contexts now avoid the generic `flowchart TD` fallback, so directive lines such as `classDef` and `click` stay on directive completions instead of being treated like new diagram headers.
- In progress: the active roadmap is now the mature LSP/lint umbrella plan. U1 capability tracking is complete; U2 has completed the text-scan payload fallback shrink, the first sequence payload parser-fact deepening slice, and the directive-line completion fallback hardening; its fallback directive leakage guard is now tighter for `init`/`initialize`/`wrap` and no longer falls back to `flowchart TD` on directive lines. U3 now preserves role-aware semantic items, uses them for hover, routes definition/references/prepareRename/rename through entity-only typed reference groups, and exports those semantic items as semantic-token surfaces. U6 now has full-document semantic tokens, range projection, delta projection, and fix-backed quickfix code actions, and now refreshes semantic tokens on supported configuration changes; U4 now has broader stable rule descriptors, a public configurable-rule registry for lint/config surfaces, and config disablement for core analysis diagnostics, and it still needs more Mermaid-aware lint rules, severity coverage, and binding/CLI/LSP alignment. Unknown warning fact rule ids now surface as explicit internal contract gaps instead of collapsing into a generic semantic warning bucket.
- Blocked: none
- Next action: continue U4 by broadening the remaining core diagnostics and Mermaid-aware lint rules under the shared rule contract, then continue U2 family fact deepening where rename/lint readiness is still partial.

# Citations

- [LSP completion foundations plan](../../plans/2026-06-24-001-feat-lsp-completion-foundations-plan.md)
- [Parser and semantic seam plan](../../plans/2026-06-24-002-refactor-parser-semantic-seam-plan.md)
- [Mature Mermaid LSP roadmap plan](../../plans/2026-06-24-003-refactor-mature-mermaid-lsp-roadmap-plan.md)
- [Diagnostics-first session handoff](sessions/2026-06-23-diagnostics-first-analysis-plan-handoff.md)
- [Diagnostics-first analysis ADR](../../adr/0070-diagnostics-first-analysis-contract.md)
- [Editor parser/semantic seam ADR](../../adr/0071-editor-parser-semantic-seam.md)
- [merman-analysis crate](../../../crates/merman-analysis/src/lib.rs)
- [FFI protocol](../../bindings/FFI_PROTOCOL.md)
- [FFI binding strategy ADR](../../adr/0066-ffi-binding-strategy.md)
- [WASM package surface ADR](../../adr/0069-wasm-package-surface-semantics.md)
