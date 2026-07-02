---
type: Session Handoff
status: active
related_plan: docs/plans/2026-06-23-002-refactor-diagnostics-first-analysis-plan.md
timestamp: 2026-06-23
tags:
  - diagnostics
  - lint
  - lsp
  - bindings
  - ffi
  - wasm
---

# Summary

The maintainer accepted an alpha-stage fearless refactor direction for `merman`: make diagnostics a
first-class analysis contract and treat rendering as only one consumer. The plan was prompted by
comparison with `mermaid-lint`, current binding validation limits, and the desire to prepare lint
and LSP foundations without adding Mermaid JS as a production fallback.

# Verified State

- A new plan exists at `docs/plans/2026-06-23-002-refactor-diagnostics-first-analysis-plan.md`.
- ADR 0070 exists and records the diagnostics-first analysis contract, including canonical JSON
  shape, alternatives, risks, and migration rules.
- FFI, UniFFI, options JSON, and README docs now describe `analyze_json` as the canonical
  diagnostics payload and `validate_json` as the compatibility projection.
- `crates/merman-analysis` exists and provides the first shared diagnostics payload, source
  descriptor, span, summary, and source-map/UTF-16 range helpers.
- `merman-analysis::Analyzer` now runs a render-free core parse pass and emits normalized
  `AnalysisPayload` diagnostics for no-diagram, parse, unsupported diagram, config parse,
  source-byte resource-limit, panic, Block width overflow, and GitGraph duplicate commit IDs.
- The binding-core, FFI, UniFFI, WASM, and platform wrapper surfaces now expose canonical `analyze_json` alongside compatibility `validate` paths.
- The browser package `@mermanjs/web` now exposes `analyze()` / `analyzeJson()` in checked-in `pkg` and `dist` artifacts after rebuilding with a matching `wasm-bindgen-cli`.
- `merman-cli` has render-oriented Markdown extraction, but not lint-oriented document diagnostics.
- Existing family warnings in Block and GitGraph are normalized by the analysis crate when they
  appear in parsed model `warnings`.
- Local shell compatibility was fixed so `python` maps to `python3` for non-interactive zsh.

# Open Threads

- U1 is complete at the documentation/protocol level; no Rust binding symbols were added yet.
- U2 is complete as a render-free foundation crate.
- U3 is implemented in `merman-analysis`; the crate now depends on `merman-core` behind forwarded
  default features and owns the first small semantic-warning registry.
- Keep parser-level exact spans progressive; whole-diagram and Markdown fence spans are acceptable for the first lint/LSP-ready payload.
- Thin wrapper documentation has been aligned; no blocking runtime drift remains for the analysis bridge.

# Next Action

Review the remaining thin wrappers and docs for any naming drift, then decide whether to keep the current bridge or continue the migration into package-layer polish.

# Citations

- [Diagnostics-first plan](../../../plans/2026-06-23-002-refactor-diagnostics-first-analysis-plan.md)
- [Diagnostics-first analysis ADR](../../../adr/0070-diagnostics-first-analysis-contract.md)
- [merman-analysis crate](../../../../crates/merman-analysis/src/lib.rs)
- [FFI protocol](../../../bindings/FFI_PROTOCOL.md)
- [FFI binding strategy ADR](../../../adr/0066-ffi-binding-strategy.md)
- [WASM package surface ADR](../../../adr/0069-wasm-package-surface-semantics.md)
