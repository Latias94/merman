# Web WASM Playground

Status: Active
Last updated: 2026-06-01

## Why This Lane Exists

Merman already has a safe binding facade, C ABI, UniFFI bindings, and platform packaging work. The
web surface is still missing: users cannot run merman in a browser or try it from GitHub Pages
without installing native tooling. `repo-ref/merman-page` proves the desired live-editor UX, and
`repo-ref/RaTeX` proves a suitable Rust WASM plus TypeScript package structure.

## Relevant Authority

- ADRs:
  - `docs/adr/0003-workspace-structure.md`
  - `docs/adr/0066-ffi-binding-strategy.md`
- Existing docs:
  - `docs/bindings/OPTIONS_JSON.md`
  - `docs/release/PACKAGE_SURFACES.md`
- Related reference code:
  - `repo-ref/RaTeX/crates/ratex-wasm`
  - `repo-ref/RaTeX/platforms/web`
  - `repo-ref/RaTeX/website/scripts/verify-dist-wasm.mjs`
  - `repo-ref/merman-page`

## Problem

The current repository has no `merman-wasm` crate, no TypeScript package, and no Pages build. The
reference playground contains a mock WASM loader and a placeholder Rust crate that cannot be
checked from its current `repo-ref` location. The real browser path must first prove that the
existing safe facade can compile for `wasm32-unknown-unknown`.

## Target State

- A workspace `crates/merman-wasm` crate exposes a small `wasm-bindgen` API over
  `merman-bindings-core`.
- A `platforms/web` TypeScript package builds the WASM package and exposes typed helpers.
- A `playground` app consumes the web package and provides a live editor suitable for GitHub Pages.
- CI/Pages builds fail if the generated static output misses the WASM JavaScript shim or `.wasm`
  binary.
- Docs record the web/WASM package surface and local build commands.

## In Scope

- Browser WASM rendering to SVG, semantic JSON, and layout JSON.
- A validation helper that maps binding errors into JavaScript-friendly results.
- A Ratex-style TypeScript wrapper package under `platforms/web`.
- Migrating and hardening the existing `repo-ref/merman-page` live editor.
- GitHub Pages build and static artifact verification.

## Out Of Scope

- Replacing the existing C ABI, UniFFI, Python, Apple, Android, or Flutter binding surfaces.
- Browser raster/PDF output from Rust in the first slice.
- Publishing an npm package before local and Pages builds are stable.
- Reworking Mermaid parity fixtures or unrelated rendering behavior.

## Starting Assumptions

| Assumption | Confidence | Evidence | Consequence if wrong |
| --- | --- | --- | --- |
| `merman-bindings-core` is the correct WASM backend boundary. | High | ADR 0066 and existing facade tests | Duplicate browser-specific render logic would drift from native bindings. |
| Browser WASM should use `wasm-bindgen`, not UniFFI or the C ABI. | High | RaTeX web surface and UniFFI browser limitations | Generated bindings would be larger or awkward for web consumers. |
| First browser output should be SVG/JSON only. | High | `docs/bindings/OPTIONS_JSON.md` current surface | Raster/PDF can be split after the core browser package is stable. |
| The first compile blockers are target randomness features. | High | 2026-06-01 wasm checks hit `uuid` and `getrandom` errors | The first implementation task must fix target compatibility before adding frontend build complexity. |

## Architecture Direction

Keep the same layering as the native binding work:

```text
merman-core / merman-render / merman
        |
merman-bindings-core       (safe facade, options JSON, error classification)
        |
merman-editor-core         (shared editor diagnostics, completion, symbols, navigation)
        |
crates/merman-wasm        (wasm-bindgen transport, JS values, panic hook)
        |
platforms/web             (TypeScript package, typed options, dynamic WASM init)
        |
playground                (live editor / GitHub Pages app)
```

The WASM crate should expose strings and JSON instead of Rust structs. The TypeScript package can
layer typed options over the same JSON contract already used by native bindings. Browser editor
APIs are stateless document queries over `merman-editor-core`; the playground projects those results
into Monaco providers for diagnostics, completion, hover, document symbols, definition,
references, rename, code actions, and semantic tokens.

## Closeout Condition

This lane can close when:

- the WASM crate builds with `wasm-pack`,
- the TypeScript package builds against the generated package,
- the playground renders with the real WASM module,
- the Pages workflow uploads a verified static artifact,
- and any npm publishing or raster follow-ons are split or explicitly deferred.
