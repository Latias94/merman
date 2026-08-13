# @mermanjs/web-render

Render Mermaid diagrams to SVG in a browser with Cytoscape and ELK layouts plus math support,
without shipping the analysis, editor, or ASCII workflows from the complete SDK.

This package is published on npm's `alpha` dist-tag. Pin an exact version when reproducible installs
matter.

## Quick start

```sh
npm install @mermanjs/web-render@alpha
```

```ts
import { initMerman, renderSvgToElement } from "@mermanjs/web-render";

await initMerman();

const target = document.querySelector("#diagram");
if (!target) throw new Error("missing #diagram mount point");

renderSvgToElement(target, `flowchart TD
  A --> B`);
```

Use `renderSvg()` when the host needs the serialized SVG string instead of a mounted element.

## Runtime and resources

Load this package only in a browser main thread or Web Worker. Initialize it once per realm and
reuse it; a host that creates a Worker owns Worker termination after failure, replacement, or
teardown.

Browser bindings use the `interactive` resource profile by default. For untrusted public input,
pass `{ resources: { profile: "constrained" } }` and enforce host timeout, memory, concurrency, and
process-isolation limits as well.

See the [browser package guide](https://github.com/Latias94/merman/blob/main/platforms/web/README.md)
for safe DOM mounting, custom WASM loading, resource policy, package selection, and source-checkout
development.
