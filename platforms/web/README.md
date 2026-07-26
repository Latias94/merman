# Merman Browser Packages

This workspace builds the lockstep, browser-only Merman package group. Each admitted package
contains one matching WASM artifact, its TypeScript wrapper, provenance, and legal material.

Do not use these packages for Node.js or SSR. They require a browser main-thread or Web Worker
realm when loading the WASM module. A future Node transport, if admitted, will be a separate
package rather than a browser-WASM fallback.

## Choose A Package

| Package | Use it for | Status |
| --- | --- | --- |
| `@mermanjs/web` | Complete browser rendering, analysis, ASCII output, and editor APIs. | Required complete release surface. |
| `@mermanjs/web-analysis` | Detection, validation, facts, and semantic analysis without SVG, ASCII, or editor sessions. | Admitted slim release surface. |
| `@mermanjs/web-editor` | Parser-backed editor sessions in a dedicated browser Worker. | Admitted slim release surface. |
| `@mermanjs/web-ascii` | Supported ASCII diagram output. | Admitted slim release surface. |
| `@mermanjs/web-render` | Complete SVG-only workflow with both layouts and math. | Admitted complete rendering surface. |

When released, public packages use one version and one release contract. Workflow-specific slim
packages are published only when their independently measured installed size is at least 15% below
the complete package. The complete SVG-only renderer is admitted for its distinct capability
contract, with its smaller artifact recorded separately. The Playground deliberately uses
`@mermanjs/web` for both editor and renderer until its two-realm measurement proves that a split
improves the real user path.

## Browser Quick Start

```ts
import { initMerman, renderSvg } from "@mermanjs/web";

await initMerman();

const svg = renderSvg("flowchart TD\\n  A[Start] --> B[Done]");
```

For a custom cache, service worker, or fetch policy, retain the wrapper's loader and pass the
actual response or bytes through wasm-bindgen's `module_or_path` contract:

```ts
import {
  initMerman,
  loadMermanWasmModule,
  MERMAN_WASM_URL,
} from "@mermanjs/web";

const wasm = await fetch(MERMAN_WASM_URL, { cache: "reload" });
await initMerman({ loader: loadMermanWasmModule, wasm });
```

Call `runtimeCatalog()` after initialization when an integration needs to inspect the compiled
capabilities, operations, and resource profiles. Validate the flat catalog's local relations and
tolerate newly introduced stable IDs; do not infer availability from package names or Cargo feature
names. Resource limits are described in
[`docs/bindings/OPTIONS_JSON.md`](../../docs/bindings/OPTIONS_JSON.md); browser preview normally
uses the `interactive` profile, while a public submission service needs host-level timeout,
memory, concurrency, and process-isolation controls in addition to `constrained` limits.

## Maintainer Build

```sh
npm install --prefix platforms/web
npm run build --prefix platforms/web
npm run test:wasm-inputs --prefix platforms/web
npm run smoke --prefix platforms/web
```

`web-surface-descriptor.json` maps package names to exact artifact profiles. Cargo feature
selection lives in the artifact-profile authority; this workspace derives its WASM build and
package assembly from that authority instead of maintaining a second feature matrix. Use
`npm run verify:wasm-inputs --prefix platforms/web` to validate all generated WASM inputs and
`npm run verify:packages --prefix platforms/web` to enforce the one-WASM, legal-material,
provenance, export, and size-admission invariants.

The private workspace itself is never published. Release automation packs and verifies every
admitted package as a version-locked group, then performs staged dist-tag reconciliation without
rebuilding in the privileged publish job.
