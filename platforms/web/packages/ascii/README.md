# @mermanjs/web-ascii

Render supported Mermaid diagram families as ASCII or Unicode text in a browser.

This guide targets the `0.10.0-alpha.1` release candidate, with Web transport API `8` and Options
JSON schema `3`. Check the
[prebuilt product status](https://github.com/Latias94/merman/blob/main/docs/release/PREBUILT_PRODUCTS.md)
and [publish order](https://github.com/Latias94/merman/blob/main/docs/release/PUBLISH_ORDER.md)
for verified public availability.

## Quick start

```sh
npm install --save-exact @mermanjs/web-ascii@0.10.0-alpha.1
```

```ts
import { initMerman, renderAscii } from "@mermanjs/web-ascii";

await initMerman();

const output = renderAscii(`flowchart TD
  A[Start] --> B[Done]`);
console.log(output);
```

## Scope

This package exposes no callable SVG rendering, semantic analysis, or editor sessions. Load it only
in a browser main thread or Web Worker; Node.js and SSR integrations should use a native surface.

ASCII and Unicode coverage is explicit by diagram family. Check the [ASCII support
matrix](https://github.com/Latias94/merman/blob/main/docs/rendering/ASCII_SUPPORT_MATRIX.md)
before choosing this package for a required diagram set.

See the [browser package guide](https://github.com/Latias94/merman/blob/main/platforms/web/README.md)
for package selection, runtime lifecycle, resource policy, and source-checkout development. See the
[current changelog](https://github.com/Latias94/merman/blob/main/CHANGELOG.md) for changes and
upgrade actions.
