# Publish Order

Status: draft for next workspace release.
Last updated: 2026-05-30

## Version Decision

Next release target: `0.7.0`.

Rationale:

- crates.io versions are immutable and `0.6.0` is already insufficient for the current binding
  graph.
- `merman-render 0.6.0` on crates.io does not expose the local `ratex-math` feature needed by
  `merman-bindings-core`.
- The workspace has added public binding crates and surfaces: `merman-bindings-core`,
  `merman-ffi`, and `merman-uniffi`.
- A minor bump is clearer than a patch bump for a pre-1.0 release that adds public package
  capabilities.

`WRV-030` should align manifests to this version before package verification.

## Publish Order

Publish crates in dependency order:

1. `dugong-graphlib`
2. `dugong`
3. `manatee`
4. `merman-core`
5. `merman-render`
6. `merman-ascii`
7. `merman`
8. `merman-bindings-core`
9. `merman-ffi`
10. `merman-uniffi`
11. `merman-cli`

`roughr-merman` is versioned separately as `0.12.0`. Publish it before `merman-render` only if that
crate changed and needs a new release.

## Binding Release Chain

The binding-specific chain is:

```text
merman-render
  -> merman
  -> merman-bindings-core
  -> merman-ffi
  -> merman-uniffi
```

This is why `merman-ffi` cannot fully package-verify until `merman-bindings-core` is published, and
`merman-bindings-core` cannot fully package-verify until a newer `merman-render` with `ratex-math`
is available on crates.io.

## Pre-Publish Gates

Before publishing, run focused checks:

```bash
cargo check -p merman-ffi
cargo check -p merman-uniffi
cargo nextest run -p merman-bindings-core -p merman-ffi -p merman-uniffi
cargo nextest run -p merman-uniffi --features bindgen-smoke --test bindgen_smoke
```

For packaging, distinguish file-list checks from full crates.io dependency verification:

```bash
cargo package -p merman-render --allow-dirty
cargo package -p merman-bindings-core --allow-dirty --list
cargo package -p merman-ffi --allow-dirty --list
cargo package -p merman-uniffi --allow-dirty --list
```

After each upstream crate is published and the index is updated, full package verification can move
one step farther down the dependency chain.

## Publish Guardrail

Do not run `cargo publish` as part of an implementation lane unless the release operator explicitly
requests it. This document prepares the order and gates; it is not itself a publish command.
