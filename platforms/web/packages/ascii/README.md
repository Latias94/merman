# @mermanjs/web-ascii

Browser-only Merman SDK for supported ASCII diagram output.

## Install

<!-- BEGIN GENERATED RELEASE README NPM_ASCII_INSTALL -->

The `0.8.0-alpha.4` candidate is not published yet. Build the browser workspace from a checkout, then install this profile into the application from its local package directory:

```sh
npm ci --prefix /path/to/merman/platforms/web
npm run build --prefix /path/to/merman/platforms/web
npm install /path/to/merman/platforms/web/packages/ascii
```

<!-- END GENERATED RELEASE README NPM_ASCII_INSTALL -->

```ts
import { initMerman, renderAscii } from "@mermanjs/web-ascii";

await initMerman();
const output = renderAscii(`flowchart TD
  A[Start] --> B[Done]`);
```

This package intentionally does not export SVG rendering, semantic analysis, or editor sessions. It requires a browser main-thread or Web Worker realm for WASM loading and is not a Node.js or SSR transport.

See the [browser package guide](https://github.com/Latias94/merman/blob/main/platforms/web/README.md) for capability and resource-policy details.
