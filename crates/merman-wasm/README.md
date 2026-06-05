# merman-wasm

WebAssembly bindings for Merman browser use.

`merman-wasm` is the Rust `wasm-bindgen` transport crate behind the public
[`@merman/web`](https://github.com/Latias94/merman/tree/main/platforms/web#readme) package. It
exposes SVG rendering, semantic JSON, layout JSON, ASCII/Unicode rendering, validation, and metadata
helpers with the same options JSON contract used by the native bindings.

Most browser and TypeScript applications should install `@merman/web` rather than depending on this
crate directly:

```sh
npm install @merman/web
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

For product scope, diagram coverage, and compatibility policy, see the
[project README](https://github.com/Latias94/merman#readme) and
[alignment status](https://github.com/Latias94/merman/blob/main/docs/alignment/STATUS.md).
