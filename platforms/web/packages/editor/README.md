# @mermanjs/web-editor

Browser-only Merman package for parser-backed editor intelligence. Use it from a dedicated module
Worker when an editor workflow has independently justified a split from the complete package.

## Install

When this package surface is released:

```sh
npm install @mermanjs/web-editor
```

```ts
import { createEditorSession, initMerman } from "@mermanjs/web-editor";

await initMerman();
const session = createEditorSession("flowchart TD\\n  A -->", 1, "file:///diagram.mmd");
const diagnostics = session.diagnostics();
session.dispose();
```

The package exports analysis and editor APIs, but intentionally does not export SVG or ASCII
rendering. It requires a browser main-thread or Web Worker realm for WASM loading and is not a
Node.js or SSR transport.

See the [browser package guide](https://github.com/Latias94/merman/blob/main/platforms/web/README.md)
for lifecycle and resource-policy guidance.
