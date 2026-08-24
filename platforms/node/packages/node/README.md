# @mermanjs/node

Native Mermaid-compatible SVG rendering for Node.js 22+ and static-site build pipelines, without
Chromium, a browser-WASM fallback, or a postinstall binary download.

This package is experimental and published on npm's `alpha` dist-tag. Pin an exact version when
reproducible installs matter.

## Quick start

Install the loader package, not a platform package:

```sh
npm install @mermanjs/node@alpha
```

```js
import { createNodeEngine } from "@mermanjs/node";

const engine = await createNodeEngine();

try {
  const svg = await engine.renderSvg("flowchart TD\nA --> B");
  console.log(svg);
} finally {
  await engine.dispose();
}
```

Create an engine once, reuse it for related work, and dispose it during teardown.

## SVG output pipelines

`renderSvg()` and `renderSvgSync()` use the `parity` pipeline by default. This preserves the
Mermaid-compatible SVG structure for trusted input and parity-sensitive consumers. `parity` is
not a browser DOM-admission or sanitization check.

For SVG that will be embedded directly into HTML, or when the Mermaid source is not fully trusted,
select the sealed `resvg-safe` pipeline explicitly:

```js
const options = {
  optionsJson: JSON.stringify({ svg: { pipeline: "resvg-safe" } }),
};
const svg = await engine.renderSvg(source, options);
```

The supported pipeline values are `parity`, `readable`, and `resvg-safe`. The safe pipeline can
remove browser-only SVG features such as active content and `foreignObject` labels. It does not
provide browser `Document` or owner-document admission APIs; use `@mermanjs/web*` for browser DOM
mounting.

## Supported hosts

- macOS arm64 and x64
- Linux x64 with glibc or musl
- Windows x64 with MSVC
- Node.js `>=22`

The root loader selects one matching exact-version `@mermanjs/node-<platform>` optional dependency.
Do not install platform packages directly. The loader contains no native or WASM binary and never
downloads one during installation or runtime.

If the native addon is installed but the host dynamic loader rejects it (for example because the
glibc baseline is too new), construction throws `MermanNativeLoadError` with the target package
name and an explicit suggestion to use `@mermanjs/node-wasm`. It does not silently switch
transports.

## Capability boundary

The shipped recipe provides deterministic static SVG, Cytoscape and ELK layouts, metadata, layout
plans, runtime-catalog inspection, and generic admitted operations. Math, binary export, analysis,
ASCII, text-measurement callbacks, browser fallback, and runtime downloads are outside this package
surface. Requests for unavailable optional capabilities return Merman's typed
missing-capability error.

## Lifecycle and concurrency

The Promise-based API uses a bounded queue. An `AbortSignal` cancels queued work immediately and
requests cooperative cancellation after Rust work starts; started work stops at the next renderer
or transport-admission checkpoint and reports Merman's typed cancellation details. Per-operation
`timeoutMs` values use a monotonic Rust deadline and must be integers from 0 through 4,294,967,295.
`dispose()` waits for active work to settle and rejects new work once teardown begins.

Use `renderSvgSync()` only for an explicitly synchronous static-site build path. It refuses to run
while asynchronous operations are active or queued.

See the [Node.js package guide](https://github.com/Latias94/merman/blob/main/platforms/node/README.md)
for source-checkout development, package verification, and release design.
