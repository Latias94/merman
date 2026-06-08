# @mermanjs/web

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
import { initMerman, renderSvg } from "@mermanjs/web";

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
import { initMerman, renderSvgToElement } from "@mermanjs/web";

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
import type { MermanWasmModule } from "@mermanjs/web";

await initMerman({
  loader: async () =>
    (await import("@mermanjs/web/pkg/merman_wasm.js")) as MermanWasmModule,
  wasm: new URL("@mermanjs/web/pkg/merman_wasm_bg.wasm", import.meta.url),
});
```

Concurrent calls share the same in-flight initialization promise.

## WASM loading best practices

`@mermanjs/web` ships one full browser renderer artifact. It is intended for playgrounds, diagram
editors, documentation previews, and applications that need headless Mermaid rendering in the
browser. Treat it as a feature module, not as first-paint UI code:

- Call `initMerman()` lazily when the editor, preview pane, or first diagram render is needed.
- Preload on route hover, editor open, or `requestIdleCallback` when you know rendering is likely.
- Keep one initialized module per page; `initMerman()` is asynchronous, idempotent, and shares
  concurrent initialization work.
- Serve `pkg/merman_wasm_bg.wasm` with `Content-Type: application/wasm`, gzip or brotli
  compression, and long-lived immutable caching for versioned assets.
- Use `renderSvg()` in framework code and mount the returned SVG string through your normal
  framework path. Use `renderSvgElement()` / `renderSvgToElement()` only on the main thread because
  they require `DOMParser` and `document`.

The package currently does not publish separate render-only or ASCII-only builds. The default build
keeps the public API simple and avoids fragmenting cache behavior across package variants.

## Web Worker integration

`@mermanjs/web` does not bundle an opinionated worker wrapper yet. Worker queues, cancellation,
timeouts, transfer protocol, and framework integration usually belong to the host application. The
recommended pattern is to initialize Merman once inside a module worker and send SVG strings back to
the main thread:

```ts
// merman.worker.ts
import { initMerman, renderSvg, type SvgBindingOptions } from "@mermanjs/web";

type RenderRequest = {
  id: string;
  source: string;
  options?: SvgBindingOptions;
};

let ready: Promise<unknown> | null = null;

self.onmessage = async (event: MessageEvent<RenderRequest>) => {
  const { id, source, options } = event.data;
  try {
    ready ??= initMerman();
    await ready;
    self.postMessage({ id, ok: true, svg: renderSvg(source, options) });
  } catch (error) {
    self.postMessage({
      id,
      ok: false,
      error: error instanceof Error ? error.message : String(error),
    });
  }
};
```

```ts
// main thread
const worker = new Worker(new URL("./merman.worker.ts", import.meta.url), {
  type: "module",
});

worker.postMessage({
  id: "diagram-1",
  source: "flowchart TD\nA[Hello] --> B[World]",
  options: { svg: { pipeline: "readable" } },
});
```

Use a worker for large documents, repeated batch rendering, or editor keystroke previews where
synchronous rendering could block input. For occasional single-diagram renders, lazy main-thread
initialization is usually simpler.

## API surface

- `initMerman()`, `getMerman()`, `isMermanInitialized()`
- `renderSvg()`, `renderSvgElement()`, `renderSvgToElement()`
- `renderAscii()`
- `parseJson()`, `parseObject()`
- `layoutJson()`, `layoutObject()`
- `validate()`
- `supportedDiagrams()`, `asciiSupportedDiagrams()`, `supportedThemes()`
- `abiVersion()`, `packageVersion()`, `encodeOptions()`

All render, parse, layout, validation, and metadata functions require `initMerman()` first.
`supportedDiagrams()`, `asciiSupportedDiagrams()`, and `supportedThemes()` return typed metadata and fail
fast if the generated WebAssembly metadata drifts from the TypeScript surface.

## Benchmarking against Mermaid JS

The web binding is suitable for browser-to-browser benchmarks after initialization:

1. Build `@mermanjs/web` once.
2. Launch one headless Chromium instance.
3. Initialize `@mermanjs/web` and Mermaid JS before measuring.
4. Measure repeated `renderSvg()` calls against repeated `mermaid.render()` calls on the same
   fixtures, theme, viewport width, and warmup/measurement windows.

This is the useful comparison for playground and browser embedding performance. Native
`merman-cli` benchmarks should be reported separately because they do not include the same runtime
or DOM costs as Mermaid JS.

## License

This package is dual-licensed under either Apache-2.0 or MIT. See `LICENSE` for the full license
texts. Mermaid compatibility and upstream Mermaid MIT attribution are documented in
[`THIRD_PARTY_NOTICES.md`](https://github.com/Latias94/merman/blob/main/THIRD_PARTY_NOTICES.md).
