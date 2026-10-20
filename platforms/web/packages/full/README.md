# @mermanjs/web

The complete browser SDK for SVG rendering, analysis, ASCII output, and parser-backed editor
intelligence. Start here when evaluating Merman or when one browser realm needs more than one
workflow.

Pin an exact package version when reproducible installs matter. Current development builds omit
math; custom WASM source builds can enable `math` explicitly. Published `0.9.1` retains its
original capability set.

## Quick start

```sh
npm install @mermanjs/web@0.9.1
```

```ts
import { initMerman, renderSvgToElement } from "@mermanjs/web";

await initMerman();

const target = document.querySelector("#diagram");
if (!target) throw new Error("missing #diagram mount point");

renderSvgToElement(target, `flowchart TD
  A[Start] --> B[Done]`);
```

Use `renderSvg()` when the host needs the serialized SVG string instead of a mounted element. Source builds also
expose `prepareSource()` for repeated SVG, layout and geometry outputs from frozen parse facts.
Dispose the returned snapshot when finished; see the
[prepared-source guide](https://github.com/Latias94/merman/blob/main/platforms/web/README.md#reuse-a-parsed-source).


## Runtime boundary

Load this package only in a browser main thread or Web Worker. It is not a Node.js or SSR transport;
use [`@mermanjs/node`](https://github.com/Latias94/merman/blob/main/platforms/node/packages/node/README.md)
for supported native Node.js hosts.

Initialize Merman once per realm and reuse it. A host that creates a Worker owns Worker termination.
Dispose retained editor and text-measurement sessions when they are no longer needed.

See the [browser package guide](https://github.com/Latias94/merman/blob/main/platforms/web/README.md)
for package selection, safe DOM mounting, custom WASM loading, resource limits, and source-checkout
development.
