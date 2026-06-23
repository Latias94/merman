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

`npm run build` produces the default `browser-full` artifact used for npm publication. The surface
includes rendering, parsing, layout, ASCII, validation, and diagnostics analysis. Source and CI
builds can choose a browser WASM preset when a smaller local artifact is useful:

The WASM build uses the workspace `wasm-size` Cargo profile through `wasm-pack --profile
wasm-size`. Use `wasm-pack` 0.15.0 or newer for local builds.

| Preset | Command | Capability |
| --- | --- | --- |
| `browser-core` | `npm run build:wasm:core --prefix platforms/web` | Browser wasm-bindgen transport and metadata only. Render, parse, layout, validation, and ASCII calls report `MERMAN_UNSUPPORTED_FORMAT`. |
| `browser-render` | `npm run build:wasm:render --prefix platforms/web` | SVG, semantic JSON, layout JSON, diagnostics analysis, validation, themes, and metadata over the minimal core profile. |
| `browser-ascii` | `npm run build:wasm:ascii --prefix platforms/web` | ASCII/Unicode rendering only. This preset still carries the full core registry because the browser ASCII crate depends on the full core/host profile. |
| `browser-full` | `npm run build:wasm:full --prefix platforms/web` | Default browser artifact: full core profile, SVG/layout/parse/analysis/validate, ASCII, host browser capabilities, and ELK layout. Includes EPL-backed ELK code. |
| `browser-full-no-elk` | `node platforms/web/scripts/build-wasm.mjs --preset browser-full-no-elk` | Evidence preset for the full browser surface without ELK. Not the npm default. |
| `browser-ratex-math` | `npm run build:wasm:ratex-math --prefix platforms/web` | Full browser artifact plus the RaTeX math renderer and ELK layout. |

Run `npm run build:ts --prefix platforms/web` after a preset build when producing a complete local
package.

Each build writes `pkg/merman_wasm_preset.json`. `npm run prepack` expects `browser-full` unless
`MERMAN_WEB_ALLOW_NON_DEFAULT_PRESET=1` is set for an intentional local slim package.

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

Host/editor theme presets are separate from Mermaid's native `theme` names:

```ts
import { initMerman, renderSvg, supportedHostThemePresets } from "@mermanjs/web";

await initMerman();

const presets = supportedHostThemePresets();
const svg = renderSvg("flowchart TD\nA[Hello] --> B[World]", {
  host_theme: { preset: "one-dark" },
});
```

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

## Browser text measurement

Headless rendering cannot know the exact browser font fallback that will display the final SVG.
This can show up as clipped trailing characters or slightly different wrapping when a browser,
WebView, or user font stack resolves text differently from merman's built-in headless metrics.

For browser previews where label geometry must match the displayed font stack, provide a host text
measurer. The helper below measures text with an offscreen DOM probe and falls back to merman's
vendored measurer when the DOM is unavailable or a request is not handled:

```ts
import {
  createBrowserTextMeasurer,
  initMerman,
  renderSvgWithTextMeasurer,
} from "@mermanjs/web";

await initMerman();

const measureText = createBrowserTextMeasurer();
const svg = renderSvgWithTextMeasurer(
  "flowchart TD\nA[Start] --> B{Condition?}",
  measureText,
  {
    site_config: {
      fontFamily: '"trebuchet ms", verdana, arial, sans-serif',
      themeVariables: {
        fontFamily: '"trebuchet ms", verdana, arial, sans-serif',
      },
    },
  }
);
```

Use the same font family in both the binding options and your surrounding UI/CSS. If rendering in a
Web Worker, keep using `renderSvg()` with the headless measurer, or send measurement requests to the
main thread through your own worker protocol.

`createBrowserTextMeasurer()` measures the natural no-wrap width for HTML-like labels before it
applies `maxWidth`. Custom measurers should keep that behavior; returning `maxWidth` for a short
label can make the diagram wider than Mermaid would make it in the browser.

`analyze()` returns the diagnostics payload JSON object for linting and editor integrations.

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

The published `@mermanjs/web` package currently ships the `browser-full` artifact. It is intended for
playgrounds, diagram editors, documentation previews, and applications that need headless Mermaid
rendering in the browser. Treat it as a feature module, not as first-paint UI code:

- Call `initMerman()` lazily when the editor, preview pane, or first diagram render is needed.
- Preload on route hover, editor open, or `requestIdleCallback` when you know rendering is likely.
- Keep one initialized module per page; `initMerman()` is asynchronous, idempotent, and shares
  concurrent initialization work.
- Serve `pkg/merman_wasm_bg.wasm` with `Content-Type: application/wasm`, gzip or brotli
  compression, and long-lived immutable caching for versioned assets.
- Use `renderSvg()` in framework code and mount the returned SVG string through your normal
  framework path. Use `renderSvgElement()` / `renderSvgToElement()` only on the main thread because
  they require `DOMParser` and `document`.

The package does not publish separate npm subpaths for render-only or ASCII-only artifacts yet. Use
the source build presets above when you need to produce a local slim package, and call
`bindingCapabilities()` after initialization before relying on optional `render`, `ascii`,
`core_full`, `core_host`, `elk_layout`, or `ratex_math` capability. `selectedRegistryProfile()`
reports the active Mermaid registry profile, and `diagramFamilyCapabilities()` reports the diagram
parser/render facts registered in the current artifact. The ASCII preset currently preserves the
full core registry for compatibility with the browser ASCII implementation.

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
- `renderSvg()`, `renderSvgWithTextMeasurer()`, `renderSvgElement()`, `renderSvgToElement()`
- `renderAscii()`
- `parseJson()`, `parseObject()`
- `layoutJson()`, `layoutJsonWithTextMeasurer()`, `layoutObject()`
- `analyze()`, `analyzeJson()`, `validate()`
- `supportedDiagrams()`, `asciiSupportedDiagrams()`, `supportedThemes()`, `supportedHostThemePresets()`
- `createBrowserTextMeasurer()`, `bindingCapabilities()`, `selectedRegistryProfile()`, `diagramFamilyCapabilities()`
- `abiVersion()`, `packageVersion()`, `encodeOptions()`

All render, parse, layout, analysis, validation, and metadata functions require `initMerman()` first.
`supportedDiagrams()`, `asciiSupportedDiagrams()`, `supportedThemes()`, and
`supportedHostThemePresets()` return typed metadata and fail fast if the generated WebAssembly
metadata drifts from the TypeScript surface.

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
