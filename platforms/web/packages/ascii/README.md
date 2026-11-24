# @mermanjs/web-ascii

Render supported Mermaid diagram families as ASCII or Unicode text in a browser.

Version `0.11.0` uses Web transport API `12` and Options JSON schema `3`. Keep the facade, generated glue, and WASM at the same exact version and source revision. Check the [host installation guide](https://github.com/Latias94/merman/blob/main/docs/release/PREBUILT_PRODUCTS.md#published-versions) for verified versions and channel availability.

Use `renderAsciiResult()` to read ASCII schema `4` output metadata. Update report readers for the nullable primary layout and separate emitted overflow. [ASCII report migration](https://github.com/Latias94/merman/blob/main/docs/migrations/render-reuse-host-integration.md#ascii-output-schema-3-to-4)

Published `0.10.0-alpha.2` retains Web transport API `9`. Version `0.10.0-alpha.1` retains Web transport API `8` and Options JSON schema `3`. Upgrade all package assets together; see the [contract migration](https://github.com/Latias94/merman/blob/main/docs/release/UNRELEASED_CONTRACT_VERSIONS.md#source-appearance-and-render-evidence-follow-up) for the transport change.

## Quick start

```sh
npm install --save-exact @mermanjs/web-ascii@0.11.0
```

```ts
import { initMerman, renderAscii } from "@mermanjs/web-ascii";

await initMerman();

const output = renderAscii(`flowchart TD
  A[Start] --> B[Done]`, {
  theme: { preset: "studio", appearance: "dark" },
  ascii: { color_mode: "truecolor" },
});
console.log(output);
```

Use `themeCatalog()` to discover native presets and their available appearances. Pass a preset ID
or a v2 theme document to color truecolor and ANSI256 output.

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
