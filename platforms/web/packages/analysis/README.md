# @mermanjs/web-analysis

Detect, validate, and analyze Mermaid source in a browser without shipping SVG rendering, ASCII
output, or retained editor sessions.

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
npm install --save-exact @mermanjs/web-analysis@0.10.0-alpha.2
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
for lint ownership and adapter boundaries, and the
[current changelog](https://github.com/Latias94/merman/blob/main/CHANGELOG.md) for changes and
upgrade actions.
