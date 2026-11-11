# @mermanjs/web

The complete browser SDK for SVG rendering, analysis, ASCII output, and parser-backed editor
intelligence. Start here when evaluating Merman or when one browser realm needs more than one
workflow.

Version `0.10.0` uses Web transport API `10`, facts schema `3`, theme execution evidence schema `2`, and Options JSON schema `3`. Keep the facade, generated glue, and WASM at the same exact version and source revision. Check the [host installation guide](https://github.com/Latias94/merman/blob/main/docs/release/PREBUILT_PRODUCTS.md#published-versions) for verified versions and channel availability.

Published `0.10.0-alpha.2` retains Web transport API `9`. Version `0.10.0-alpha.1` retains Web transport API `8`, facts schema `2`, theme execution evidence schema `1`, and Options JSON schema `3`. Upgrade all package assets together and update JSON consumers for the [source appearance and render-evidence changes](https://github.com/Latias94/merman/blob/main/docs/release/UNRELEASED_CONTRACT_VERSIONS.md#source-appearance-and-render-evidence-follow-up).

The prebuilt package includes math, Cytoscape and ELK for SVG rendering. Its analysis, editor and ASCII
operations share one initialized runtime.

## Quick start

```sh
npm install --save-exact @mermanjs/web@0.10.0
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

## Themes

Use `themeCatalog()` to discover preset designs and appearances, `renderSvgResult()` with
`{ theme: { preset: "paper-cut" } }` to select one, and `exportThemePreset("paper-cut")` to obtain
its editable JSON recipe. See the [theme workflow](https://github.com/Latias94/merman/blob/main/platforms/web/README.md#choose-a-preset-for-a-diagram)
and [customization guide](https://github.com/Latias94/merman/blob/main/docs/rendering/custom-diagram-themes.md)
for shared tokens, paired appearances, JSON import/export, and actual render evidence.

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
