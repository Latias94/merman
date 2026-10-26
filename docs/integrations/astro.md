# Astro host integration

This guide describes unpublished source-branch APIs as of October 26, 2026.
Use browser, Node, and runtime artifacts built from the same branch. Published
`0.9.2` does not include these changes; an unchanged package version in a local
build does not make them released APIs.

This `release/0.9.x` guide uses Mermaid 12.1 and Binding Options JSON version **2**.
The `main` branch has its own guide and version **3** options.
Keep each branch's configuration and transport contracts together.

Use `@mermanjs/web-render` or `@mermanjs/web` inside the browser. Keep the Web
server-runtime guard: Astro frontmatter, SSR, and SSG use `@mermanjs/node` or the
explicit `@mermanjs/node-wasm` transport. Importing Web dynamically does not make
it a server renderer.

## Loaded icon packs

The host fetches or reads Iconify JSON before constructing a renderer:

```js
const iconPacks = [
  { json: loadedPackJson, registrationName: "site" },
];
```

`json` is the JSON **string**, not a parsed pack object. If the host already has
an object, serialize it with `JSON.stringify(pack)`. `registrationName` is
optional; omitting it uses the pack's prefix. The examples assume a pack with a
`server` icon in `public/icons/site.json`, registered as `site`.

The registry is immutable and scoped to the constructed session or engine.
Construct another instance to change its packs. Empty or omitted packs preserve
service-free behavior. Registry validation and resource-limit failures retain
their structured binding details; report those errors rather than silently
retrying without icons. Packs are constructor inputs, never Binding Options JSON.

## Browser adapter

Put this helper in `src/lib/merman-browser.js`. Call it from an Astro component's
bundled `<script>` or another browser entry, after the page's font stylesheets
have loaded.

```js
export async function createAstroRenderer(
  iconPacks = [],
  { browserTextMeasurement = false } = {},
) {
  if (typeof document === "undefined") {
    throw new Error("Use the Node renderer during SSR or SSG.");
  }
  const web = await import("@mermanjs/web-render");
  await web.initMerman();
  await document.fonts.ready;

  const session = web.createRenderSession({ iconPacks });
  const probes = browserTextMeasurement
    ? web.createBrowserTextMeasurementSession()
    : null;
  const fontFamily = '"Site Sans", sans-serif';

  return {
    render(target, source, diagramId, dark = false) {
      const options = {
        version: 2,
        site_config: {
          theme: dark ? "dark" : "default",
          fontFamily,
          fontSize: 16,
          themeVariables: { fontFamily, fontSize: "16px" },
        },
        svg: { diagram_id: diagramId, pipeline: "parity" },
      };
      const svg = probes
        ? session.renderSvgWithTextMeasurer(source, probes.measure, options)
        : session.renderSvg(source, options);

      const admission = web.assertNavigableSvgForDom(svg);
      const parsed = new DOMParser().parseFromString(svg, "image/svg+xml");
      if (parsed.querySelector("parsererror")) {
        throw new Error("Merman returned invalid SVG.");
      }
      const ownerDocument = target.ownerDocument;
      const root = ownerDocument.importNode(parsed.documentElement, true);
      web.prepareNavigableSvgForDomMount(admission, root, ownerDocument);
      target.replaceChildren(root);
    },
    dispose() {
      session.dispose();
      probes?.dispose();
    },
  };
}
```

Replace `Site Sans` with the application's CSS font family. Ensure the required
regular, bold, and italic faces have started loading before awaiting
`document.fonts.ready`; use `document.fonts.load()` when they are not yet used on
the page. Keep the measurement font and displayed label typography equivalent.
Setting both root and theme font values keeps the wrapping probe and displayed
font aligned without conflating their measurement responsibilities.

Browser font measurement is an explicit opt-in. Leaving
`browserTextMeasurement` false uses deterministic, font-independent approximate
metrics, even after fonts are ready. When using the host measurer, omit
`environment.text_measurement`; an explicit selector conflicts with that service.
Rejected Canvas font assignments are unhandled measurements, not successful
measurements using a previous font. Font availability and fallback remain host
environment concerns.

For a one-shot browser mount with an existing `<div id="diagram"></div>`:

