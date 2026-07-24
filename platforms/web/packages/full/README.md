# @mermanjs/web

The complete browser-only Merman SDK. It includes SVG rendering, semantic analysis, ASCII output,
and parser-backed editor APIs in one matching WASM artifact.

## Install

When this package surface is released:

```sh
npm install @mermanjs/web
```

```ts
import { initMerman, renderSvg } from "@mermanjs/web";

await initMerman();
const svg = renderSvg("flowchart TD\\n  A[Start] --> B[Done]");
```

Use this package for the first implementation and for applications that need more than one Merman
workflow. It is not a Node.js or SSR transport. Load it only in a browser main-thread or Web Worker
realm; a separate native transport is required for server-side rendering.

See the [browser package guide](https://github.com/Latias94/merman/blob/main/platforms/web/README.md)
for custom WASM loading, resource policy, and the slim package admission rules.
