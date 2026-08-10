# @mermanjs/web-ascii

Browser-only Merman SDK for supported ASCII diagram output.

## Install

This package has shipped since Merman `0.8.0-alpha.5`. Install the current alpha:

```sh
npm install @mermanjs/web-ascii@alpha
```

For local source development, build the package group and install this package from the checkout:

```sh
npm ci --prefix /path/to/merman/platforms/web
npm run build --prefix /path/to/merman/platforms/web
npm install /path/to/merman/platforms/web/packages/ascii
```

```ts
import { initMerman, renderAscii } from "@mermanjs/web-ascii";

await initMerman();
const output = renderAscii(`flowchart TD
  A[Start] --> B[Done]`);
```

This package intentionally exposes no callable SVG-rendering, semantic-analysis, or editor-session workflow. Shared package-group catalogs and types remain available for integration code. It requires a browser main-thread or Web Worker realm for WASM loading and is not a Node.js or SSR transport.

See the [browser package guide](https://github.com/Latias94/merman/blob/main/platforms/web/README.md) for capability and resource-policy details.
