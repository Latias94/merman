# @mermanjs/web

The complete browser-only Merman SDK. Its single WASM artifact includes SVG rendering with Cytoscape, ELK, and math support, semantic analysis, ASCII output, and parser-backed editor APIs.

## Install

<!-- BEGIN GENERATED RELEASE README NPM_FULL_INSTALL -->

The `0.8.0-alpha.4` candidate is not published yet. Build the browser workspace from a checkout, then install this profile into the application from its local package directory:

```sh
npm ci --prefix /path/to/merman/platforms/web
npm run build --prefix /path/to/merman/platforms/web
npm install /path/to/merman/platforms/web/packages/full
```

<!-- END GENERATED RELEASE README NPM_FULL_INSTALL -->

```ts
import { initMerman, renderSvg } from "@mermanjs/web";

await initMerman();
const svg = renderSvg(`flowchart TD
  A[Start] --> B[Done]`);
```

Use this package for the first implementation and for applications that need more than one Merman workflow. It is not a Node.js or SSR transport. Load it only in a browser main-thread or Web Worker realm; a separate native transport is required for server-side rendering.

See the [browser package guide](https://github.com/Latias94/merman/blob/main/platforms/web/README.md) for custom WASM loading, resource policy, and the slim package admission rules.
