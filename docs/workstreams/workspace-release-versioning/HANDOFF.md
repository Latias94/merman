# Workspace Release Versioning — Handoff

Status: Active
Last updated: 2026-05-30

## Current State

`WRV-010`, `WRV-020`, `WRV-030`, and `WRV-040` are complete. The binding ABI work is not the
blocker; release packaging is.

Confirmed:

- `merman-render` packages successfully from local source.
- `merman-bindings-core` package file list is correct.
- `merman-bindings-core` full package verification is blocked because crates.io
  `merman-render 0.6.0` lacks the local `ratex-math` feature.
- `merman-ffi` full package verification is blocked because `merman-bindings-core` has not been
  published.
- Since crates.io versions are immutable, the next publishable workspace release needs a version
  newer than `0.6.0`.
- `docs/release/PUBLISH_ORDER.md` selects `0.7.0` as the next release target.
- Workspace package version and internal dependency requirements are aligned to `0.7.0`.
- `docs/release/PUBLISH_ORDER.md` records the package gate matrix.

## Next Task

`WRV-050`: verify and close this lane.

Recommended direction:

- Confirm final docs and manifest state.
- Record any remaining crates.io availability blockers as release-operator handoff notes.
- Do not run `cargo publish` in this lane without an explicit release command.

## Guardrails

- Do not weaken FFI or UniFFI feature gates to force package verification.
- Do not touch ASCII work.
- Do not publish crates from Codex without explicit user instruction.
- Keep platform packaging lanes separate.
