# @mermanjs/web

The complete browser-only Merman SDK. Its single WASM artifact includes SVG rendering with Cytoscape, ELK, and math support, semantic analysis, ASCII output, and parser-backed editor APIs.

## Install

Install the current alpha from npm:

```sh
npm install @mermanjs/web@alpha
```

For local source development, build the package group and install this package from the checkout:

```sh
npm ci --prefix /path/to/merman/platforms/web
npm run build --prefix /path/to/merman/platforms/web
npm install /path/to/merman/platforms/web/packages/full
```

```ts
import { initMerman, renderSvg } from "@mermanjs/web";

await initMerman();
const svg = renderSvg(`flowchart TD
  A[Start] --> B[Done]`);
```

Use this package for the first implementation and for applications that need more than one Merman workflow. It is not a Node.js or SSR transport. Load it only in a browser main-thread or Web Worker realm; a separate native transport is required for server-side rendering.

See the [browser package guide](https://github.com/Latias94/merman/blob/main/platforms/web/README.md) for custom WASM loading, resource policy, and the slim package admission rules.
