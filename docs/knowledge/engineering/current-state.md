---
type: Current State
status: active
---

# Current State

- Goal: 推进 `docs/plans/2026-06-24-003-refactor-mature-mermaid-lsp-roadmap-plan.md`，把 merman 做成成熟的全功能 Mermaid LSP 与 lint/analysis 产品级基建；允许破坏性内部重构，持续移除启发式解析路径，完成 parser-backed semantic facts、诊断、补全、hover、symbols、definition/references、rename、code action、semantic tokens、配置/包装、测试与文档门槛。
- Branch: feat/diagnostics-analysis-contract
- Last verified: 2026-06-26 (`cargo test -p merman-analysis --lib`, `cargo check -p merman-analysis`, `cargo check -p merman-lsp`, `cargo fmt --all --check`, `git diff --check`; earlier same-day gates also covered focused LSP deprecated-config smoke tests, pull diagnostics, rule catalog/config schema exports, and analysis/binding/CLI/FFI/UniFFI/WASM/Web/Flutter/Apple catalog surfaces)
- Done: `workspace/symbol` now reuses tracked outline projections from document snapshots; semantic tokens have full/range/delta handlers over parser-backed snapshot tokens; configuration changes trigger `workspace/semanticTokens/refresh` when supported; U3 navigation and rename route through entity-only typed reference groups; directive-oriented completion contexts no longer fall back to generic `flowchart TD` on directive lines; flowchart emits parser-backed warning facts with stable source and fix spans for omitted header directions while render models still use effective `TB`; `merman-analysis` rule descriptors now carry `origin`, `default_profile`, and `evidence`, public config supports `core`/`recommended`/`strict` lint profiles plus explicit `enable_rules`, and authoring rules such as `merman.authoring.config.prefer_init_directive` and `merman.authoring.flowchart.explicit_direction` are recommended-profile hints rather than default core diagnostics. The governed lint rule catalog is now exported through Rust, CLI `lint-rules`, binding-core, C FFI, JNI, UniFFI, WASM, Web TS, Flutter/Dart, Apple Swift, and LSP custom request `merman/ruleCatalog`; LSP clients can also discover accepted analysis/lint settings through `merman/configSchema`. The core lint catalog now includes the Mermaid-backed compatibility warning `merman.compatibility.config.deprecated_flowchart_html_labels`, which reports deprecated directive usage of `flowchart.htmlLabels` without offering a quickfix until config rewrite spans are structurally safe. The directive/config source span scanner has been extracted from the rule implementation into `merman-analysis::source_directives`, giving future config lint, rewrite, completion, and hover work a reusable init-directive key-path locator instead of per-rule scanning.
- In progress: the active roadmap is now the mature LSP/lint umbrella plan. U1 capability tracking is complete. U2 has completed the text-scan payload fallback shrink, the first sequence payload parser-fact deepening slice, and directive-line completion fallback hardening. U3 is mature for parser-backed semantic items, hover, symbols, definition/references, prepareRename/rename, and semantic-token projection. U4 now has rule governance, stable descriptors with evidence references, unknown-rule internal gap diagnostics, configurable core diagnostics including a source-backed deprecated config rule, shared source-directive config key span infrastructure, authoring quickfixes behind explicit opt-in, cross-binding rule registry/export surfaces, and editor-agnostic LSP discovery requests for both rule metadata and configuration schema; it still needs a larger Mermaid-evidence-backed lint catalog plus more fix-backed rule coverage. U6 has code actions, semantic tokens, completion item resolve, and standard `textDocument/diagnostic` / `workspace/diagnostic` pull support alongside the existing push path.
- Blocked: none
- Next action: continue U4 by broadening Mermaid-evidence-backed config and compatibility rules on top of `source_directives`, then add structurally safe fix/rewrite support where source spans and formatting rules are mature enough.

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
