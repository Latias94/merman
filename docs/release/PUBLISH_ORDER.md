# Publish Order

Status: draft for next workspace release.
Last updated: 2026-07-03

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
2. `manatee`
3. `merman-core`
4. `merman-elk-layered`
5. `roughr-merman`
6. `dugong`
7. `merman-analysis`
8. `merman-ascii`
9. `merman-layout-elk`
10. `merman-editor-core`
11. `merman-render`
12. `merman`
13. `merman-lsp`
14. `merman-bindings-core`
15. `merman-cli`
16. `merman-rustdoc`
17. `merman-ffi`
18. `merman-typst-plugin`
19. `merman-uniffi`
20. `merman-wasm`

This list is intentionally identical to `.github/workflows/release-crates.yml`,
`tools/publish.py`, `docs/releasing/CRATES_IO.md`, and `docs/releasing/PUBLISHING.md`.
Run `python3 scripts/verify-release-crate-order.py` after changing any publishable crate, release
workflow, or release-order document.

`roughr-merman` is versioned separately as `0.12.1`. The workflow reads each crate's own package
version, so it can skip already-published crates while still keeping one dependency-ordered list.

## Binding Release Chain

The binding-specific chain is:

```text
merman-analysis
  -> merman-editor-core
  -> merman-lsp

merman-render
  -> merman
  -> merman-bindings-core
  -> merman-ffi
  -> merman-uniffi
  -> merman-wasm
```

This is why `merman-ffi` cannot fully package-verify until `merman-bindings-core` is published, and
`merman-bindings-core` cannot fully package-verify until a newer `merman-render` with `ratex-math`
is available on crates.io. `merman-wasm` comes last because it combines the browser wasm-bindgen
transport with the released binding core, renderer, ASCII, and editor-language crates.

## Pre-Publish Gates

Before publishing, run focused checks:

```bash
python3 scripts/verify-release-crate-order.py
cargo check -p merman-ffi
cargo check -p merman-uniffi
cargo nextest run -p merman-bindings-core -p merman-ffi -p merman-uniffi
cargo nextest run -p merman-uniffi --features bindgen-smoke --test bindgen_smoke
```

For crates.io packaging, prefer publish dry-runs once registry dependencies are available. The
release workflow runs this gate automatically for every unpublished crate immediately before the
real publish, so it also covers `merman-bindings-core`, `merman-ffi`, and `merman-uniffi`.

```bash
cargo publish -p merman-render --locked --dry-run --registry crates-io
cargo publish -p merman-bindings-core --locked --dry-run --registry crates-io
cargo publish -p merman-ffi --locked --dry-run --registry crates-io
cargo publish -p merman-uniffi --locked --dry-run --registry crates-io
```

Before upstream crates for the same release are visible in crates.io, keep using `cargo package
--list` only as a file-list check. It does not replace publish dry-run verification.

## Current Package Matrix

As of 2026-07-03:

| Crate | Gate | Current result |
| --- | --- | --- |
| `dugong-graphlib` | crates.io lookup | Published |
| `manatee` | crates.io lookup | Published |
| `merman-core` | crates.io lookup | Published |
| `merman-elk-layered` | crates.io lookup | Published |
| `roughr-merman` | release workflow skip-if-published check | Versioned separately as `0.12.1`; publish only when that crate version is not already visible |
| `dugong` | crates.io lookup | Published |
| `merman-analysis` | release workflow dry-run before publish | Pending after `merman-core` is published |
| `merman-ascii` | `cargo publish -p merman-ascii --locked --dry-run --allow-dirty --registry crates-io` | Pass locally; not yet published |
| `merman-layout-elk` | crates.io lookup | Published |
| `merman-editor-core` | release workflow dry-run before publish | Pending after `merman-analysis` is published |
| `merman-render` | `cargo publish -p merman-render --locked --dry-run --allow-dirty --registry crates-io` | Pass locally after release-source fix; not yet published |
| `merman` | release workflow dry-run before publish | Pending after `merman-render` is published |
| `merman-lsp` | release workflow dry-run before publish | Pending after `merman-editor-core` and `merman` are published |
| `merman-bindings-core` | release workflow dry-run before publish | Pending after `merman` is published |
| `merman-cli` | release workflow dry-run before publish | Pending after `merman` is published |
| `merman-rustdoc` | release workflow dry-run before publish | Pending after `merman` is published |
| `merman-ffi` | release workflow dry-run before publish | Pending after `merman-bindings-core` is published |
| `merman-typst-plugin` | release workflow dry-run before publish | Pending after `merman-bindings-core` is published |
| `merman-uniffi` | release workflow dry-run before publish | Pending after `merman-bindings-core` is published |
| `merman-wasm` | release workflow dry-run before publish | Pending after `merman-bindings-core`, `merman-editor-core`, and `merman` are published |

## Publish Guardrail

Do not run `cargo publish` as part of an implementation lane unless the release operator explicitly
requests it. This document prepares the order and gates; it is not itself a publish command.
