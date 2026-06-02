# @merman/web

Browser integration for merman. This package wraps the `merman-wasm` wasm-bindgen output with a
small TypeScript API.

Use the live build at [Merman Playground](https://frankorz.com/merman/).

## Build

```sh
npm install --prefix platforms/web
npm run build --prefix platforms/web
npm run smoke --prefix platforms/web
```

## Usage

```ts
import { initMerman, renderSvg } from "@merman/web";

await initMerman();

const svg = renderSvg("flowchart TD\nA[Hello] --> B[World]", {
  svg: { pipeline: "readable" },
});
```

The options object is serialized to the shared merman binding options JSON contract documented in
`docs/bindings/OPTIONS_JSON.md`.

## Browser DOM helper

For non-framework browser integrations, render directly into a host element:

```ts
import { initMerman, renderSvgToElement } from "@merman/web";

await initMerman();

renderSvgToElement(document.querySelector("#preview")!, "sequenceDiagram\nA->>B: hello", {
  svg: { diagram_id: "preview" },
});
```

Framework integrations can use `renderSvg()` and mount the returned SVG string with their normal
HTML/SVG insertion path.

## Custom wasm loading

By default, `initMerman()` dynamically imports `../pkg/merman_wasm.js`. If a bundler or CDN setup
needs to provide the wasm-bindgen module or wasm URL explicitly, pass initialization options:

```ts
import type { MermanWasmModule } from "@merman/web";

await initMerman({
  loader: async () =>
    (await import("@merman/web/pkg/merman_wasm.js")) as MermanWasmModule,
  wasm: new URL("@merman/web/pkg/merman_wasm_bg.wasm", import.meta.url),
});
```

Concurrent calls share the same in-flight initialization promise.

## API surface

- `initMerman()`, `getMerman()`, `isMermanInitialized()`
- `renderSvg()`, `renderSvgElement()`, `renderSvgToElement()`
- `renderAscii()`
- `parseJson()`, `parseObject()`
- `layoutJson()`, `layoutObject()`
- `validate()`
- `supportedDiagrams()`, `asciiSupportedDiagrams()`, `themes()`
- `abiVersion()`, `packageVersion()`, `encodeOptions()`

All render, parse, layout, validation, and metadata functions require `initMerman()` first.
`supportedDiagrams()`, `asciiSupportedDiagrams()`, and `themes()` return typed metadata and fail
fast if the generated WebAssembly metadata drifts from the TypeScript surface.

## Benchmarking against Mermaid JS

The web binding is suitable for browser-to-browser benchmarks after initialization:

1. Build `@merman/web` once.
2. Launch one headless Chromium instance.
3. Initialize `@merman/web` and Mermaid JS before measuring.
4. Measure repeated `renderSvg()` calls against repeated `mermaid.render()` calls on the same
   fixtures, theme, viewport width, and warmup/measurement windows.

This is the useful comparison for playground and browser embedding performance. Native
`merman-cli` benchmarks should be reported separately because they do not include the same runtime
or DOM costs as Mermaid JS.
