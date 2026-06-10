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
wasm-pack build crates/merman-wasm --target web --out-dir ../../target/merman-wasm-pkg
```

The checked-in TypeScript wrapper builds this crate into `platforms/web/pkg`:

```sh
npm run build --prefix platforms/web
npm run smoke --prefix platforms/web
```

For feature-preset size measurements, use:

```sh
cargo run -p xtask -- wasm-size-matrix --surface browser
```

For product scope, diagram coverage, and compatibility policy, see the
[project README](https://github.com/Latias94/merman#readme) and
[alignment status](https://github.com/Latias94/merman/blob/main/docs/alignment/STATUS.md).
