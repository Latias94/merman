# @mermanjs/web-editor

Parser-backed diagnostics, symbols, and editor intelligence for browser-based Mermaid editors. Use
retained sessions from a dedicated module Worker when the editor benefits from an isolated runtime.

This package is published on npm's `alpha` dist-tag. Pin an exact version when reproducible installs
matter.

## Quick start

```sh
npm install @mermanjs/web-editor@alpha
```

```ts
import { createEditorSession, initMerman } from "@mermanjs/web-editor";

await initMerman();

const session = createEditorSession(
  `flowchart TD
  A -->`,
  1,
  "file:///diagram.mmd",
);

try {
  const diagnostics = session.diagnostics();
  const symbols = session.searchDocumentSymbols("A");
  console.log({ diagnostics, symbols });
} finally {
  session.dispose();
}
```

For one-shot queries without retained state, use functions such as
`editorSearchDocumentSymbols(source, query, uri)`. Search remains scoped to the supplied document;
it does not scan a workspace.

## Scope and lifecycle

The package includes analysis and editor workflows but no callable SVG or ASCII rendering. Load it
only in a browser main thread or Web Worker. Dispose every retained session, and terminate a Worker
after failure, replacement, or application teardown.

See the [browser package guide](https://github.com/Latias94/merman/blob/main/platforms/web/README.md)
for runtime lifecycle, resource policy, package selection, and source-checkout development.
