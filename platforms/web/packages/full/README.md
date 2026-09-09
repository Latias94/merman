# @mermanjs/web

The complete browser SDK for SVG and DrawingList rendering, analysis, ASCII output, and parser-backed editor
intelligence. Start here when evaluating Merman or when one browser realm needs more than one
workflow.

This package is published on npm's `alpha` dist-tag. Pin an exact version when reproducible installs
matter.

The `alpha` tag currently resolves to immutable `0.8.0-alpha.6`, which does not include DrawingList.
The DrawingList API below requires a package built from current unreleased source or a later
matching release.

## Quick start

```sh
npm install @mermanjs/web@alpha
```

```ts
import { initMerman, renderSvgToElement } from "@mermanjs/web";

await initMerman();

const target = document.querySelector("#diagram");
if (!target) throw new Error("missing #diagram mount point");

renderSvgToElement(target, `flowchart TD
  A[Start] --> B[Done]`);
```

Use `renderSvg()` when the host needs the serialized SVG string instead of a mounted element.
In current unreleased source, use `renderDrawingList()` when a Canvas, WebGL, or other graphics host
wants the renderer-neutral v1 document instead of SVG.

## Runtime boundary

Load this package only in a browser main thread or Web Worker. It is not a Node.js or SSR transport;
use [`@mermanjs/node`](https://github.com/Latias94/merman/blob/main/platforms/node/packages/node/README.md)
for supported native Node.js hosts.

Initialize Merman once per realm and reuse it. A host that creates a Worker owns Worker termination.
Dispose retained editor and text-measurement sessions when they are no longer needed.

See the [browser package guide](https://github.com/Latias94/merman/blob/main/platforms/web/README.md)
for package selection, safe DOM mounting, custom WASM loading, resource limits, and source-checkout
development.