```js
import { createAstroRenderer } from "../lib/merman-browser.js";

const response = await fetch("/icons/site.json");
if (!response.ok) throw new Error(`Icon pack request failed: ${response.status}`);
const iconPacks = [{ json: await response.text(), registrationName: "site" }];
const renderer = await createAstroRenderer(iconPacks, {
  browserTextMeasurement: true,
});
try {
  const target = document.querySelector("#diagram");
  if (!target) throw new Error("Missing diagram target.");
  renderer.render(
    target,
    'flowchart LR\nA@{ icon: "site:server", label: "Server" } --> B[Ready]',
    `merman-${crypto.randomUUID()}`,
    document.documentElement.dataset.theme === "dark",
  );
} finally {
  renderer.dispose();
}
```

Mounted SVG does not require a live session. For repeated renders, retain the
renderer, original source, and diagram ID until the host removes that view.
Pass new per-call options through the adapter when the host theme changes, then
dispose the renderer during teardown. Disposal is idempotent, rejects subsequent
session calls, and defers freeing an active session if invoked from a measurement
callback. It does not remove already mounted SVG.

On this maintained line, sessions expose SVG, layout, and edge geometry methods
and their host-measurer variants. This branch does not add SVG-result APIs.

## SSG and SSR adapter

Put this helper in `src/lib/merman-build.mjs`. It reads the same host-owned pack
and reuses one engine for a batch:

```js
import { readFile } from "node:fs/promises";
import { createNodeEngine } from "@mermanjs/node";

export async function renderBuildDiagrams(diagrams) {
  const json = await readFile(
    new URL("../../public/icons/site.json", import.meta.url),
    "utf8",
  );
  const fontFamily = '"Site Sans", sans-serif';
  const engine = await createNodeEngine({
    iconPacks: [{ json, registrationName: "site" }],
    bindingOptions: {
      version: 2,
      runtime_policy: "deterministic",
      site_config: {
        theme: "default",
        fontFamily,
        fontSize: 16,
        themeVariables: { fontFamily, fontSize: "16px" },
      },
    },
  });
  try {
    const output = [];
    for (const { source, diagramId } of diagrams) {
      output.push(await engine.renderSvg(source, {
        optionsJson: JSON.stringify({
          version: 2,
          svg: { diagram_id: diagramId, pipeline: "parity" },
        }),
      }));
    }
    return output;
  } finally {
    await engine.dispose();
  }
}
```

For the explicit Node-WASM transport, change the import to
`@mermanjs/node-wasm` and use that branch's matching Node-WASM artifacts and
loader setup. Both Node transports take the same `iconPacks` and `bindingOptions`.
Per-render Node options travel in `optionsJson`; this differs from the Web
method's direct options object.

An Astro component can use the helper in frontmatter for trusted build inputs:

```astro
---
import { renderBuildDiagrams } from "../lib/merman-build.mjs";
const [svg] = await renderBuildDiagrams([{
  source: 'flowchart LR\nA@{ icon: "site:server", label: "Server" } --> B[Ready]',
  diagramId: "guide-server-overview",
}]);
---
<figure set:html={svg} />
```

Allocate diagram IDs across the whole generated page, including repeated
components. Node uses deterministic measurement; it has no `document.fonts` or
browser measurement callback. A configured family does not load a font or
guarantee exact browser bounds. Use a browser render with an explicitly selected
host measurer when the page needs measurements from its loaded fonts.

## Host responsibilities

Astro or the application owns pack acquisition, fetch failures, font loading,
theme observation, redraw scheduling, and page lifecycle. Adjust pack URLs for
the site's base path. For client navigation, initialize each new view and dispose
its renderer when that view is removed; a one-time bundled script is not a
navigation lifecycle manager.

Give every mounted diagram a unique `svg.diagram_id`; keep that ID stable for
redraws of the same mount. Keep original sources separately from generated DOM.
Choose how to report render failures or retain the last good output. Package
functions such as `renderSvg()` do not inherit a session's packs. This adapter
adds no Astro plugin, automatic pack loader, or global theme observer.

See the [Web package guide](../../platforms/web/README.md#mount-svg-safely) for
DOM admission policy and the [binding options guide](../bindings/OPTIONS_JSON.md)
for the current branch's configuration contract.
