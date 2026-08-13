# @mermanjs/web-ascii

Render supported Mermaid diagram families as ASCII or Unicode text in a browser.

This package is published on npm's `alpha` dist-tag. Pin an exact version when reproducible installs
matter.

## Quick start

```sh
npm install @mermanjs/web-ascii@alpha
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
for package selection, runtime lifecycle, resource policy, and source-checkout development.
