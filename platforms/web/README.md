# Merman Browser Packages

This workspace builds the lockstep, browser-only Merman package group. Each admitted package contains one matching WASM artifact, its TypeScript wrapper, provenance, and legal material.

> These commands build and install a source checkout. Published npm packages are versioned independently of this documentation.

Do not use these packages for Node.js or SSR. They require a browser main-thread or Web Worker realm when loading the WASM module. A future Node transport, if admitted, will be a separate package rather than a browser-WASM fallback.

## Choose A Package

| Package | Use it for | Status |
| --- | --- | --- |
| `@mermanjs/web` | Complete browser rendering, analysis, ASCII output, and editor APIs. | Required complete release surface. |
| `@mermanjs/web-analysis` | Detection, validation, facts, and semantic analysis without SVG, ASCII, or editor sessions. | Admitted slim release surface. |
| `@mermanjs/web-editor` | Parser-backed editor sessions in a dedicated browser Worker. | Admitted slim release surface. |
| `@mermanjs/web-ascii` | Supported ASCII diagram output. | Admitted slim release surface. |
| `@mermanjs/web-render` | Complete SVG-only workflow with both layouts and math. | Admitted complete rendering surface. |

Public packages use one version and one release contract. Workflow-specific slim packages are published only when their independently measured installed size is at least 15% below the complete package. The complete SVG-only renderer is admitted for its distinct capability contract, with its smaller artifact recorded separately. The Playground uses `@mermanjs/web` in both the main renderer and editor Worker because its same-revision whole-site R16 measurement found that adding the editor artifact did not lower cold transfer or preserve peak memory. That application-specific result does not change the supported `@mermanjs/web-editor` package surface.

## Browser Quick Start

Build the local package group and install the complete browser package:

```sh
npm ci --prefix /path/to/merman/platforms/web
npm run build --prefix /path/to/merman/platforms/web
npm install /path/to/merman/platforms/web/packages/full
```

```ts
import { initMerman, renderSvg } from "@mermanjs/web";

await initMerman();

const svg = renderSvg(`flowchart TD
  A[Start] --> B[Done]`);
```

For a custom cache, service worker, or fetch policy, retain the wrapper's loader and pass the actual response or bytes through wasm-bindgen's `module_or_path` contract:

```ts
import {
  initMerman,
  loadMermanWasmModule,
  MERMAN_WASM_URL,
} from "@mermanjs/web";

const wasm = await fetch(MERMAN_WASM_URL, { cache: "reload" });
await initMerman({ loader: loadMermanWasmModule, wasm });
```

## Runtime Lifecycle

Initialize Merman once per browser realm and reuse it; repeated `initMerman()` calls share the cached initialization and module. There is no separate WASM unload API, so a main-thread runtime lives for the page realm. The package does not create or own a Worker: a host that loads Merman in a dedicated Worker must terminate that Worker after initialization failure, replacement, or application teardown. Synchronous WASM calls cannot be interrupted from the same realm, so Worker termination is the hard cancellation boundary. Dispose every `BrowserTextMeasurementSession` created by `createBrowserTextMeasurementSession()` as soon as it is no longer needed.

Call `runtimeCatalog()` after initialization when an integration needs to inspect the compiled capabilities, operations, and resource profiles. Validate the flat catalog's local relations and tolerate newly introduced stable IDs; do not infer availability from package names or Cargo feature names. Resource limits are described in [`docs/bindings/OPTIONS_JSON.md`](https://github.com/Latias94/merman/blob/main/docs/bindings/OPTIONS_JSON.md); browser preview normally uses the `interactive` profile, while a public submission service needs host-level timeout, memory, concurrency, and process-isolation controls in addition to `constrained` limits.

## Maintainer Build

```sh
npm install --prefix platforms/web
npm run build --prefix platforms/web
npm run test:wasm-inputs --prefix platforms/web
npm run smoke --prefix platforms/web
```

`web-surface-descriptor.json` maps package names to exact artifact profiles. Cargo feature selection lives in the artifact-profile authority; this workspace derives its WASM build and package assembly from that authority instead of maintaining a second feature matrix. Use `npm run verify:wasm-inputs --prefix platforms/web` to validate all generated WASM inputs and `npm run verify:packages --prefix platforms/web` to enforce the one-WASM, legal-material, provenance, export, and size-admission invariants.

Package evidence is intentionally layered. The artifact profile and runtime catalog prove the compiled Rust/WASM capabilities. The recorded static-module closure proves only the JavaScript and declaration files reachable from that package entry; explicit source tests independently require and forbid the workflow modules for each surface. Shared catalogs and types describe the lockstep package group and do not imply that a workflow implementation is compiled into a slim WASM artifact.

The private workspace itself is never published. Release automation packs and verifies every admitted package as a version-locked group, then performs staged dist-tag reconciliation without rebuilding in the privileged publish job.
