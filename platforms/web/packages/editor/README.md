# @mermanjs/web-editor

Parser-backed diagnostics, symbols, and editor intelligence for browser-based Mermaid editors. Use
retained sessions from a dedicated module Worker when the editor benefits from an isolated runtime.

This guide targets the unpublished `0.10.0-alpha.2` release candidate with Web transport API `9`,
facts schema `3` and Options JSON schema `3`. Build matching facade, glue and WASM from one reviewed
source revision before publication; the registry command below applies after publication.
Published `0.10.0-alpha.1` retains Web transport API `8`, facts schema `2` and Options JSON schema `3`.
See the
[development migration](https://github.com/Latias94/merman/blob/main/docs/release/UNRELEASED_CONTRACT_VERSIONS.md#source-appearance-and-render-evidence-follow-up). Check the
[prebuilt product status](https://github.com/Latias94/merman/blob/main/docs/release/PREBUILT_PRODUCTS.md)
and [publish order](https://github.com/Latias94/merman/blob/main/docs/release/PUBLISH_ORDER.md)
for verified public availability.

## Quick start

```sh
npm install --save-exact @mermanjs/web-editor@0.10.0-alpha.2
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
for runtime lifecycle, resource policy, package selection, and source-checkout development. See the
[current changelog](https://github.com/Latias94/merman/blob/main/CHANGELOG.md) for changes and
upgrade actions.
