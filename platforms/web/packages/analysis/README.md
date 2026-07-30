# @mermanjs/web-analysis

Browser-only Merman semantic analysis, validation, and diagram detection without SVG rendering, ASCII rendering, or editor-session workflow implementations. Shared package-group catalogs and types remain available for integration code.

## Install

Build the local package group and install this package:

```sh
npm ci --prefix /path/to/merman/platforms/web
npm run build --prefix /path/to/merman/platforms/web
npm install /path/to/merman/platforms/web/packages/analysis
```

```ts
import { analyze, initMerman } from "@mermanjs/web-analysis";

await initMerman();
const result = analyze(`flowchart TD
  A --> B`);
```

Use this package for browser linting, detection, and metadata workflows. It intentionally cannot render SVG or ASCII and cannot create editor sessions. It requires a browser main-thread or Web Worker realm for WASM loading and is not a Node.js or SSR transport.

See the [browser package guide](https://github.com/Latias94/merman/blob/main/platforms/web/README.md) for resource policy and custom loading.
