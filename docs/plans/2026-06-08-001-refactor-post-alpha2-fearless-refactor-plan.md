---
title: Post Alpha.2 Fearless Refactor
type: refactor
status: active
date: 2026-06-08
execution: code
---

# Post Alpha.2 Fearless Refactor

## Summary

Prepare the next alpha by removing shallow release-facing seams that survived `0.7.0-alpha.2`. The work proceeds through focused, independently landable refactor slices recorded in `docs/workstreams/post-alpha2-fearless-refactor`.

---

## Problem Frame

The alpha line can still change internal architecture without creating long-term compatibility debt. Binding render requests, diagram family facts, headless operations, presentation theme roles, admission inventory, and xtask parity harness code have already been deepened; the remaining risk is duplicated harness policy and implementation-era surfaces becoming de facto contracts.

---

## Requirements

**Architecture**

- R1. Each refactor must move caller knowledge behind a deeper Module Interface or delete the shallow surface outright.
- R2. Public binding ABI, FFI package contracts, and options JSON must stay stable unless a new ADR records the contract change.
- R3. Headless render entry points must keep canonical render paths distinct from expert parse/layout/debug paths.

**Parity**

- R4. Mermaid parity behavior must stay source-backed and must not hide semantic drift behind broad comparator normalization.
- R5. Browser-dependent residuals may be documented or narrowly accepted only when the acceptance is fixture-specific and guarded by focused evidence.
- R6. Xtask compare/admission policies must be projected from one inventory or adapter source rather than duplicated across command harnesses.

**Execution**

- R7. Each slice must have focused nextest/check coverage before commit, with wider gates added when a shared contract changes.
- R8. Workstream evidence must name the changed Module, the focused gates that passed, and any remaining follow-on.

---

## Key Technical Decisions

- KTD1. Keep the workstream as the execution ledger: `docs/workstreams/post-alpha2-fearless-refactor/TODO.md`, `EVIDENCE_AND_GATES.md`, and `WORKSTREAM.json` carry task status and gate evidence, while this CE plan records the durable implementation strategy.
- KTD2. Prefer deep Module Interfaces over helper churn: a new helper earns its keep only when it reduces call-site policy knowledge, centralizes validation, or gives tests a better surface.
- KTD3. Treat compare harness cleanup as plumbing unless proven otherwise: DOM signature mode, normalization, and residual policy semantics must not change during PA2R-080.
- KTD4. Preserve repo-local release posture: use focused `cargo nextest` filters first, run `cargo fmt --all --check`, and commit only the files touched by the slice.

---

## Implementation Units

### U1. Compare harness invocation Module

- **Goal:** Move `compare-all-svgs` common compare argument construction, mode-suffixed report path naming, flowchart-only text measurement, and optional root-report argument policy behind a small internal invocation Module.
- **Files:** `crates/xtask/src/cmd/compare/all.rs`; optionally `crates/xtask/src/cmd/compare/mod.rs` if the Module deserves its own file.
- **Patterns:** Follow the existing `compare::diagrams` adapter registry and `compare::root` policy helpers: one caller-facing method should produce the per-diagram args plus the report path needed for bounded failure summaries.
- **Test Scenarios:** Build args with DOM mode, decimals, filter, mode-suffixed report path, flowchart text measurer, root-report top/all/none cases, and a non-root-report family.
- **Verification:** `cargo nextest run -p xtask compare_all`; representative `cargo run -p xtask -- compare-all-svgs --diagram info --filter upstream_info_spec --check-dom --dom-mode parity --dom-decimals 3`; `cargo check -p xtask`; `cargo fmt --all --check`.

### U2. Compare harness residual policy review

- **Goal:** Reassess whether `RootParityResidualPolicy` still belongs in `compare/all.rs` after U1 or should move into a root-parity policy Module.
- **Files:** `crates/xtask/src/cmd/compare/all.rs`; possibly `crates/xtask/src/cmd/compare/root.rs`.
- **Patterns:** Keep fixture-specific acceptance records narrow and make missing expected residuals fail loudly.
- **Test Scenarios:** Existing root parity policy tests must continue to reject changed residual values and preserve unexpected mismatches.
- **Verification:** `cargo nextest run -p xtask root_parity_policy`; `cargo check -p xtask`.

### U3. Next highest-leverage slice selection

- **Goal:** After PA2R-080, choose the next refactor by deletion-test leverage rather than module size.
- **Files:** `docs/workstreams/post-alpha2-fearless-refactor/TODO.md`; candidate source files from the next selected lane.
- **Patterns:** Prefer stale pass-through code, duplicate policy, or untested cross-module knowledge before style-only cleanup.
- **Test Scenarios:** The selected slice must define tests against the Module Interface before private implementation details.
- **Verification:** Workstream evidence records the selected task and focused gate set before implementation.

---

## Scope Boundaries

- Do not change public package names, FFI package metadata, or release workflow semantics in this refactor lane.
- Do not rewrite DOM normalization or comparator signatures while cleaning harness plumbing.
- Do not reopen the closed `docs/workstreams/merman-0-7-architecture-deepening` lane; reference it as history only.
- Do not edit `repo-ref/` except for read-only comparison.

---

## System-Wide Impact

This plan affects release confidence rather than end-user features. The expected impact is smaller public surface area, less duplicated parity policy, and clearer evidence for why alpha-era internals are safe to keep evolving.

---

## Risks & Dependencies

- Compare harness helpers can accidentally change CLI behavior if argument order or optional flag policy drifts; tests should assert exact argument vectors.
- Root parity residual handling can become too broad if accepted fragments are generalized; keep records fixture-specific.
- Focused tests may miss cross-family regressions; representative `compare-all-svgs` runs are required before committing harness changes.

---

## Sources

- `docs/workstreams/post-alpha2-fearless-refactor/DESIGN.md`
- `docs/workstreams/post-alpha2-fearless-refactor/TODO.md`
- `docs/workstreams/post-alpha2-fearless-refactor/EVIDENCE_AND_GATES.md`
- `docs/adr/0014-upstream-parity-policy.md`
- `docs/adr/0066-ffi-binding-strategy.md`
- `docs/adr/0068-render-side-presentation-theme-view.md`
