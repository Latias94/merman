# Merman for Node.js

`@mermanjs/node` is Merman's experimental native package for deterministic SVG rendering in
Node.js 22+ and static-site builds. A small loader selects one exact-version native package for the
current host; it has no browser-WASM fallback or postinstall downloader.

Application developers should start with the [`@mermanjs/node` package guide](packages/node/README.md).

## Quick start

The npm `alpha` dist-tag remains on the published alpha.5 group while the verified alpha.6 group
awaits the first-publication bootstrap for `@mermanjs/node-wasm`. Use alpha.5 for registry installs
until all seven alpha.6 packages are published, or build the exact reviewed commit when validating
alpha.6 behavior.

```sh
npm install @mermanjs/node@alpha
```

When validating alpha.6 from source, pin the exact commit accepted by release preflight rather than a moving branch.

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

## SVG output pipelines

`renderSvg()` and `renderSvgSync()` use the `parity` pipeline by default. This preserves the
Mermaid-compatible SVG structure and is the right choice for trusted input or parity-sensitive
consumers. `parity` is not a browser DOM-admission or sanitization check.

For SVG that will be embedded directly into HTML, or when the Mermaid source is not fully trusted,
select the sealed `resvg-safe` pipeline explicitly:

```js
const options = {
  optionsJson: JSON.stringify({ svg: { pipeline: "resvg-safe" } }),
};
const svg = await engine.renderSvg(source, options);
```

The supported pipeline values are `parity`, `readable`, and `resvg-safe`. The safe pipeline can
remove browser-only SVG features such as active content and `foreignObject` labels. It does not
provide browser `Document` or owner-document admission APIs; use `@mermanjs/web*` for browser DOM
mounting.

The package supports macOS arm64/x64, Linux x64 glibc/musl, and Windows x64 MSVC. Its shipped
recipe includes deterministic SVG plus Cytoscape and ELK layouts. Math, binary export, analysis,
ASCII, text-measurement callbacks, browser fallback, and runtime downloads remain outside this
surface.

## Package layout

- `packages/node` contains the public ESM loader, JavaScript engine, declarations, and user README.
- Generated `@mermanjs/node-<platform>` packages contain the native binary for one supported host.
- `@mermanjs/node-wasm` is a separate explicit Node-targeted WASM package for deployments that
  cannot load a native addon. It does not reuse `@mermanjs/web`, and its single-threaded execution
  cannot observe a mid-call JavaScript `AbortSignal`.

Install `@mermanjs/node` for native execution or `@mermanjs/node-wasm` when a portable Node
artifact is preferred. The native loader's exact-version optional dependencies keep the selected
platform package aligned with the loader; it never silently changes transport.

Native cancellation is cooperative after JavaScript queue admission. The bridge does not forcibly
remove work already submitted to libuv; the Promise settles when that worker observes cancellation
at transport admission or a later renderer checkpoint, preserving the canonical typed envelope.

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
