# @mermanjs/node-wasm

Explicit Node-targeted WebAssembly rendering for Node.js 22+ and static-site build pipelines.
This is a separate opt-in package; `@mermanjs/node` remains the native N-API package and never
silently falls back to browser or Node WASM.

## Quick start

```sh
npm install @mermanjs/node-wasm@alpha
```

```js
import { createNodeEngine } from "@mermanjs/node-wasm";

const engine = await createNodeEngine();

try {
  const svg = await engine.renderSvg("flowchart TD\nA --> B");
  console.log(svg);
} finally {
  await engine.dispose();
}
```

The package embeds a Node-targeted wasm-bindgen artifact and does not depend on `@mermanjs/web`.
It is intended for Node-only server-side rendering and static generation. The WASM transport is
single-threaded, so an `AbortSignal` cannot interrupt an operation after it enters the candidate;
operation deadlines remain available through `timeoutMs`.

`renderSvg()` and `renderSvgSync()` use the `parity` pipeline by default. This preserves the
Mermaid-compatible SVG structure for trusted input and parity-sensitive consumers. For SVG that
will be embedded directly into HTML, or when the Mermaid source is not fully trusted, select the
sealed `resvg-safe` pipeline through `optionsJson`:

```js
const svg = await engine.renderSvg(source, {
  optionsJson: JSON.stringify({ svg: { pipeline: "resvg-safe" } }),
});
```

The pipeline values are `parity`, `readable`, and `resvg-safe`. The safe pipeline does not provide
browser `Document` or owner-document admission APIs; use `@mermanjs/web*` for browser DOM mounting.

The native package is the recommended choice when a supported platform binary is available. Use
this package when a deployment target cannot load the native package or when a single portable
Node artifact is more useful than native startup and throughput.
