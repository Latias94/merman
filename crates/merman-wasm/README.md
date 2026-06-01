# merman-wasm

WebAssembly bindings for browser use.

This crate is a thin `wasm-bindgen` transport over `merman-bindings-core`. It exposes SVG,
semantic JSON, and layout JSON with the same options JSON contract used by the native bindings.

## Build

```sh
wasm-pack build crates/merman-wasm --target web --out-dir ../../target/merman-wasm-pkg
```

The TypeScript package and playground are tracked by `docs/workstreams/web-wasm-playground`.
