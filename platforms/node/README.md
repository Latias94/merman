# Merman for Node.js

`@mermanjs/node` is Merman's experimental native package for deterministic SVG rendering in
Node.js 22+ and static-site builds. A small loader selects one exact-version native package for the
current host; it has no browser-WASM fallback or postinstall downloader.

This guide targets the `0.10.0-alpha.1` release candidate. Check the
[prebuilt product status](../../docs/release/PREBUILT_PRODUCTS.md) and
[publish order](../../docs/release/PUBLISH_ORDER.md) for verified public availability. See the
[current Node changelog](CHANGELOG.md) for changes and upgrade actions.

Application developers should start with the [`@mermanjs/node` package guide](packages/node/README.md).

## Quick start

The loader, selected native platform package, and explicit `@mermanjs/node-wasm` transport use one
exact package-group version. The packages target Mermaid `12.2.0` and use
[Options JSON schema `3`](../../docs/bindings/OPTIONS_JSON.md).

```sh
npm install --save-exact @mermanjs/node@0.10.0-alpha.1
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

## SVG output pipelines

`renderSvg()` and `renderSvgSync()` use the `parity` pipeline by default. This preserves the
Mermaid-compatible SVG structure and is the right choice for trusted input or parity-sensitive
consumers. `parity` is not a browser DOM-admission or sanitization check.

For a resvg/usvg consumer that cannot display HTML labels, select `resvg-safe` explicitly:

```js
const options = {
  optionsJson: JSON.stringify({ svg: { pipeline: "resvg-safe" } }),
};
const svg = await engine.renderSvg(source, options);
```

The supported pipeline values are `parity`, `readable`, and `resvg-safe`. `readable` is an advanced
text overlay for consumers that ignore HTML labels; browsers can display both copies and show
overlapping text. Use `parity` for browser display. `resvg-safe` also removes unsupported styles,
so it is not a lossless replacement for `readable`. HTML embedding still requires a host-owned
admission policy; selecting a pipeline alone does not establish browser DOM safety.

The `resvg-safe` pipeline removes browser-only content such as active elements and `foreignObject`
labels. It does not
provide browser `Document` or owner-document admission APIs; use `@mermanjs/web*` for browser DOM
mounting.

The native package supports macOS arm64/x64, Linux arm64/x64 glibc/musl, and Windows x64 MSVC.
Both Node transports include deterministic SVG with math plus Cytoscape and ELK layouts. Binary
export, analysis, ASCII, text-measurement callbacks, browser fallback, and runtime downloads remain
outside this surface.

The [host integration recipes](../../crates/merman/examples/README.md#host-integration-recipes)
describe the library boundary across Rust and other delivery surfaces. This native package can
serve the SVG recipe; it does not expose the ASCII/agent-log or PNG recipe. Use a matching Rust or
CLI build for those outputs instead of inferring package support from workspace capabilities.
The host owns semantic theme selection, caching, scheduling, persistence, and viewer or clipboard
actions. SVG presentation values remain separate from the terminal palette used by ASCII-capable
artifacts. See the [compiled-theme guide](packages/node/README.md#compiled-themes) for preset
selection, discovery, and the theme system's alpha boundaries.

## Loaded icon packs

Both Node packages accept `createNodeEngine({ iconPacks: [{ json, registrationName? }] })`.
`json` is an already loaded Iconify pack JSON string; `registrationName` optionally replaces its
prefix. This constructor service is separate from `bindingOptions` and request options. Each
engine owns an immutable registry, and an omitted or empty array preserves ordinary rendering.
The host owns file loading, network requests, and disposal. See the
[native package example](packages/node/README.md#loaded-icon-packs).

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
artifact assembly and installed-package smoke run in the target-specific release workflow.
The native target lanes install only the loader tarball, resolve its exact optional dependency
through a small local registry, inspect the resulting lockfile, and execute ordinary SVG and
outlined formula rendering on the matching CPU. Both transports also execute on Node 22.0.0.
The GNU lanes build and execute at glibc 2.31; ARM64 uses native ARM64 runners.

Building the Node-WASM candidate requires `wasm-pack 0.15.0` and Binaryen `version_133`.
Put that release's `wasm-opt` on `PATH` before running the build owner, as CI, release preflight,
and the Node release workflow do. The older optimizer downloaded by wasm-pack can fail in its
precomputation pass on the current module. The build retains release optimization.

The Linux GNU builder uses the full Node Bullseye image to preserve the declared glibc 2.31
floor. Its inherited `buildpack-deps` tools and CA bundle are checked during image assembly;
the Dockerfile does not reinstall them from Bullseye's retired package repositories. Slim
images are not interchangeable with this build image. After pulling the base images, the
build steps can be verified without network access:

```sh
docker build --network=none --platform linux/amd64 --file platforms/node/Dockerfile.gnu --tag merman-node-gnu:check platforms/node
```

This keeps the existing compatibility builder usable; it does not restore security maintenance
for Bullseye. A maintained replacement needs separate glibc-floor and native-package evidence.

The package group is versioned and published in lockstep. See the [package surface
guide](../../docs/release/PACKAGE_SURFACES.md) for the artifact, target, provenance, and release
contract.
