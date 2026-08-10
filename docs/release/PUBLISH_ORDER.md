# Publish Order

Status: maintained workspace publish order.
Last updated: 2026-08-10

## Version Decision

Published workspace prerelease baseline: `0.8.0-alpha.5`.

The next workspace release remains in development and has no selected version yet. For npm flow
testing only, the browser and Node package groups may publish `0.8.0-alpha.5` from a reviewed,
immutable source commit that is newer than the workspace `v0.8.0-alpha.5` tag. Their provenance
must identify that exact commit, and documentation must not imply cross-channel byte identity.

Rationale:

- crates.io versions are immutable and `0.8.0-alpha.1` has already started the 0.8 release line.
- The workspace has added 0.8-line Typst/package-size feature work and Mermaid parity fixes that
  should be tested behind a prerelease before the next stable cut.
- Workspace-coupled platform packages should stay aligned so downstream Web, FFI, and documentation
  integrations test one coherent version graph. The unpublished VS Code extension follows its own
  `0.1.x` version track and records the bundled workspace runtime separately.

Workspace-coupled manifests remain aligned to `0.8.0-alpha.5`. Python package metadata uses the
PEP 440 spelling `0.8.0a5`, but manifest alignment does not prove that a surface reached its
registry or that separately published alpha.5 channels share one source snapshot. The
independently versioned VS Code extension, Typst wrapper, and `roughr-merman` remain on their own
release axes.

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
12. `merman-export`
13. `merman`
14. `merman-lsp`
15. `merman-bindings-core`
16. `merman-cli`
17. `merman-rustdoc`
18. `merman-ffi`
19. `merman-typst-plugin`
20. `merman-uniffi`
21. `merman-wasm`

This list is intentionally identical to `.github/workflows/release-crates.yml`,
`tools/publish.py`, `docs/releasing/CRATES_IO.md`, and `docs/releasing/PUBLISHING.md`.
Run `python3 scripts/verify-release-crate-order.py` after changing any publishable crate, release
workflow, or release-order document. The guard checks duplicate entries, cross-file sync, the
publishable workspace crate set, and Cargo metadata dependency topology, so a crate must appear
after every publishable workspace dependency it needs for a crates.io publish.

`roughr-merman` is versioned separately as `0.12.2`. The workflow reads each crate's own package
version, so it can skip already-published crates while still keeping one dependency-ordered list.

## Binding Release Chain

The binding-specific chain is:

```text
merman-analysis
  -> merman-editor-core
  -> merman-lsp

merman-render
  -> merman-export
  -> merman
  -> merman-bindings-core
  -> merman-ffi
  -> merman-uniffi
  -> merman-wasm
```

This is why `merman-ffi` cannot fully package-verify until `merman-bindings-core` is published, and
`merman-bindings-core` cannot fully package-verify until a newer `merman-render` with `math`
is available on crates.io. `merman-wasm` comes last because it combines the browser wasm-bindgen
transport with the released binding core, renderer, ASCII, and editor-capable crates.

## Browser Package Group

The npm browser SDK is not a single Cargo publication and is intentionally outside the crates.io
topological order. After the selected source revision has passed release preflight, run
`release-web.yml` for the admitted package group: `@mermanjs/web`, `@mermanjs/web-analysis`,
`@mermanjs/web-editor`, `@mermanjs/web-ascii`, and `@mermanjs/web-render`. The workflow publishes
missing exact versions to a staging tag, verifies every member, then promotes the requested public
tag as a recoverable group operation.

The first version of a new split Web package cannot use npm Trusted Publishing before the package
exists. Run `release-web.yml` without publication, download the verified package-group artifact,
publish only the missing exact tarballs under its staging tag with a maintainer's 2FA-protected npm
credential, configure Trusted Publishing for those package names, then rerun the workflow with
publication enabled. Do not keep the bootstrap credential in GitHub Actions.

## Node Native Package Group

The experimental Node package is also a lockstep npm group, but it is native rather than browser
WASM: `@mermanjs/node`, `@mermanjs/node-darwin-arm64`, `@mermanjs/node-darwin-x64`,
`@mermanjs/node-linux-x64-gnu`, `@mermanjs/node-linux-x64-musl`, and
`@mermanjs/node-win32-x64-msvc`. Run `release-node.yml` against a reviewed immutable source commit
after the matching preflight succeeds. It builds and installs every native target, publishes
missing exact versions under a staging tag in platform-first order, verifies registry integrity,
then promotes the requested tag with the root loader last.

The first version of each npm package cannot use npm Trusted Publishing before the package exists.
For that one bootstrap, download the verified group artifact from a non-publishing run, publish the
five platform tarballs and then the loader under the workflow's staging tag with a maintainer's
2FA-protected npm credential, configure Trusted Publishing for all six package names, and rerun
`release-node.yml` with publishing enabled. Thereafter the workflow owns reconciled publishing and
provenance; do not keep an npm token in GitHub Actions.

## Pre-Publish Gates

Before publishing, run focused checks:

```bash
python3 scripts/verify-release-crate-order.py
cargo check -p merman-ffi
cargo check -p merman-uniffi
cargo nextest run -p merman-bindings-core -p merman-ffi -p merman-uniffi
```

For crates.io packaging, prefer publish dry-runs once registry dependencies are available. The
release workflow runs this gate automatically for every unpublished crate immediately before the
real publish, so it also covers `merman-bindings-core`, `merman-ffi`, and `merman-uniffi`.

```bash
cargo publish -p merman-render --locked --dry-run --registry crates-io
cargo publish -p merman-export --locked --dry-run --registry crates-io
cargo publish -p merman-bindings-core --locked --dry-run --registry crates-io
cargo publish -p merman-ffi --locked --dry-run --registry crates-io
cargo publish -p merman-uniffi --locked --dry-run --registry crates-io
```

Before upstream crates for the same release are visible in crates.io, keep using `cargo package
--list` only as a file-list check. It does not replace publish dry-run verification.

## Pre-Alpha.3 Package Evidence Snapshot

The table below is historical pre-publication evidence captured on 2026-07-03. Query crates.io or
the owning artifact release directly for current availability; do not infer it from this snapshot.

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
| `merman-export` | release workflow dry-run before publish | Pending after `merman-render` is published |
| `merman` | release workflow dry-run before publish | Pending after `merman-export` is published |
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
