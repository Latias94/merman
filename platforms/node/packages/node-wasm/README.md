# @mermanjs/node-wasm

Explicit Node-targeted WebAssembly rendering for Node.js 22+ and static-site build pipelines.
This is a separate opt-in package; `@mermanjs/node` remains the native N-API package and never
silently falls back to browser or Node WASM.

This guide targets the `0.10.0-alpha.1` release candidate with Options JSON schema `3`. Check the
[prebuilt product status](https://github.com/Latias94/merman/blob/main/docs/release/PREBUILT_PRODUCTS.md)
and [publish order](https://github.com/Latias94/merman/blob/main/docs/release/PUBLISH_ORDER.md)
for verified public availability.

## Quick start

```sh
npm install --save-exact @mermanjs/node-wasm@0.10.0-alpha.1
```

```js
import { createNodeEngine } from "@mermanjs/node-wasm";

const engine = await createNodeEngine();

try {
  const svg = await engine.renderSvg("flowchart TD\nA --> B");
  console.log(svg);
} finally {
  await engine.dispose();
}
```

The package embeds a Node-targeted wasm-bindgen artifact and does not depend on `@mermanjs/web`.
It is intended for Node-only server-side rendering and static generation. The WASM transport is
single-threaded, so an `AbortSignal` cannot interrupt an operation after it enters WASM;
operation deadlines remain available through `timeoutMs`.

`renderSvg()` and `renderSvgSync()` use the `parity` pipeline by default. This preserves the
Mermaid-compatible SVG structure for trusted input and parity-sensitive consumers. For a resvg/usvg
consumer that cannot display HTML labels, select `resvg-safe` explicitly through `optionsJson`:

```js
const svg = await engine.renderSvg(source, {
  optionsJson: JSON.stringify({ svg: { pipeline: "resvg-safe" } }),
});
```

The pipeline values are `parity`, `readable`, and `resvg-safe`. `readable` is a text overlay for
consumers that ignore HTML labels; browsers can display both copies and show overlapping text.
Use `parity` for browser display, with a host-owned DOM-admission policy when embedding SVG into
HTML. Pipeline selection alone does not establish browser DOM safety. The `resvg-safe` pipeline
does not provide browser `Document` or owner-document admission APIs; use `@mermanjs/web*` for
browser DOM mounting.

The native package is the recommended choice when a supported platform binary is available. Use
this package when a deployment target cannot load the native package or when a single portable
Node artifact is more useful than native startup and throughput.

The package includes math, Cytoscape and ELK layouts, matching the native SVG recipe.
`engine.renderSvg('flowchart LR\nA["$$x^2$$"]')` renders outlined formula geometry and the runtime
catalog advertises `math`. The default `interactive` resource profile admits formulas; an
explicitly `constrained` profile rejects them even when math is compiled. For untrusted input,
enforce host execution, memory, and concurrency limits alongside the selected resource profile.
Node.js 22.0.0 is the minimum runtime.

## Compiled themes

Select a preset through the top-level `theme` group in Options JSON schema `3`. Discover preset
IDs through the initialized engine's theme catalog:

```js
const catalog = JSON.parse(engine.metadataJson("theme-catalog"));
const themedSvg = await engine.renderSvg("flowchart LR\nA --> B", {
  optionsJson: JSON.stringify({ version: 3, theme: { preset: "spotless" } }),
});
```

Catalog `available` means that the recipe can compile under the current resource policy. All
built-in presets remain `alpha` with empty `qualified_cells`; availability and unknown
profile/admission IDs never establish portable output. Flowchart glow and shadow effects require
the Classic look and supported shapes. SVG fonts resolve on the viewing host; exported recipes
do not embed them.

Use `executeOperation()` or `executeOperationSync()` with operation IDs
`materialize-theme-json`, `describe-theme-support-json`, or `export-theme-preset-json` for
shared definition JSON, support queries and preset export. Materialization returns a complete
spec that can be passed as `theme: { spec }`; no intermediate file is required. Replace old
`presentation.theme` inputs and `presentation-catalog` metadata calls when upgrading. See the
[theme guide](https://github.com/Latias94/merman/blob/main/docs/rendering/custom-diagram-themes.md)
for authoring and portability, and the
[current Node changelog](https://github.com/Latias94/merman/blob/main/platforms/node/CHANGELOG.md)
for upgrade actions.

## Loaded icon packs

The native and Node-WASM packages accept the same
`createNodeEngine({ iconPacks: [{ json, registrationName? }] })` constructor argument. `json` is
an already loaded Iconify pack JSON string; the host owns acquisition. `registrationName`
optionally replaces the pack prefix. The registry is immutable and scoped to that engine;
per-request options preserve it, and `dispose()` releases it after queued work settles.
An omitted argument or `[]` preserves ordinary rendering.

Use the [native package example](https://github.com/Latias94/merman/blob/main/platforms/node/packages/node/README.md#loaded-icon-packs)
with the import changed to `@mermanjs/node-wasm`. Keep packs outside `bindingOptions`.
The `icon-registry` constructor catalog exposes the same fixed resource ceilings on both
transports. Construction errors preserve `MermanOperationError.iconRegistryDetails` and,
for resource failures, `resourceDetails`.
