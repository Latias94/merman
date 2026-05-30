# UniFFI Bindings

Status: Active
Last updated: 2026-05-30

## Why This Lane Exists

`merman-ffi` now provides a hardened C ABI for SVG, semantic JSON, and layout JSON. Swift, Kotlin,
Python, and Ruby consumers can get a more idiomatic API through UniFFI, but UniFFI must reuse the
same safe behavior as the C ABI instead of becoming a parallel renderer pipeline.

## Relevant Authority

- ADRs:
  - `docs/adr/0066-ffi-binding-strategy.md`
- Existing docs:
  - `docs/bindings/FFI_PROTOCOL.md`
  - `crates/merman-ffi/README.md`
- Related workstreams:
  - `docs/workstreams/ffi-api`
  - `docs/workstreams/ffi-release-hardening`
- Local reference:
  - `repo-ref/RaTeX/docs/binding-architecture.md`
  - `repo-ref/RaTeX/crates/ratex-ffi`

## Problem

The current FFI implementation owns options parsing, renderer construction, error mapping, and
byte-output behavior inside `merman-ffi`. Adding UniFFI directly on top of that crate would either
duplicate the same logic or make generated bindings depend on unsafe C ABI details.

## Target State

- A safe shared binding facade exists for Mermaid source plus options JSON in, bytes/string payloads
  out.
- `merman-ffi` delegates to that facade without changing its public C ABI.
- A minimal `merman-uniffi` crate exposes idiomatic methods for `render_svg`, `parse_json`, and
  `layout_json`.
- Generated binding smoke checks prove UniFFI scaffolding builds at least one target binding or
  records a precise toolchain blocker.
- The C ABI remains the canonical low-level contract.

## In Scope

- A new safe facade crate or module, preferably `crates/merman-bindings-core`.
- Refactoring `merman-ffi` to reuse the facade.
- A minimal `crates/merman-uniffi` crate with UniFFI 0.31.1.
- Tests for parity between C ABI behavior and the shared facade.
- Workstream docs and evidence.

## Out Of Scope

- iOS XCFramework packaging.
- Android Gradle/JNI packaging.
- Flutter, Node, React Native, or JVM wrapper packages.
- Raster byte APIs.
- Replacing or weakening the C ABI.
- ASCII renderer changes.

## Starting Assumptions

| Assumption | Confidence | Evidence | Consequence if wrong |
| --- | --- | --- | --- |
| UniFFI 0.31.1 is the current crates.io version. | High | `cargo search uniffi --limit 3`, `cargo info uniffi@0.31.1` on 2026-05-30 | Workstream dependency metadata must be updated before implementation. |
| A safe facade can own options parsing and error mapping without exposing unsafe ABI types. | High | Existing `merman-ffi/src/lib.rs` isolates those concerns today | If not, split smaller adapter functions before adding UniFFI. |
| UniFFI should expose strings/bytes and not Rust renderer structs. | High | ADR 0066 and C ABI protocol | Generated APIs stay stable while renderer internals keep evolving. |
| Full crate packaging may still be blocked by workspace publish order. | Medium | `ffi-release-hardening` package concern | Treat release packaging as a separate lane from generated binding proof. |

## Architecture Direction

Create a safe binding layer below all external bindings:

```text
merman-core / merman-render / merman
        ↓
merman-bindings-core   (safe facade: options, errors, byte outputs)
        ↓
  ┌───────────────┬────────────────┐
  │ merman-ffi    │ merman-uniffi   │
  │ C ABI         │ generated APIs  │
  └───────────────┴────────────────┘
```

The facade should use Rust enums/structs internally but keep its public behavior aligned to the C
ABI protocol: tolerant options JSON, stable result codes, JSON error payloads where needed, and
feature-gated RaTeX handling. UniFFI can then expose ergonomic exceptions or result objects while
the underlying behavior stays shared.

## Closeout Condition

This lane can close when:

- the shared facade exists and is tested,
- `merman-ffi` delegates to it without public C ABI drift,
- `merman-uniffi` builds and exposes the minimal SVG/parse/layout surface,
- generated binding smoke evidence is recorded,
- and platform packaging is split into follow-on lanes.
