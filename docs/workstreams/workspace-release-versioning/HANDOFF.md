# Workspace Release Versioning — Handoff

Status: Active
Last updated: 2026-05-30

## Current State

`WRV-010` is complete. The binding ABI work is not the blocker; release packaging is.

Confirmed:

- `merman-render` packages successfully from local source.
- `merman-bindings-core` package file list is correct.
- `merman-bindings-core` full package verification is blocked because crates.io
  `merman-render 0.6.0` lacks the local `ratex-math` feature.
- `merman-ffi` full package verification is blocked because `merman-bindings-core` has not been
  published.
- Since crates.io versions are immutable, the next publishable workspace release needs a version
  newer than `0.6.0`.

## Next Task

`WRV-020`: document publish order and choose the next workspace release version.

Recommended direction:

- Prefer a patch bump unless public API policy requires a minor bump.
- Publish in dependency order.
- Do not run `cargo publish` in this lane without an explicit release command.

## Guardrails

- Do not weaken FFI or UniFFI feature gates to force package verification.
- Do not touch ASCII work.
- Do not publish crates from Codex without explicit user instruction.
- Keep platform packaging lanes separate.
