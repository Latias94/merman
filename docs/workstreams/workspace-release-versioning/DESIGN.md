# Workspace Release Versioning

Status: Active
Last updated: 2026-05-30

## Why This Lane Exists

The FFI and UniFFI binding surface is implemented, but release packaging is blocked by workspace
publish order. `merman-ffi` now depends on the new `merman-bindings-core` crate, and
`merman-bindings-core` depends on a local `merman-render` feature set that is newer than the
published `merman-render 0.6.0`.

## Problem

`cargo package -p merman-ffi --allow-dirty` cannot fully verify against crates.io yet:

- `merman-bindings-core` does not exist on crates.io.
- `merman-bindings-core 0.6.0` cannot verify against crates.io because published
  `merman-render 0.6.0` does not expose the local `ratex-math` feature.
- crates.io versions are immutable, so the next publishable release must use a newer workspace
  version than `0.6.0`.

## Target State

- A concrete next workspace version is selected.
- Internal workspace dependency versions agree with that release version.
- Publish order is documented and validated as far as crates.io allows before actual publishing.
- Package gates distinguish file-list checks, local package verification, and crates.io dependency
  availability.
- Binding/platform lanes can depend on a known release baseline.

## In Scope

- Release version decision and dependency-version alignment.
- Publish order documentation.
- Package validation commands for publishable workspace crates.
- Evidence explaining package verification blockers before publish.

## Out Of Scope

- Actually running `cargo publish`.
- Platform packages such as iOS, Android, Flutter, Node, or Python.
- FFI ABI redesign.
- ASCII renderer changes.

## Architecture Direction

Treat release packaging as a workspace-level lane. Binding crates are downstream of core/render
crates:

```text
merman-core
  -> merman-render
  -> merman
  -> merman-bindings-core
  -> merman-ffi
  -> merman-uniffi
```

The publish order must follow that graph. Package verification for crates that depend on unpublished
workspace crates should use file-list checks or a local registry workflow until their dependencies
are published.

## Closeout Condition

This lane can close when:

- the release version and publish order are documented,
- workspace dependency versions are aligned,
- package evidence is recorded,
- and any remaining crates.io-only blocker is explicit.
