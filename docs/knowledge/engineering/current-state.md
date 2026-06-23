---
type: Current State
status: active
---

# Current State

- Goal: 执行 diagnostics-first analysis core 重构：把 `validate` 从粗粒度通过/失败接口升级成跨 Rust、CLI、WASM、FFI、UniFFI、平台 wrapper 共用的诊断契约，为 `merman-cli lint` 和未来 LSP 打基础。
- Branch: main
- Last verified: 2026-06-23
- Done: diagnostics-first scope was confirmed; `docs/plans/2026-06-23-002-refactor-diagnostics-first-analysis-plan.md` was written; ADR 0070 and binding protocol docs now reserve diagnostics-first analysis as the canonical validation/lint contract; `merman-analysis` now provides the first shared payload/source-map types with schema and UTF-16 range tests; Python compatibility was fixed locally by mapping `python` to `python3` in `~/.zshenv`; previous Flowchart ELK source-backed probes remained the last code-verification state from 2026-06-18.
- In progress: build the Rust analysis pipeline on top of the new payload/source-map crate before migrating bindings.
- Blocked: none
- Next action: implement U3 from the diagnostics-first plan: map no-diagram, parse, unsupported, resource, and existing family warnings into `AnalysisPayload` without invoking render/layout by default.

# Citations

- [Diagnostics-first plan](../../plans/2026-06-23-002-refactor-diagnostics-first-analysis-plan.md)
- [Diagnostics-first session handoff](sessions/2026-06-23-diagnostics-first-analysis-plan-handoff.md)
- [Diagnostics-first analysis ADR](../../adr/0070-diagnostics-first-analysis-contract.md)
- [merman-analysis crate](../../../crates/merman-analysis/src/lib.rs)
- [FFI protocol](../../bindings/FFI_PROTOCOL.md)
- [FFI binding strategy ADR](../../adr/0066-ffi-binding-strategy.md)
- [WASM package surface ADR](../../adr/0069-wasm-package-surface-semantics.md)
