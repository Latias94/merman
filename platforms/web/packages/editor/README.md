# @mermanjs/web-editor

Browser-only Merman package for parser-backed editor intelligence. Use it from a dedicated module Worker when an editor workflow has independently justified a split from the complete package.

## Install

This package belongs to the experimental Merman `0.8.0-alpha.5` browser group. Install the current alpha and verify the resolved version and provenance before depending on prerelease-only behavior:

```sh
npm install @mermanjs/web-editor@alpha
```

For local source development, build the package group and install this package from the checkout:

```sh
npm ci --prefix /path/to/merman/platforms/web
npm run build --prefix /path/to/merman/platforms/web
npm install /path/to/merman/platforms/web/packages/editor
```

```ts
import {
  createEditorSession,
  editorSearchDocumentSymbols,
  initMerman,
} from "@mermanjs/web-editor";

await initMerman();
const session = createEditorSession(
  `flowchart TD
  A -->`,
  1,
  "file:///diagram.mmd",
);
const diagnostics = session.diagnostics();
const symbols = session.searchDocumentSymbols("A");
const oneShotSymbols = editorSearchDocumentSymbols(
  "flowchart TD\nA --> B",
  "A",
);
session.dispose();
```

For one-shot queries without a retained session, call `editorSearchDocumentSymbols(source, query)`. The search is scoped to the supplied document; it does not scan a workspace.

The package exports analysis and editor workflows, but intentionally exposes no callable SVG or ASCII rendering workflow. Shared package-group catalogs and types remain available for integration code. It requires a browser main-thread or Web Worker realm for WASM loading and is not a Node.js or SSR transport.

See the [browser package guide](https://github.com/Latias94/merman/blob/main/platforms/web/README.md) for lifecycle and resource-policy guidance.
