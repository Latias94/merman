# Merman for browsers

Use Merman's Rust parser, renderers, analysis, and editor APIs in a browser through WebAssembly.

Start with `@mermanjs/web` when evaluating Merman or when one browser realm needs several
workflows. Choose a focused package when the application has one isolated workflow and the smaller
boundary is useful. These packages require a browser main thread or Web Worker; for Node.js and
static-site builds, use [`@mermanjs/node`](../node/packages/node/README.md).

## Choose a package

| Need | Package | Includes |
| --- | --- | --- |
| A complete SDK or a starting point | [`@mermanjs/web`](packages/full/README.md) | SVG, analysis, ASCII, and editor APIs |
| SVG rendering only | [`@mermanjs/web-render`](packages/render/README.md) | Cytoscape and ELK layouts plus math |
| Validation and semantic facts | [`@mermanjs/web-analysis`](packages/analysis/README.md) | Detection, validation, analysis, and lint facts |
| Editor intelligence in a Worker | [`@mermanjs/web-editor`](packages/editor/README.md) | Analysis and parser-backed editor sessions |
| ASCII or Unicode output | [`@mermanjs/web-ascii`](packages/ascii/README.md) | Supported terminal-oriented diagram output |

All public browser packages use one lockstep version. The current prerelease is published on npm's
`alpha` dist-tag; pin an exact version when reproducible installs matter.

Prefer one Merman package per browser realm. Combining the complete package with a focused package
creates another WASM runtime unless that duplication has been measured and is intentional.

## Quick start

```sh
npm install @mermanjs/web@alpha
```

```ts
import { initMerman, renderSvgToElement } from "@mermanjs/web";

await initMerman();

const target = document.querySelector("#diagram");
if (!target) throw new Error("missing #diagram mount point");

renderSvgToElement(target, `flowchart TD
  A[Start] --> B[Done]`);
```

Initialize Merman once per browser realm and reuse it. Call `renderSvg()` instead when the host
needs the serialized SVG string rather than a mounted element.

## Mount SVG safely

`renderSvg()` returns Mermaid-parity source and deliberately makes no browser DOM-safety claim.
Use `renderSvgToElement(target, source)` for the standard navigable authoring surface: it validates
the serialized SVG, parses it in the target's document, rechecks fragment resolution at that exact
mount boundary, and hardens external anchors with a new browsing context plus
`noopener noreferrer`.

Hosts that own parsing themselves must choose one explicit capability:

- `assertSelfContainedSvgForDom()` rejects navigation as well as external rendering resources.
- `assertNavigableSvgForDom()` admits Mermaid-compatible anchor navigation while continuing to
  reject external images, styles, filters, scripts, tracking, and automatic resource loads.

The validators return opaque, package-instance-bound capabilities. Keep the returned value until
the real owner document is known. For navigable SVG, call `prepareNavigableSvgForDomMount()` after
importing the parsed root into that document; for a closed preview, call
`prepareSelfContainedSvgForDomMount()`. Both helpers revalidate the actual parsed root immediately
before insertion. Object literals, structured clones, source/tree substitution, and a
self-contained admission cannot stand in for a navigable admission. CSP, iframe/origin isolation,
and whether navigation remains enabled during stale UI states are host responsibilities.

## Runtime lifecycle and resource limits

Repeated `initMerman()` calls share the cached initialization and module. There is no separate WASM
unload API, so a main-thread runtime lives for the page realm. The package does not create or own a
Worker: a host that loads Merman in a dedicated Worker must terminate that Worker after
initialization failure, replacement, or application teardown. Synchronous WASM calls cannot be
interrupted from the same realm, so Worker termination is the hard cancellation boundary. Dispose
every `BrowserTextMeasurementSession` created by `createBrowserTextMeasurementSession()` when it is
no longer needed.

Call `runtimeCatalog()` after initialization when an integration needs to inspect compiled
capabilities, operations, and resource profiles. Browser previews normally use the `interactive`
profile. Public submission services also need host-level timeout, memory, concurrency, and process
isolation around the `constrained` profile. See the [binding options guide](../../docs/bindings/OPTIONS_JSON.md)
for the full resource contract.

## Custom WASM loading

Keep the package loader and pass a response, URL, request, module, or bytes through wasm-bindgen's
`module_or_path` contract when the host owns cache, service-worker, or fetch policy:

```ts
import {
  initMerman,
  loadMermanWasmModule,
  MERMAN_WASM_URL,
} from "@mermanjs/web";

const wasm = await fetch(MERMAN_WASM_URL, { cache: "reload" });
if (!wasm.ok) {
  throw new Error(`Failed to load Merman WASM: ${wasm.status}`);
}
await initMerman({ loader: loadMermanWasmModule, wasm });
```

The generated raw wasm-bindgen shim has no implicit module-path fallback. The public loader supplies
`MERMAN_WASM_URL` only when initialization receives no input, which keeps URL and cache policy at
the package/host boundary and prevents internal bundles from acquiring a hidden second WASM asset.

## Develop from source

```sh
npm ci --prefix platforms/web
npm run build --prefix platforms/web
npm test --prefix platforms/web
npm run smoke --prefix platforms/web
```

`web-surface-descriptor.json` maps package names to artifact profiles. The workspace derives Cargo
features, generated entries, and package assembly from those authorities rather than maintaining a
second feature matrix. Use `npm run verify:wasm-inputs --prefix platforms/web` and
`npm run verify:packages --prefix platforms/web` for the package-specific contracts.

The private workspace itself is never published. Release automation packs and verifies every
admitted package as a version-locked group before reconciling dist-tags. See the [package surface
guide](../../docs/release/PACKAGE_SURFACES.md) for artifact, provenance, legal-material, and
size-admission details.
