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

## Supported hosts

- macOS arm64 and x64
- Linux x64 with glibc or musl
- Windows x64 with MSVC
- Node.js `>=22`

The root loader selects one matching exact-version `@mermanjs/node-<platform>` optional dependency.
Do not install platform packages directly. The loader contains no native or WASM binary and never
downloads one during installation or runtime.

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
