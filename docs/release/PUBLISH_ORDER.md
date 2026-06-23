# Publish Order

Status: draft for next workspace release.
Last updated: 2026-06-23

## Version Decision

Next release target: `0.8.0-alpha.2`.

Rationale:

- crates.io versions are immutable and `0.8.0-alpha.1` has already started the 0.8 release line.
- The workspace has added 0.8-line Typst/package-size feature work and Mermaid parity fixes that
  should be tested behind a prerelease before the next stable cut.
- The platform packages should stay aligned with the workspace release so downstream editor, web,
  FFI, and documentation integrations test one coherent version graph.

Manifests are aligned to `0.8.0-alpha.2` for this release. Python package metadata uses the PEP 440
spelling `0.8.0a2`.

## Publish Order

Publish crates in dependency order:

1. `dugong-graphlib`
2. `dugong`
3. `manatee`
4. `merman-core`
5. `merman-elk-layered`
6. `merman-layout-elk`
7. `merman-render`
8. `merman-ascii`
9. `merman`
10. `merman-rustdoc`
11. `merman-bindings-core`
12. `merman-ffi`
13. `merman-uniffi`
14. `merman-cli`

`roughr-merman` is versioned separately as `0.12.1`. Publish it before `merman-render` only if that
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
cargo package -p merman-elk-layered --allow-dirty
cargo package -p merman-layout-elk --allow-dirty --list
cargo package -p merman-render --allow-dirty
cargo package -p merman-rustdoc --allow-dirty --list
cargo package -p merman-bindings-core --allow-dirty --list
cargo package -p merman-ffi --allow-dirty --list
cargo package -p merman-uniffi --allow-dirty --list
```

After each upstream crate is published and the index is updated, full package verification can move
one step farther down the dependency chain.

## Current Package Matrix

As of 2026-06-23:

| Crate | Gate | Current result |
| --- | --- | --- |
| `dugong-graphlib` | `cargo package -p dugong-graphlib --allow-dirty` | Pass |
| `manatee` | `cargo package -p manatee --allow-dirty` | Pass |
| `merman-core` | `cargo package -p merman-core --allow-dirty` | Pass |
| `dugong` | `cargo package -p dugong --allow-dirty` | Blocked until `dugong-graphlib 0.8.0-alpha.2` is published |
| `merman-elk-layered` | `cargo package -p merman-elk-layered --allow-dirty` | Pass |
| `merman-layout-elk` | `cargo package -p merman-layout-elk --allow-dirty --list` | Pass |
| `merman-layout-elk` | `cargo package -p merman-layout-elk --allow-dirty` | Blocked until `merman-elk-layered 0.8.0-alpha.2` is published |
| `merman-render` | `cargo package -p merman-render --allow-dirty` | Blocked until `dugong 0.8.0-alpha.2` and `merman-layout-elk 0.8.0-alpha.2` are published |
| `merman-ascii` | `cargo package -p merman-ascii --allow-dirty --list` | Pass |
| `merman` | `cargo package -p merman --allow-dirty --list` | Pass |
| `merman-rustdoc` | `cargo package -p merman-rustdoc --allow-dirty --list` | Pass |
| `merman-rustdoc` | `cargo package -p merman-rustdoc --allow-dirty` | Blocked until `merman 0.8.0-alpha.2` is published |
| `merman-bindings-core` | `cargo package -p merman-bindings-core --allow-dirty --list` | Pass |
| `merman-bindings-core` | `cargo package -p merman-bindings-core --allow-dirty` | Blocked until `merman 0.8.0-alpha.2` is published |
| `merman-ffi` | `cargo package -p merman-ffi --allow-dirty --list` | Pass |
| `merman-ffi` | `cargo package -p merman-ffi --allow-dirty` | Blocked until `merman-bindings-core 0.8.0-alpha.2` is published |
| `merman-uniffi` | `cargo package -p merman-uniffi --allow-dirty --list` | Pass |
| `merman-uniffi` | `cargo package -p merman-uniffi --allow-dirty` | Blocked until `merman-bindings-core 0.8.0-alpha.2` is published |
| `merman-cli` | `cargo package -p merman-cli --allow-dirty --list` | Pass |

## Publish Guardrail

Do not run `cargo publish` as part of an implementation lane unless the release operator explicitly
requests it. This document prepares the order and gates; it is not itself a publish command.
