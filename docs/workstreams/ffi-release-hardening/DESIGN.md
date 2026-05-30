# FFI Release Hardening

Status: Closed with packaging concern
Last updated: 2026-05-30

## Why This Lane Exists

`ffi-api` delivered the first C ABI release candidate. Before building UniFFI or host-specific
packages on top of it, the C surface needs release-grade checks: package viability, a realistic C
consumer smoke test, and a clear documentation entry point.

## Relevant Authority

- ADRs:
  - `docs/adr/0066-ffi-binding-strategy.md`
  - `docs/adr/0050-release-quality-gates.md`
- Existing docs:
  - `docs/bindings/FFI_PROTOCOL.md`
  - `README.md`
- Related workstreams:
  - `docs/workstreams/ffi-api`

## Problem

The first FFI crate compiles and has header coverage, but downstream users still need proof that the
crate can be packaged, linked from C, and discovered from the main project docs.

## Target State

- `cargo package -p merman-ffi --allow-dirty` succeeds or any packaging blocker is fixed.
- The FFI test suite includes a real C source that calls exported functions, reads result buffers,
  and frees them through `merman_buffer_free`.
- The README points non-Rust consumers to the C ABI crate and protocol.
- Evidence records the focused gates and any broader gates intentionally deferred.

## In Scope

- `crates/merman-ffi` packaging metadata, tests, examples, and README-related documentation.
- `docs/bindings/FFI_PROTOCOL.md` clarifications needed by C consumers.
- Workstream docs and evidence for this hardening pass.

## Out Of Scope

- UniFFI crate generation.
- Raster output ABI.
- ASCII renderer workstreams.
- Full workspace release publishing.

## Starting Assumptions

| Assumption | Confidence | Evidence | Consequence if wrong |
| --- | --- | --- | --- |
| The C ABI functions are already behaviorally correct for SVG/parse/layout. | High | `docs/workstreams/ffi-api/EVIDENCE_AND_GATES.md` | This lane shifts from hardening to bug fixing and may need a narrower follow-on. |
| `merman-ffi` should remain the canonical low-level binding surface. | High | ADR 0066 | UniFFI work should wait until this surface has stable checks. |
| Package validation may expose missing publish metadata or examples. | Medium | New crate added recently | Fix package blockers before adding higher-level bindings. |

## Architecture Direction

Keep all unsafe ABI ownership inside `merman-ffi`. The release hardening tests should exercise the
public header and exported symbols without duplicating renderer logic in tests. Documentation should
describe the C ABI as the stable low-level bridge and keep UniFFI framed as a future high-level
facade over the same behavior.

## Closeout Condition

This lane can close when:

- the package and targeted FFI test gates pass,
- the realistic C smoke test proves call/free behavior through the public header,
- docs expose the C ABI entry point,
- and UniFFI/raster remain split as explicit follow-ons.

Closeout note: the code, docs, and focused FFI gates are complete. Full `cargo package -p
merman-ffi --allow-dirty` is blocked by crates.io release ordering because the published
`merman-render 0.6.0` package does not contain the local `ratex-math` feature required by this
workspace state. The package file list gate passes, and full package verification should be rerun
after the next workspace crate version is published in dependency order.
