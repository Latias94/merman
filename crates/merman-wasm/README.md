# merman-wasm

WebAssembly bindings for Merman browser use.

`merman-wasm` is the Rust `wasm-bindgen` transport crate behind the public
[`@mermanjs/web`](https://github.com/Latias94/merman/tree/main/platforms/web#readme) package. It
exposes SVG rendering, semantic JSON, layout JSON, ASCII/Unicode rendering, validation, metadata
helpers, optional diagnostics analysis, and an optional editor-language surface with the same options
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
The current alpha WASM contract reports ABI 2. Host callbacks receive one of 19 exact measurement
operation names; the contiguous core operation range is `0..18`, ending in `raw-bbox-height`.

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
npm run build:wasm:render-only --prefix platforms/web
npm run build:wasm:ascii --prefix platforms/web
npm run build:wasm:full --prefix platforms/web
npm run build:wasm:math --prefix platforms/web
```

The generated module exports `bindingCapabilities()`, `diagramFamilyCapabilities()`, and
`lintRuleCatalog()` so JavaScript callers can detect whether the current artifact includes
`render`, `analysis`, `ascii`, `cytoscape_layout`, `elk_layout`, `ratex_math`, or `editor_language`
support, inspect its ordered `system_adapter_ids`, inspect the complete pinned Mermaid language
catalog, and discover configurable or
authoring-only lint rules. Lint rule entries include evidence references so browser hosts can
explain why a rule is classified as Mermaid-backed compatibility or Merman authoring guidance.
All artifacts share the same language catalog; slim presets only omit output, analysis, editor, or
host capabilities.
Render entry points stay present on the JavaScript surface for shape stability, but artifacts built
without render support return `MERMAN_UNSUPPORTED_FORMAT`; callers should use
`bindingCapabilities().render` rather than function presence for capability checks. Optional surfaces
such as analysis, editor-language, and host text-measurement remain feature/preset gated when their
features are not compiled into the artifact.

The Rust feature boundary for diagnostics and validation is `analysis`. It controls `analyze*`,
`analysisFacts`, `validate`, and lint rule catalog helpers. The `editor-language` feature implies
`analysis`, while ASCII-only browser builds can omit both.

`analyze*` returns diagnostics schema v1. `analysisFacts` returns parser-only facts schema v2 and
rejects facts v1 at the version boundary; the TextScan-capable alpha implementation from
`0.8.0-alpha.3` is not retained. The two JSON contracts are independently versioned, and their
versions are independent from
wasm-bindgen/package ABI versions and Mermaid's own `*-v2` ids.

The Rust feature boundary for the browser editor APIs is `editor-language`. Slim browser presets
leave that feature off so they do not compile `merman-editor-core` into render-only or ASCII-only
artifacts. The current published npm default remains `browser-full`, so editor-language stays
enabled in the public package unless a future release decision changes that default.

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
