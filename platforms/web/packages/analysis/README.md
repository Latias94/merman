# @mermanjs/web-analysis

Browser-only Merman semantic analysis, validation, and diagram detection without SVG, ASCII, or
editor-session exports.

## Install

When this package surface is released:

```sh
npm install @mermanjs/web-analysis
```

```ts
import { analyze, initMerman } from "@mermanjs/web-analysis";

await initMerman();
const result = analyze("flowchart TD\\n  A --> B");
```

Use this package for browser linting, detection, and metadata workflows. It intentionally cannot
render SVG or ASCII and cannot create editor sessions. It requires a browser main-thread or Web
Worker realm for WASM loading and is not a Node.js or SSR transport.

See the [browser package guide](https://github.com/Latias94/merman/blob/main/platforms/web/README.md)
for resource policy and custom loading.
