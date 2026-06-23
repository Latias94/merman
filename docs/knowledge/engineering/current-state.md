---
type: Current State
status: active
---

# Current State

- Goal: 完成 `docs/plans/2026-06-24-001-feat-lsp-completion-foundations-plan.md` 对应的开发与无畏重构，持续打磨 `merman-lsp`、诊断投递和补全基建，并在需要时做可回滚的增量提交，持续到 2026-06-24 10:00。
- Branch: feat/diagnostics-analysis-contract
- Last verified: 2026-06-24
- Done: `merman-lsp` now exists as a dedicated crate; diagnostics are published from `merman-analysis`; Markdown fence diagnostics are remapped; plain Mermaid documents also get a snapshot fence; shared LSP mapping helpers now live in `merman-analysis`; snapshot and completion both use shared completion context/index helpers; completion can surface local node IDs; `cargo fmt --all --check`, `cargo check -p merman-lsp -p merman-analysis`, `cargo test -p merman-lsp --tests`, `cargo test -p merman-analysis --test lsp_positions -- --nocapture`, `cargo test -p merman-lsp --test server_smoke -- --nocapture`, `cargo test -p merman-lsp --test completion -- --nocapture`, `cargo test -p merman-lsp --test diagnostics -- --nocapture`, and `cargo test -p merman-lsp --test document_store -- --nocapture` passed; Python compatibility still resolves `python` to `python3` on macOS.
- In progress: deepen the snapshot/completion seam further and decide whether to add lint-oriented entry points next.
- Blocked: none
- Next action: decide whether the next slice should be lint plumbing, richer completion metadata, or a first ADR/docs pass for the LSP seam.

# Citations

- [LSP completion foundations plan](../../plans/2026-06-24-001-feat-lsp-completion-foundations-plan.md)
- [Diagnostics-first session handoff](sessions/2026-06-23-diagnostics-first-analysis-plan-handoff.md)
- [Diagnostics-first analysis ADR](../../adr/0070-diagnostics-first-analysis-contract.md)
- [merman-analysis crate](../../../crates/merman-analysis/src/lib.rs)
- [FFI protocol](../../bindings/FFI_PROTOCOL.md)
- [FFI binding strategy ADR](../../adr/0066-ffi-binding-strategy.md)
- [WASM package surface ADR](../../adr/0069-wasm-package-surface-semantics.md)
