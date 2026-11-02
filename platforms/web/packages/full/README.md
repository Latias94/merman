# @mermanjs/web

The complete browser SDK for SVG rendering, analysis, ASCII output, and parser-backed editor
intelligence. Start here when evaluating Merman or when one browser realm needs more than one
workflow.

This guide targets the `0.10.0-alpha.1` release candidate, with Web transport API `8` and Options
JSON schema `3`. Check the
[prebuilt product status](https://github.com/Latias94/merman/blob/main/docs/release/PREBUILT_PRODUCTS.md)
and [publish order](https://github.com/Latias94/merman/blob/main/docs/release/PUBLISH_ORDER.md)
for verified public availability.

The package includes math, Cytoscape and ELK for SVG rendering. Its analysis, editor and ASCII
operations share one initialized runtime.

## Quick start

```sh
npm install --save-exact @mermanjs/web@0.10.0-alpha.1
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
Use `prepareSource()` for repeated SVG, layout and geometry outputs from frozen parse facts.
Dispose the returned snapshot when finished; see the
[prepared-source guide](https://github.com/Latias94/merman/blob/main/platforms/web/README.md#reuse-a-parsed-source).

For retained DOM updates and explicit Kanban width adaptation, use
[`mountDiagram()`](https://github.com/Latias94/merman/blob/main/platforms/web/README.md#maintain-a-mounted-diagram).

## Runtime boundary

Load this package only in a browser main thread or Web Worker. It is not a Node.js or SSR transport;
use [`@mermanjs/node`](https://github.com/Latias94/merman/blob/main/platforms/node/packages/node/README.md)
for supported native Node.js hosts.

Initialize Merman once per realm and reuse it. A host that creates a Worker owns Worker termination.
Dispose retained editor and text-measurement sessions when they are no longer needed.

See the [browser package guide](https://github.com/Latias94/merman/blob/main/platforms/web/README.md)
for package selection, safe DOM mounting, custom WASM loading, resource limits, and source-checkout
development. See the [current changelog](https://github.com/Latias94/merman/blob/main/CHANGELOG.md)
for changes and upgrade actions.
