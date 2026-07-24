# merman-wasm

WebAssembly bindings for Merman browser use.

`merman-wasm` is the Rust `wasm-bindgen` transport crate behind the public
[`@mermanjs/web`](https://github.com/Latias94/merman/tree/main/platforms/web#readme) package. It
exposes SVG rendering, semantic JSON, layout JSON, ASCII/Unicode rendering, validation, metadata
helpers, optional diagnostics analysis, and an optional editor surface with the same options
JSON contract used by the native bindings.
Metadata helpers include Mermaid core themes and separate host/editor theme presets for
`host_theme.preset`.

This crate is intentionally a browser/JS WebAssembly surface. It uses wasm-bindgen and browser
imports; it is not the Typst or pure `wasm32-unknown-unknown` package surface.

Most browser and TypeScript applications should install `@mermanjs/web` rather than depending on this
crate directly:

```sh
npm install @mermanjs/web
```

For browser font fidelity, use the web wrapper's `renderSvgWithTextMeasurer`,
`layoutJsonWithTextMeasurer`, and `createBrowserTextMeasurer` APIs. The wrapper owns the
TypeScript request/response shape and DOM helper for browser text measurement.
The browser transport reports transport API version `3` and flat runtime catalog schema `1`. These
versions are independent from native ABI 3, the Typst plugin ABI, and text-measurement protocol
version `1`. Host callbacks receive one of 19 exact measurement operation names; the contiguous
core operation range is `0..18`, ending in `raw-bbox-height`.

Use this crate directly when you need to rebuild the wasm-bindgen package from source or integrate
the generated wasm artifacts into a custom packaging flow.

## Build

```sh
wasm-pack build crates/merman-wasm --target web --profile wasm-size --out-dir ../../target/merman-wasm-pkg
```

The web wrapper requires `wasm-pack` 0.15.0 or newer because it builds with the workspace
`wasm-size` Cargo profile.

The checked-in TypeScript wrapper builds this crate into `platforms/web/pkg`:

```sh
npm run build --prefix platforms/web
npm run smoke --prefix platforms/web
```

The Web descriptor maps each standalone package to one exact `web-*` artifact profile. Build all
owned packages or one selected package with:

```sh
npm run build:wasm --prefix platforms/web
npm run build:wasm:analysis --prefix platforms/web
npm run build:wasm:render --prefix platforms/web
npm run build:wasm:ascii --prefix platforms/web
npm run build:wasm:editor --prefix platforms/web
npm run build:wasm:full --prefix platforms/web
```

The generated module exports `runtimeCatalog()`, `diagramFamilyCapabilities()`, and
`lintRuleCatalog()`. `runtimeCatalog()` is the closed runtime authority: callers inspect stable
`capability_ids`, `output_ids`, `system_adapter_ids`, and text-measurement provider IDs instead of
inferring support from function presence or legacy booleans. The family catalog covers the complete
pinned Mermaid language surface, while lint entries distinguish Mermaid-backed compatibility from
Merman authoring guidance. All artifacts share the same language catalog; narrower artifact
profiles omit only callable output, analysis, editor, engine, or adapter capabilities.

Render entry points stay present on the low-level WebAssembly surface for transport-shape stability.
An artifact without the requested output or engine returns a typed `missing-capability` error naming
the stable capability ID. Optional analysis, editor, and host text-measurement APIs remain absent or
fail closed when their corresponding capability is not compiled.

The Rust feature boundary for diagnostics and validation is `analysis`. It controls `analyze*`,
`analysisFacts`, `validate`, and lint rule catalog helpers. The `editor` feature implies
`analysis`, while ASCII-only browser builds can omit both.

`analyze*` returns diagnostics schema v1. `analysisFacts` returns parser-only facts schema v1 and
rejects every other version at the boundary; the TextScan-capable implementation from
`0.8.0-alpha.3` is not retained. The two JSON contracts are independently versioned, and their
versions are independent from
wasm-bindgen/package ABI versions and Mermaid's own `*-v2` ids.

The Rust feature boundary for the browser editor APIs is `editor`; its public capability
ID is also `editor`. Artifacts other than `web-editor` and `web-full` leave the feature off so they do not
compile `merman-editor-core` into analysis, render, or ASCII artifacts. The default package is
`@mermanjs/web`, backed by `web-full`.

For exact artifact-profile size measurements and committed budget checks, use:

```sh
cargo run -p xtask -- wasm-size-matrix --surface web --budget-file docs/release/WASM_SIZE_BUDGETS.json
cargo run -p xtask -- wasm-size-matrix --artifact-profile web-full --budget-file docs/release/WASM_SIZE_BUDGETS.json
```

The matrix reports raw, stripped, gzip, and brotli bytes. gzip and brotli are measured from the
stripped artifact. `--no-strip` is available only for exploratory measurements without a budget
file because committed budgets require all four stripped-artifact metrics.

For product scope, diagram coverage, and compatibility policy, see the
[project README](https://github.com/Latias94/merman#readme) and
[alignment status](https://github.com/Latias94/merman/blob/main/docs/alignment/STATUS.md).
