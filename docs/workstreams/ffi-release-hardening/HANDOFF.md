# FFI Release Hardening — Handoff

Status: Closed with packaging concern
Last updated: 2026-05-30

## Current State

This follow-on lane hardened the completed `ffi-api` C ABI release candidate.

Completed:

- added a real C consumer smoke test that compiles and runs C code against the ABI via function
  pointers
- changed `merman-ffi` to use a crate-local README instead of the root README as package docs
- documented the C ABI from the root README and `docs/bindings/FFI_PROTOCOL.md`
- proved local FFI tests and clippy under default and `ratex-math` feature sets

## Packaging Concern

`cargo package -p merman-ffi --allow-dirty` currently fails before verification because crates.io
`merman-render 0.6.0` does not expose the local `ratex-math` feature used by this workspace state.
This should be handled by a workspace release-versioning/publish-order follow-on, not by weakening
the FFI ABI.

`cargo package -p merman-ffi --allow-dirty --list` passes and confirms the expected package files.

## Do Not Touch

- ASCII workstreams or renderer code.
- UniFFI crates or generated bindings.
- Raster ABI functions.

## Expected Follow-On

Recommended next lane: `uniffi-bindings`, starting with a shared safe bindings facade. Before an
actual crate release, run a workspace release-versioning lane so `merman-render`, `merman`, and
`merman-ffi` publish in dependency order.
