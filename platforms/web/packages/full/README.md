# @mermanjs/web

The complete browser SDK for SVG rendering, analysis, ASCII output, and parser-backed editor
intelligence. Start here when evaluating Merman or when one browser realm needs more than one
workflow.

The current source uses Web transport API `12`, facts schema `3`, Theme v2, and Options JSON schema `4`. Keep the facade, generated glue, and WASM at the same exact version and source revision. Check the [host installation guide](https://github.com/Latias94/merman/blob/main/docs/release/PREBUILT_PRODUCTS.md#published-versions) for verified versions and channel availability.

Use `renderAsciiResult()` to read ASCII schema `4` output metadata. Update report readers for the nullable primary layout and separate emitted overflow. [ASCII report migration](https://github.com/Latias94/merman/blob/main/docs/migrations/render-reuse-host-integration.md#ascii-output-schema-3-to-4)

Published `0.10.0-alpha.2` retains Web transport API `9`. Version `0.10.0-alpha.1` retains Web transport API `8`, facts schema `2`, theme execution evidence schema `1`, and Options JSON schema `3`. Upgrade all package assets together and update JSON consumers for the [source appearance and render-evidence changes](https://github.com/Latias94/merman/blob/main/docs/release/UNRELEASED_CONTRACT_VERSIONS.md#source-appearance-and-render-evidence-follow-up).

The prebuilt package includes math, Cytoscape and ELK for SVG rendering. Its analysis, editor and ASCII
operations share one initialized runtime.

## Quick start

```sh
npm install --save-exact @mermanjs/web@0.11.0
```

```ts
import { initMerman, renderSvgToElement } from "@mermanjs/web";

await initMerman();

const target = document.querySelector("#diagram");
if (!target) throw new Error("missing #diagram mount point");

renderSvgToElement(target, `flowchart TD
  A[Start] --> B[Done]`);
```

Use `renderSvg()` when the host needs the serialized SVG string instead of a mounted element.

The runtime defaults to Unix epoch time and UTC for reproducible output. For a live Gantt today
marker, capture the browser clock explicitly and reuse the same options for related operations:

```ts
import { renderSvg } from "@mermanjs/web";

const instant = new Date();
const svg = renderSvg(source, {
  fixed_unix_millis: instant.getTime(),
  fixed_local_offset_minutes: -instant.getTimezoneOffset(),
});
```

This preserves time of day. The date-only `fixed_today` option instead selects local midnight;
only one of those clock selectors may be non-null. The offset above is a fixed snapshot rather
than a complete daylight-saving rule set. See the [shared options reference](../../../../docs/bindings/OPTIONS_JSON.md#fixed-time-options).
Use `prepareSource()` for repeated SVG, layout and geometry outputs from frozen parse facts.
Dispose the returned snapshot when finished; see the
[prepared-source guide](https://github.com/Latias94/merman/blob/main/platforms/web/README.md#reuse-a-parsed-source).

For retained DOM updates and explicit Kanban width adaptation, use
[`mountDiagram()`](https://github.com/Latias94/merman/blob/main/platforms/web/README.md#maintain-a-mounted-diagram).

## Themes

Use `themeCatalog()` to discover presets and appearances. Pass `{ theme: "paper-cut" }` to
`renderSvgResult()` to select a preset and read `metadata.theme_outcome`. Use
`exportThemePreset("paper-cut")` for an editable v2 document and `resolveTheme(document)` to
check its settings and contrast. See the [browser theme guide](https://github.com/Latias94/merman/blob/main/platforms/web/README.md#choose-a-preset-for-a-diagram).

## Runtime boundary

Load this package only in a browser main thread or Web Worker. It is not a Node.js or SSR transport;
use [`@mermanjs/node`](https://github.com/Latias94/merman/blob/main/platforms/node/packages/node/README.md)
for supported native Node.js hosts.

Initialize Merman once per realm and reuse it. A host that creates a Worker owns Worker termination.
Dispose retained editor and text-measurement sessions when they are no longer needed.

See the [browser package guide](https://github.com/Latias94/merman/blob/main/platforms/web/README.md)
for package selection, safe DOM mounting, custom WASM loading, resource limits, and source-checkout
development. See the [current changelog](https://github.com/Latias94/merman/blob/main/CHANGELOG.md)
for changes and upgrade actions.
