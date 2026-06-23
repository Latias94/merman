---
type: Current State
status: active
---

# Current State

- Goal: 执行 diagnostics-first analysis core 重构：把 `validate` 从粗粒度通过/失败接口升级成跨 Rust、CLI、WASM、FFI、UniFFI、平台 wrapper 共用的诊断契约，为 `merman-cli lint` 和未来 LSP 打基础。
- Branch: feat/diagnostics-analysis-contract
- Last verified: 2026-06-23
- Done: diagnostics-first scope was confirmed; `docs/plans/2026-06-23-002-refactor-diagnostics-first-analysis-plan.md` was written; ADR 0070 and binding protocol docs now reserve diagnostics-first analysis as the canonical validation/lint contract; `merman-analysis` now provides the shared payload/source-map types plus a render-free `Analyzer` that maps no-diagram, parse, unsupported, config, source-byte resource-limit, panic, Block width warnings, and GitGraph duplicate commit warnings into `AnalysisPayload`; Python compatibility was fixed locally by mapping `python` to `python3` in `~/.zshenv`; previous Flowchart ELK source-backed probes remained the last code-verification state from 2026-06-18.
- In progress: migrate binding and CLI consumers onto the canonical analysis pipeline while keeping legacy validation projections compatible during alpha.
- Blocked: none
- Next action: implement U4 from the diagnostics-first plan: add `analyze_json` surfaces to `merman-bindings-core`, C FFI, UniFFI, WASM, and CLI-adjacent entry points, then project `validate_json` from the same payload.

# Citations

- [Diagnostics-first plan](../../plans/2026-06-23-002-refactor-diagnostics-first-analysis-plan.md)
- [Diagnostics-first session handoff](sessions/2026-06-23-diagnostics-first-analysis-plan-handoff.md)
- [Diagnostics-first analysis ADR](../../adr/0070-diagnostics-first-analysis-contract.md)
- [merman-analysis crate](../../../crates/merman-analysis/src/lib.rs)
- [FFI protocol](../../bindings/FFI_PROTOCOL.md)
- [FFI binding strategy ADR](../../adr/0066-ffi-binding-strategy.md)
- [WASM package surface ADR](../../adr/0069-wasm-package-surface-semantics.md)
