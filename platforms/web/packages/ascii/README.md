# @mermanjs/web-ascii

Browser-only Merman SDK for supported ASCII diagram output.

## Install

Use the generated command for the current repository release state:

<!-- BEGIN GENERATED RELEASE README NPM_ASCII_INSTALL -->

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

This package intentionally exposes no callable SVG-rendering, semantic-analysis, or editor-session workflow. Shared package-group catalogs and types remain available for integration code. It requires a browser main-thread or Web Worker realm for WASM loading and is not a Node.js or SSR transport.

See the [browser package guide](https://github.com/Latias94/merman/blob/main/platforms/web/README.md) for capability and resource-policy details.
