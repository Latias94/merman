---
type: Current State
status: active
---

# Current State

- Goal: 推进 `docs/plans/2026-06-24-003-refactor-mature-mermaid-lsp-roadmap-plan.md`，把 merman 做成成熟的全功能 Mermaid LSP 与 lint/analysis 产品级基建；允许破坏性内部重构，持续移除启发式解析路径，完成 parser-backed semantic facts、诊断、补全、hover、symbols、definition/references、rename、code action、semantic tokens、配置/包装、测试与文档门槛。
- Branch: feat/diagnostics-analysis-contract
- Last verified: 2026-06-25 (`cargo fmt --all --check`, focused `cargo test -p merman-bindings-core analyze_json_honors_lint --lib -- --nocapture`, focused `cargo test -p merman-bindings-core analyze_json_honors_lint_severity_overrides --lib -- --nocapture`; broader `cargo test -p merman-bindings-core --lib --tests` still hits the pre-existing `flowchart-elk` render regression)
- Done: `workspace/symbol` now reuses tracked outline projections from document snapshots, is advertised in `ServerCapabilities`, and is covered by focused capability and smoke regressions that verify cross-document lookup without a second parser path.
- In progress: the active roadmap is now the mature LSP/lint umbrella plan. U1 capability tracking is complete; U2 has completed the text-scan payload fallback shrink and the first sequence payload parser-fact deepening slice; U3 now preserves role-aware semantic items, uses them for hover, routes definition/references/prepareRename/rename through entity-only typed reference groups, and exports those semantic items as semantic-token surfaces. U6 now has full-document semantic tokens plus fix-backed quickfix code actions; U4 has its first source-backed fix rule and shared binding/LSP-side lint rule config, and it still needs broader lint descriptors, configuration, severity overrides, and more Mermaid-aware rules.
- Blocked: none
- Next action: continue U4 by turning the first lint rule into a broader configurable rule engine, then add more source-span-backed `DiagnosticFix` rules and continue U2 family fact deepening where rename/lint readiness is still partial.

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
