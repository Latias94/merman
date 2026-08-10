# @mermanjs/web-render

This is the browser-only complete SVG rendering package. Its artifact contains `svg`, both supported layout engines, and RaTeX math, but no analysis, editor-session, or ASCII-rendering workflow implementation. Shared package-group catalogs and types remain available for integration code.

## Install

This package has shipped since Merman `0.8.0-alpha.5`. Install the current alpha:

```sh
npm install @mermanjs/web-render@alpha
```

For local source development, build the package group and install this package from the checkout:

```sh
npm ci --prefix /path/to/merman/platforms/web
npm run build --prefix /path/to/merman/platforms/web
npm install /path/to/merman/platforms/web/packages/render
```

Install it when an application needs complete Mermaid SVG rendering without the analysis, editor, or ASCII workflows from `@mermanjs/web`:

```ts
import { initMerman, renderSvg } from "@mermanjs/web-render";

await initMerman();
const svg = renderSvg(`flowchart TD
  A --> B`);
```

It requires a browser main-thread or Web Worker realm for WASM loading and is not a Node.js or SSR transport. Initialize it once per realm and reuse it; when a host creates a dedicated Worker, that host owns Worker termination after failure, replacement, or teardown. A server-side application must use a separately admitted native transport.

Browser bindings use the `interactive` resource profile by default. For untrusted public input, pass `{ resources: { profile: "constrained" } }` and also enforce host timeout, memory, concurrency, and process-isolation limits.

The package participates in the same lockstep release, provenance, legal-material, declaration, and lifecycle checks as the rest of the public browser package group. Its size is measured against `@mermanjs/web`, but its product boundary is complete SVG capability rather than a 15% slim-workflow threshold.

See the [browser package guide](https://github.com/Latias94/merman/blob/main/platforms/web/README.md#runtime-lifecycle) for custom WASM loading, runtime lifecycle, and resource-policy guidance.
