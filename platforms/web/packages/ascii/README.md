# @mermanjs/web-ascii

Browser-only Merman SDK for supported ASCII diagram output.

## Install

When this package surface is released:

```sh
npm install @mermanjs/web-ascii
```

```ts
import { initMerman, renderAscii } from "@mermanjs/web-ascii";

await initMerman();
const output = renderAscii("flowchart TD\\n  A[Start] --> B[Done]");
```

This package intentionally does not export SVG rendering, semantic analysis, or editor sessions.
It requires a browser main-thread or Web Worker realm for WASM loading and is not a Node.js or SSR
transport.

See the [browser package guide](https://github.com/Latias94/merman/blob/main/platforms/web/README.md)
for capability and resource-policy details.
