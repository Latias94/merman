---
type: Current State
status: active
---

# Current State

- Goal: 完成 `docs/plans/2026-06-24-001-feat-lsp-completion-foundations-plan.md` 对应的开发与无畏重构，持续打磨 `merman-lsp`、诊断投递、补全基建、共享结构层与基础导航能力，并在需要时做可回滚的增量提交，持续到 2026-06-24 10:00。
- Branch: feat/diagnostics-analysis-contract
- Last verified: 2026-06-24 (`cargo fmt --all`, `cargo check -p merman-lsp`, `cargo test -p merman-lsp --tests`)
- Done: `merman-lsp` now exists as a dedicated crate; diagnostics are published from `merman-analysis`; Markdown fence diagnostics are remapped; plain Mermaid documents also get a snapshot fence; shared LSP mapping helpers now live in `merman-analysis`; snapshot and completion both use shared completion context/index helpers; snapshot now carries diagram type and directive-prefix facts for each fence; completion now covers diagram headers, directions, operators, directives, shapes, and local node IDs with snapshot-derived replacement edits; `merman-analysis::document::analyze_document` now gives CLI lint and LSP one shared document-analysis seam; `server_smoke` proves initialize/open/change/save publish the current diagnostics version; `document_store` now validates both plain Mermaid and Markdown fence snapshot facts and proves newer versions replace older snapshots; hover/documentSymbol/definition/references/prepareRename/rename now share a fence-local structure/navigation layer; Python compatibility still resolves `python` to `python3` on macOS.
- In progress: shared LSP snapshot seam is now widening toward a more product-ready navigation surface, with the next decision focused on whether to formalize a new follow-up plan or continue the current fearless refactor slice in place.
- Blocked: none
- Next action: decide whether to keep extending the same navigation layer toward code actions/linked editing, or split out a new plan slice for lint/LSP polish.

# Citations

- [LSP completion foundations plan](../../plans/2026-06-24-001-feat-lsp-completion-foundations-plan.md)
- [Diagnostics-first session handoff](sessions/2026-06-23-diagnostics-first-analysis-plan-handoff.md)
- [Diagnostics-first analysis ADR](../../adr/0070-diagnostics-first-analysis-contract.md)
- [merman-analysis crate](../../../crates/merman-analysis/src/lib.rs)
- [FFI protocol](../../bindings/FFI_PROTOCOL.md)
- [FFI binding strategy ADR](../../adr/0066-ffi-binding-strategy.md)
- [WASM package surface ADR](../../adr/0069-wasm-package-surface-semantics.md)
