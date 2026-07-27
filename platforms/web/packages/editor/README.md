# @mermanjs/web-editor

Browser-only Merman package for parser-backed editor intelligence. Use it from a dedicated module Worker when an editor workflow has independently justified a split from the complete package.

## Install

<!-- BEGIN GENERATED RELEASE README NPM_EDITOR_INSTALL -->

The `0.8.0-alpha.4` candidate is not published yet. Build the browser workspace from a checkout, then install this profile into the application from its local package directory:

```sh
npm ci --prefix /path/to/merman/platforms/web
npm run build --prefix /path/to/merman/platforms/web
npm install /path/to/merman/platforms/web/packages/editor
```

<!-- END GENERATED RELEASE README NPM_EDITOR_INSTALL -->

```ts
import { createEditorSession, initMerman } from "@mermanjs/web-editor";

await initMerman();
const session = createEditorSession(
  `flowchart TD
  A -->`,
  1,
  "file:///diagram.mmd",
);
const diagnostics = session.diagnostics();
session.dispose();
```

The package exports analysis and editor APIs, but intentionally does not export SVG or ASCII rendering. It requires a browser main-thread or Web Worker realm for WASM loading and is not a Node.js or SSR transport.

See the [browser package guide](https://github.com/Latias94/merman/blob/main/platforms/web/README.md) for lifecycle and resource-policy guidance.
