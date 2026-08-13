# Merman for Node.js

`@mermanjs/node` is Merman's experimental native package for deterministic SVG rendering in
Node.js 22+ and static-site builds. A small loader selects one exact-version native package for the
current host; it has no browser-WASM fallback or postinstall downloader.

Application developers should start with the [`@mermanjs/node` package guide](packages/node/README.md).

## Quick start

```sh
npm install @mermanjs/node@alpha
```

```js
import { createNodeEngine } from "@mermanjs/node";

const engine = await createNodeEngine();

try {
  const svg = await engine.renderSvg("flowchart TD\nA --> B");
  console.log(svg);
} finally {
  await engine.dispose();
}
```

The package supports macOS arm64/x64, Linux x64 glibc/musl, and Windows x64 MSVC. Its shipped
recipe includes deterministic SVG plus Cytoscape and ELK layouts. Math, binary export, analysis,
ASCII, text-measurement callbacks, browser fallback, and runtime downloads remain outside this
surface.

## Package layout

- `packages/node` contains the public ESM loader, JavaScript engine, declarations, and user README.
- Generated `@mermanjs/node-<platform>` packages contain the native binary for one supported host.
- The Node-targeted WASM implementation is an internal comparison transport and is never selected
  by the public loader.

Install only `@mermanjs/node`; its exact-version optional dependencies keep the selected platform
package aligned with the loader.

## Develop and verify

```sh
npm ci --prefix platforms/node
npm test --prefix platforms/node
npm run check:packages --prefix platforms/node
```

`npm test` exercises the JavaScript API, bounded executor, catalog validation, loader behavior, and
benchmark contract with test transports. `check:packages` verifies the source manifests; native
candidate assembly and installed-package smoke run in the target-specific release workflow.

The package group is versioned and published in lockstep. See the [package surface
guide](../../docs/release/PACKAGE_SURFACES.md) for the artifact, target, provenance, and release
contract.
