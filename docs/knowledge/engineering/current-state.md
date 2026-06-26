---
type: Current State
status: active
---

# Current State

- Goal: 推进 `docs/plans/2026-06-24-003-refactor-mature-mermaid-lsp-roadmap-plan.md`，把 merman 做成成熟的全功能 Mermaid LSP 与 lint/analysis 产品级基建；允许破坏性内部重构，持续移除启发式解析路径，完成 parser-backed semantic facts、诊断、补全、hover、symbols、definition/references、rename、code action、semantic tokens、配置/包装、测试与文档门槛。
- Branch: feat/diagnostics-analysis-contract
- Last verified: 2026-06-26 (`cargo fmt --all`, `cargo fmt --all --check`, `cargo test -p merman-analysis --lib --tests`, `cargo test -p merman-core flowchart --lib`, `cargo test -p merman-cli cli_lint --test cli_compat`, `cargo test -p merman-lsp code_actions --lib`, `cargo test -p merman-lsp --lib --tests`, `python3 /Users/frankorz/.codex/skills/engineering-wiki-memory/scripts/wiki_memory.py validate --root docs/knowledge/engineering`, `git diff --check`; broader previous gate on the same date also passed `cargo nextest run -p merman-core --no-fail-fast` and `cargo nextest run -p merman-analysis --no-fail-fast`)
- Done: `workspace/symbol` now reuses tracked outline projections from document snapshots; semantic tokens have full/range/delta handlers over parser-backed snapshot tokens; configuration changes trigger `workspace/semanticTokens/refresh` when supported; U3 navigation and rename route through entity-only typed reference groups; directive-oriented completion contexts no longer fall back to generic `flowchart TD` on directive lines; flowchart emits parser-backed warning facts with stable source and fix spans for omitted header directions while render models still use effective `TB`; `merman-analysis` rule descriptors now carry `origin` and `default_profile`, public config supports `core`/`recommended`/`strict` lint profiles plus explicit `enable_rules`, and authoring rules such as `merman.authoring.config.prefer_init_directive` and `merman.authoring.flowchart.explicit_direction` are recommended-profile hints rather than default core diagnostics.
- In progress: the active roadmap is now the mature LSP/lint umbrella plan. U1 capability tracking is complete. U2 has completed the text-scan payload fallback shrink, the first sequence payload parser-fact deepening slice, and directive-line completion fallback hardening. U3 is mature for parser-backed semantic items, hover, symbols, definition/references, prepareRename/rename, and semantic-token projection. U4 now has rule governance, stable descriptors, unknown-rule internal gap diagnostics, configurable core diagnostics, and authoring quickfixes behind explicit opt-in; it still needs a larger Mermaid-evidence-backed lint catalog, user-facing rule registry/export surfaces, and broader binding/CLI/LSP alignment. U6 has code actions and semantic tokens wired, but needs more fix-producing rules and completion-resolution polish.
- Blocked: none
- Next action: continue U4 by broadening the remaining core diagnostics and Mermaid-aware lint rules under the shared rule contract, then continue U2 family fact deepening where rename/lint readiness is still partial.

# Citations

- [LSP completion foundations plan](../../plans/2026-06-24-001-feat-lsp-completion-foundations-plan.md)
- [Parser and semantic seam plan](../../plans/2026-06-24-002-refactor-parser-semantic-seam-plan.md)
- [Mature Mermaid LSP roadmap plan](../../plans/2026-06-24-003-refactor-mature-mermaid-lsp-roadmap-plan.md)
- [Diagnostics-first session handoff](sessions/2026-06-23-diagnostics-first-analysis-plan-handoff.md)
- [Diagnostics-first analysis ADR](../../adr/0070-diagnostics-first-analysis-contract.md)
- [Editor parser/semantic seam ADR](../../adr/0071-editor-parser-semantic-seam.md)
- [Lint rule governance ADR](../../adr/0072-lint-rule-governance.md)
- [merman-analysis crate](../../../crates/merman-analysis/src/lib.rs)
- [FFI protocol](../../bindings/FFI_PROTOCOL.md)
- [FFI binding strategy ADR](../../adr/0066-ffi-binding-strategy.md)
- [WASM package surface ADR](../../adr/0069-wasm-package-surface-semantics.md)
