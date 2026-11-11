# @mermanjs/web-render

Render Mermaid diagrams to SVG in a browser with Cytoscape and ELK layouts,
without shipping the analysis, editor, or ASCII workflows from the complete SDK.

Version `0.10.0` uses Web transport API `10`, theme execution evidence schema `2`, and Options JSON schema `3`. Keep the facade, generated glue, and WASM at the same exact version and source revision. Check the [host installation guide](https://github.com/Latias94/merman/blob/main/docs/release/PREBUILT_PRODUCTS.md#published-versions) for verified versions and channel availability.

Published `0.10.0-alpha.2` retains Web transport API `9`. Version `0.10.0-alpha.1` retains Web transport API `8`, theme execution evidence schema `1`, and Options JSON schema `3`. Upgrade all package assets together and update evidence consumers for the [render-evidence changes](https://github.com/Latias94/merman/blob/main/docs/release/UNRELEASED_CONTRACT_VERSIONS.md#source-appearance-and-render-evidence-follow-up).

This prebuilt renderer includes math, Cytoscape and ELK. It stays focused by omitting the complete SDK's
analysis, editor and ASCII operations.

## Quick start

```sh
npm install --save-exact @mermanjs/web-render@0.10.0
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
Use `prepareSource()` for repeated SVG, layout and geometry outputs from frozen parse facts.
Dispose the returned snapshot when finished; see the
[prepared-source guide](https://github.com/Latias94/merman/blob/main/platforms/web/README.md#reuse-a-parsed-source).

For retained DOM updates and explicit Kanban width adaptation, use
[`mountDiagram()`](https://github.com/Latias94/merman/blob/main/platforms/web/README.md#maintain-a-mounted-diagram).

## Themes

Use `themeCatalog()` to discover preset designs and appearances, `renderSvgResult()` with
`{ theme: { preset: "paper-cut" } }` to select one, and `exportThemePreset("paper-cut")` to obtain
its editable JSON recipe. See the [theme workflow](https://github.com/Latias94/merman/blob/main/platforms/web/README.md#choose-a-preset-for-a-diagram)
and [customization guide](https://github.com/Latias94/merman/blob/main/docs/rendering/custom-diagram-themes.md)
for shared tokens, paired appearances, JSON import/export, and actual render evidence.

## Runtime and resources

Load this package only in a browser main thread or Web Worker. Initialize it once per realm and
reuse it; a host that creates a Worker owns Worker termination after failure, replacement, or
teardown.

Browser bindings use the `interactive` resource profile by default. For untrusted public input,
pass `{ resources: { profile: "constrained" } }` and enforce host timeout, memory, concurrency, and
process-isolation limits as well.

See the [browser package guide](https://github.com/Latias94/merman/blob/main/platforms/web/README.md)
for safe DOM mounting, custom WASM loading, resource policy, package selection, and source-checkout
development. See the [current changelog](https://github.com/Latias94/merman/blob/main/CHANGELOG.md)
for changes and upgrade actions.
