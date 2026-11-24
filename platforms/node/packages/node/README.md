# @mermanjs/node

Native Mermaid-compatible SVG rendering for Node.js 22+ and static-site build pipelines, without
Chromium, a browser-WASM fallback, or a postinstall binary download.

The package remains experimental. The current source targets Mermaid `12.3.0` and uses Node transport API `3` and Options JSON schema `4`. Keep the loader and native platform package at the same exact version and source revision. [Verified versions and installation](https://github.com/Latias94/merman/blob/main/docs/release/PREBUILT_PRODUCTS.md#published-versions) · [0.11.0 migration](https://github.com/Latias94/merman/blob/main/docs/migrations/render-reuse-host-integration.md#refresh-wrappers-and-runtime-artifacts)

## Quick start

Install the published `0.11.0` loader, which selects a matching native package using transport API `2`.

```sh
npm install --save-exact @mermanjs/node@0.11.0
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

Create an engine once, reuse it for related work, and dispose it during teardown.

## Compiled themes

Theme v2 is available in builds from the current source tree. Discover preset IDs and appearances
with `engine.themeCatalog()`. The schema 2 catalog includes each preset's default appearance and six
swatches, along with Mermaid classic theme IDs and native capabilities by diagram family.

The `theme` field in Options JSON schema `4` accepts these selections.

| Selection | Options JSON value |
| --- | --- |
| Use a preset's default appearance | `"studio"` |
| Select a preset or document with an appearance or precedence | `{ "preset": "studio", "appearance": "dark" }` |
| Apply an inline document | `{ "schema_version": 2, "extends": "studio", "seed": { "accent": "#0052CC" } }` |
| Clear the constructor selection for this request | `"none"` |

A selector takes exactly one of `preset` or `document`. Precedence defaults to `"host"`. Use
`"source"` to follow a Mermaid theme explicitly declared by the diagram. If the request omits
`theme`, it inherits the constructor selection. Omit `appearance` to use the theme's default.
Set top-level `background` to a hex color or `"transparent"` to override the SVG canvas base.

```js
const preset = await engine.exportThemePreset("studio");
const document = { ...preset, seed: { ...preset.seed, accent: "#0052CC" } };
const resolution = await engine.resolveTheme(document, { appearance: "dark" });
if (!resolution.ok) throw new Error(resolution.message);

const result = await engine.executeOperation({
  operationId: "svg",
  source: "flowchart LR\nA[Review] --> B[Publish]",
  optionsJson: JSON.stringify({ theme: { document, appearance: "dark" } }),
});
const { theme_outcome } = JSON.parse(result.metadata_json);
```

Save the authored document with `JSON.stringify(document, null, 2)`. To check JSON loaded from disk,
pass the text to `resolveTheme()` for document-relative `line`, `column`, and `path` diagnostics.
Set `auto_fix_contrast: true` to preview adjusted foreground colors in the resolved result. Edit
the document to carry those adjustments into later renders. [Theme document schema](https://github.com/Latias94/merman/blob/main/schemas/theme-v2.schema.json)

For SVG, `theme_outcome` reports `mode` (`merman_native` or `mermaid_parity`), optional `preset_id`
and `appearance`, and `notices`. Both async theme helpers use the engine queue and accept `signal`
and `timeoutMs`. Synchronous callers can use `executeOperationSync()` with `export-theme-preset`
(preset ID as `source`) or `resolve-theme` (`{ document, appearance?, auto_fix_contrast? }` as JSON `source`).

## SVG output pipelines

`renderSvg()` and `renderSvgSync()` use the `parity` pipeline by default. This preserves the
Mermaid-compatible SVG structure for trusted input and parity-sensitive consumers. `parity` is
not a browser DOM-admission or sanitization check.

For a resvg/usvg consumer that cannot display HTML labels, select `resvg-safe` explicitly:

```js
const options = {
  optionsJson: JSON.stringify({ svg: { pipeline: "resvg-safe" } }),
};
const svg = await engine.renderSvg(source, options);
```

The supported pipeline values are `parity`, `static`, `readable`, and `resvg-safe`. Use `static`
for fixed illustrations to omit unused generated palette rules. `readable` is an advanced
text overlay for consumers that ignore HTML labels; browsers can display both copies and show
overlapping text. Use `parity` for browser display. `resvg-safe` also removes unsupported styles,
so it is not a lossless replacement for `readable`. HTML embedding still requires a host-owned
admission policy; selecting a pipeline alone does not establish browser DOM safety.

The `resvg-safe` pipeline removes browser-only content such as active elements and `foreignObject`
labels. It does not
provide browser `Document` or owner-document admission APIs; use `@mermanjs/web*` for browser DOM
mounting.

## Supported hosts

- macOS arm64 and x64
- Linux arm64 and x64 with glibc or musl
- Windows x64 with MSVC
- Node.js `>=22`

The root loader selects one matching exact-version `@mermanjs/node-<platform>` optional dependency.
Do not install platform packages directly. The loader contains no native or WASM binary and never
downloads one during installation or runtime.

If the native addon is installed but the host dynamic loader rejects it (for example because the
glibc baseline is too new), construction throws `MermanNativeLoadError` with the target package
name and an explicit suggestion to use `@mermanjs/node-wasm`. It does not silently switch
transports.

## Capability boundary

The package provides deterministic static SVG with math, Cytoscape and ELK layouts, metadata,
layout plans, runtime-catalog inspection, and generic admitted operations. GNU Linux targets retain
glibc 2.31 as their compatibility floor. Binary export, analysis, ASCII, text-measurement callbacks,
browser fallback, and runtime downloads are outside this package surface. Requests for unavailable
optional capabilities return Merman's typed missing-capability error.

Render formulas with the ordinary SVG methods:

```js
const formula = await engine.renderSvg('flowchart LR\nA["$$x^2$$"]');
```

The runtime catalog advertises `math` for both the native and explicit Node-WASM products.
The default `interactive` resource profile admits formulas; an explicitly `constrained` profile
rejects them even when math is compiled. For untrusted input, enforce host execution, memory,
and concurrency limits alongside the selected resource profile.

## Lifecycle and concurrency

The Promise-based API uses a bounded queue. An `AbortSignal` cancels queued work immediately and
requests cooperative cancellation after Rust work starts; started work stops at the next renderer
or transport-admission checkpoint and reports Merman's typed cancellation details. Per-operation
`timeoutMs` values use a monotonic Rust deadline and must be integers from 0 through 4,294,967,295.
`dispose()` waits for active work to settle and rejects new work once teardown begins.

Use `renderSvgSync()` only for an explicitly synchronous static-site build path. It refuses to run
while asynchronous operations are active or queued.

## Loaded icon packs

Load Iconify pack JSON in the host and pass its text as a constructor service:

```js
import { readFile } from "node:fs/promises";
import { createNodeEngine } from "@mermanjs/node";

const json = await readFile("./icons.json", "utf8");
const engine = await createNodeEngine({
  iconPacks: [{ json, registrationName: "app" }],
});
try {
  const svg = await engine.renderSvg('flowchart TD\nA@{ icon: "app:rocket", label: "Launch" }');
  console.log(svg);
} finally {
  await engine.dispose();
}
```

The example assumes the pack contains `rocket`. `json` must be the loaded JSON string.
Omit `registrationName` to use the pack's prefix. A registered icon takes precedence over a
matching built-in icon in that engine. Packs are admitted once into immutable engine-local
state; changing the original array or rendering through another engine does not change it.
Per-request options preserve the installed registry.

`iconPacks` belongs to `CreateNodeEngineOptions`, separately from `bindingOptions`.
An omitted value or `[]` retains ordinary rendering. The constructor performs no network or
filesystem access. The fixed pack/content limits advertised by the `icon-registry` constructor
contract apply across native and Node-WASM. Content failures retain
`MermanOperationError.iconRegistryDetails`, and resource failures also retain
`resourceDetails`. A failed constructor publishes no engine. Dispose engines when their build
or request scope ends.

See the [Node.js package guide](https://github.com/Latias94/merman/blob/main/platforms/node/README.md)
for source-checkout development, package verification, and release design.
