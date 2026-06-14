# merman-wasm

WebAssembly bindings for Merman browser use.

`merman-wasm` is the Rust `wasm-bindgen` transport crate behind the public
[`@mermanjs/web`](https://github.com/Latias94/merman/tree/main/platforms/web#readme) package. It
exposes SVG rendering, semantic JSON, layout JSON, ASCII/Unicode rendering, validation, and metadata
helpers with the same options JSON contract used by the native bindings.
Metadata helpers include Mermaid core themes and separate host/editor theme presets for
`host_theme.preset`.

This crate is intentionally a browser/JS WebAssembly surface. It uses wasm-bindgen and browser
imports; it is not the Typst or pure `wasm32-unknown-unknown` package surface.

Most browser and TypeScript applications should install `@mermanjs/web` rather than depending on this
crate directly:

```sh
npm install @mermanjs/web
```

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

The default web package build uses the `browser-full` preset. The wrapper also exposes source-build
presets for local variant builds:

```sh
npm run build:wasm:core --prefix platforms/web
npm run build:wasm:render --prefix platforms/web
npm run build:wasm:ascii --prefix platforms/web
npm run build:wasm:full --prefix platforms/web
npm run build:wasm:ratex-math --prefix platforms/web
```

The generated module exports `bindingCapabilities()`, `selectedRegistryProfile()`, and
`diagramFamilyCapabilities()` so JavaScript callers can detect whether the current artifact includes
`render`, `ascii`, `core_full`, `core_host`, or `ratex_math` support and which diagram parser/render
facts are registered. The ASCII preset still carries the full core registry because it depends on
the browser ASCII implementation, but render entry points remain disabled.

For feature-preset size measurements, use:

```sh
cargo run -p xtask -- wasm-size-matrix --surface browser
cargo run -p xtask -- wasm-size-matrix --budget-file docs/release/WASM_SIZE_BUDGETS.json
```

The matrix reports raw, stripped, gzip, and brotli bytes. gzip and brotli are measured from the
stripped artifact unless `--no-strip` is used.

For product scope, diagram coverage, and compatibility policy, see the
[project README](https://github.com/Latias94/merman#readme) and
[alignment status](https://github.com/Latias94/merman/blob/main/docs/alignment/STATUS.md).
