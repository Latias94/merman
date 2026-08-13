# @mermanjs/web-analysis

Detect, validate, and analyze Mermaid source in a browser without shipping SVG rendering, ASCII
output, or retained editor sessions.

This package is published on npm's `alpha` dist-tag. Pin an exact version when reproducible installs
matter.

## Quick start

```sh
npm install @mermanjs/web-analysis@alpha
```

```ts
import { analyzeDocument, initMerman } from "@mermanjs/web-analysis";

await initMerman();

const markdownSource =
  "# Diagram\n\n```mermaid\nflowchart TD\n  A -->\n```\n";
const result = analyzeDocument(
  markdownSource,
  "file:///workspace/README.md",
  { lint: { profile: "recommended" } },
);

if (!result.valid) {
  console.error(result.diagnostics);
}
```

The URI extension selects standalone Mermaid, Markdown, or MDX modeling. Diagnostics and fixes use
host-document coordinates.

## Scope

Use this package for browser linting, detection, and metadata workflows. It intentionally cannot
render SVG or ASCII and cannot create editor sessions. Load it only in a browser main thread or Web
Worker. Node.js and SSR analysis integrations should use `merman-cli`; use `@mermanjs/node` only
for in-process SVG rendering.

See the [browser package guide](https://github.com/Latias94/merman/blob/main/platforms/web/README.md)
for custom WASM loading, resource policy, package selection, and source-checkout development. See
the [integration guide](https://github.com/Latias94/merman/blob/main/docs/integrations/README.md)
for lint ownership and adapter boundaries.
