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
- FFI, UniFFI, options JSON, and README docs now describe `analyze_json` as the reserved canonical
  diagnostics payload and `validate_json` as the alpha compatibility projection.
- `crates/merman-analysis` exists and provides the first shared diagnostics payload, source
  descriptor, span, summary, and source-map/UTF-16 range helpers.
- The current validation path still returns a coarse payload from `merman-bindings-core`.
- `merman-cli` has render-oriented Markdown extraction, but not lint-oriented document diagnostics.
- Existing family warnings in Block and GitGraph show useful diagnostic data already exists, but it is not normalized.
- Local shell compatibility was fixed so `python` maps to `python3` for non-interactive zsh.

# Open Threads

- U1 is complete at the documentation/protocol level; no Rust binding symbols were added yet.
- U2 is complete as a render-free foundation crate. It does not yet depend on `merman-core` or run
  Mermaid parsing; U3 should add the analyzer pipeline.
- Decide during implementation whether `merman-analysis` owns only generic diagnostics or also owns the first rule registry.
- Keep parser-level exact spans progressive; whole-diagram and Markdown fence spans are acceptable for the first lint/LSP-ready payload.

# Next Action

Start from U3 in the diagnostics-first plan. Map core parse/no-diagram/unsupported/resource errors
and existing family warnings into `AnalysisPayload` before migrating binding functions.

# Citations

- [Diagnostics-first plan](../../../plans/2026-06-23-002-refactor-diagnostics-first-analysis-plan.md)
- [Diagnostics-first analysis ADR](../../../adr/0070-diagnostics-first-analysis-contract.md)
- [merman-analysis crate](../../../../crates/merman-analysis/src/lib.rs)
- [FFI protocol](../../../bindings/FFI_PROTOCOL.md)
- [FFI binding strategy ADR](../../../adr/0066-ffi-binding-strategy.md)
- [WASM package surface ADR](../../../adr/0069-wasm-package-surface-semantics.md)
