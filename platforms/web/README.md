# @mermanjs/web

Browser integration for merman. This package wraps the `merman-wasm` wasm-bindgen output with a
small TypeScript API.

Use the live build at [Merman Playground](https://frankorz.com/merman/).

## Build

```sh
npm install --prefix platforms/web
npm run build --prefix platforms/web
npm run smoke --prefix platforms/web
```

The wrapper smoke asserts WASM ABI 2. The text-measurement test covers all 19 operation names,
including raw bbox height, and probes state isolation between ordinary and middle-baseline
createText bbox-y requests.

`npm run build` produces the default `browser-full` artifact used for npm publication. The surface
includes rendering, parsing, layout, ASCII, validation, diagnostics analysis, and the current
editor-language APIs. Source and CI builds can choose a browser WASM preset when a smaller local
artifact is useful:

The WASM build uses the workspace `wasm-size` Cargo profile through `wasm-pack --profile
wasm-size`. Use `wasm-pack` 0.15.0 or newer for local builds.
`web-surface-descriptor.json` is the machine-readable source for preset features/capabilities and
public subpath mappings; build, generated-surface, release, and size gates consume that descriptor.

| Preset | Command | Capability |
| --- | --- | --- |
| `browser-core` | `npm run build:wasm:core --prefix platforms/web` | Browser wasm-bindgen transport plus metadata, analysis, facts, and validation. Render, parse, layout, ASCII, and editor-language calls are unavailable. |
| `browser-render` | `npm run build:wasm:render --prefix platforms/web` | SVG, semantic JSON, layout JSON, metadata, analysis, facts, and validation over the minimal core profile. Editor-language calls are unavailable. |
| `browser-render-only` | `npm run build:wasm:render-only --prefix platforms/web` | SVG, semantic JSON, layout JSON, and metadata without diagnostics analysis, validation, lint catalog, ASCII, or editor-language dependencies. |
| `browser-ascii` | `npm run build:wasm:ascii --prefix platforms/web` | ASCII/Unicode rendering and metadata without diagnostics analysis or editor-language dependencies. |
| `browser-editor` | `npm run build:wasm:editor --prefix platforms/web` | Full 35-family catalog, analysis, and parser-backed editor-language APIs without SVG rendering, ASCII, host capabilities, or ELK. |
| `browser-full` | `npm run build:wasm:full --prefix platforms/web` | Default browser artifact: full core profile, SVG/layout/parse/analysis/validate, ASCII, editor-language APIs, host browser capabilities, and ELK layout. Includes EPL-backed ELK code. |
| `browser-full-no-elk` | `node platforms/web/scripts/build-wasm.mjs --preset browser-full-no-elk` | Evidence preset for the full browser surface without ELK. Keeps editor-language enabled. Not the npm default. |
| `browser-ratex-math` | `npm run build:wasm:ratex-math --prefix platforms/web` | Full browser artifact plus the RaTeX math renderer and ELK layout. Keeps editor-language enabled. |

Run `npm run build:ts --prefix platforms/web` after a preset build when producing a complete local
package. The TypeScript build first runs `npm run check:contracts --prefix platforms/web`, which
checks the wasm-bindgen declarations against the public wrapper, `MermanWasmModule`, and the
capability-specific subpath runtime bindings.

Each build writes `pkg/merman_wasm_preset.json` and `pkg/merman_wasm_inputs.json`. The input
manifest hashes the normal/build Cargo dependency closure for `merman-wasm`, workspace manifests
and lockfile, the pinned toolchain, generated ABI inputs, the selected preset and features, the
owned WASM build module directory, and the emitted wasm-bindgen artifacts. Cargo closure discovery
uses an isolated workspace probe so features enabled by unrelated workspace members cannot leak
into slim surfaces. Compile-time renderer assets live under their owning crate `src` tree. The
manifest deliberately ignores documentation, runtime-only audit assets, and dev-only dependencies.
Freshness is content-based; timestamps and file existence are not evidence that a committed WASM
binary matches the current Rust source.

Builds hold a per-output cross-process lock and write into a staging directory. Child surfaces are
published with one directory rename. The root `pkg` output must preserve child directories, so it is
a multi-file, manifest-last transaction rather than an OS-atomic directory replacement: the
committed manifest is absent throughout the replacement and a failure restores the previous output.
A verifier therefore sees either the previous complete artifact, a fail-closed transition, or the
new complete artifact. Interrupted transactions are recovered on the next build. The package
descriptor owns every allowed `pkg/` surface; undeclared directories and files fail prepack instead
of entering the tarball.

Run `npm run verify:wasm-inputs --prefix platforms/web` to check the default artifact or add
`-- --all-surfaces` to check every published surface. Consumers that load a subset can use a
descriptor-backed selection such as `-- --surfaces root,editor`. `smoke` and `prepack` run the
relevant check automatically and fail with one exact repository-root rebuild command when an input,
feature profile, manifest, or artifact differs. `npm run prepack` still expects `browser-full` unless
`MERMAN_WEB_ALLOW_NON_DEFAULT_PRESET=1` is set for an intentional local slim package, and it also
runs the wrapper/subpath contract check.

Call `bindingCapabilities()` after `initMerman()` when you need to branch on optional surfaces.
Call `runtimeContract()` when you also need the loaded ABI/package/options schema, registry facts,
or descriptor-driven resource profile values. Render-enabled artifacts include the complete
resource catalog; analysis/editor and ASCII artifacts include only the input limits their compiled
operations enforce. Only an artifact without render, analysis, or ASCII operations returns
`resources: null`. Do not duplicate profile values in application code.
Slim subpaths do not export wrappers for capabilities they intentionally omit. For example,
`@mermanjs/web/core` has no `renderSvg()`, `renderAscii()`, or editor-language exports, and
`@mermanjs/web/render` has no ASCII or editor-language exports.
`@mermanjs/web/render-only` has no analysis, validation, lint catalog, ASCII, or editor-language
exports.
`@mermanjs/web/ascii` has no analysis, validation, lint catalog, render, parse, layout, or
editor-language exports.
`@mermanjs/web/editor` has no render, parse/layout JSON, ASCII, browser text-measurement, host, or
ELK exports. It includes the complete 35-family language catalog, just like every other artifact.
`bindingCapabilities().analysis` is the supported runtime contract for whether the loaded artifact
exposes `analyze()`, `analysisFacts()`, `detectDiagramFacts()`, document analysis, validation, and
`lintRuleCatalog()`.
`bindingCapabilities().editor_language` is the supported runtime contract for whether the loaded
artifact exposes `createEditorSession()`, `editorDiagramDetection()`, `editorDiagnostics()`,
`editorCodeActions()`, `editorCompletions()`,
`editorHover()`, `editorDocumentSymbols()`, `editorWorkspaceSymbols()`, `editorDefinition()`,
`editorReferences()`, `editorPrepareRename()`, `editorRename()`,
`editorSemanticTokenDescriptor()`, and `editorSemanticTokens()`.

## Published entry points

The package publishes one default full artifact plus opt-in subpath entry points:

| Entry point | WASM preset | Intended use |
| --- | --- | --- |
| `@mermanjs/web` | `browser-full` | Default playground/editor package with render, layout, parse, ASCII, analysis, validation, editor APIs, and ELK. |
| `@mermanjs/web/core` | `browser-core` | Smallest browser artifact for metadata, analysis, facts, and validation. Render, layout, parse JSON, ASCII, and editor API wrappers are not exported. |
| `@mermanjs/web/render` | `browser-render` | SVG/layout/parse plus metadata, analysis, facts, and validation over the minimal core registry. ASCII and editor API wrappers are not exported. |
| `@mermanjs/web/render-only` | `browser-render-only` | SVG/layout/parse plus metadata. Analysis, validation, lint catalog, ASCII, and editor API wrappers are not exported. |
| `@mermanjs/web/ascii` | `browser-ascii` | ASCII/Unicode rendering plus metadata. Analysis, validation, lint catalog, SVG/layout/parse, and editor API wrappers are not exported. |
| `@mermanjs/web/editor` | `browser-editor` | Full-family analysis, validation, facts, and parser-backed editor APIs for a dedicated Worker. Render, layout, parse JSON, ASCII, host, and ELK wrappers are not exported. |
| `@mermanjs/web/full` | `browser-full` | Explicit full preset import; equivalent capabilities to the default package. |
| `@mermanjs/web/catalog` | None | Pure generated diagram/theme/capability catalogs and normalizers; does not initialize or import WASM. |
| `@mermanjs/web/svg-safety` | None | Pure SVG DOM-safety assertion and policy helpers for isolated render realms; does not initialize or import WASM. |
| `@mermanjs/web/text-measurement-abi` | None | Generated ABI 2 text-measurement operation descriptors; does not initialize or import WASM. |

There is no separate `@mermanjs/web/analysis` entry point. `@mermanjs/web/core` is already the
smallest analysis-capable artifact, so an analysis alias would add API surface without reducing the
download size.

## Usage

```ts
import { initMerman, renderSvg } from "@mermanjs/web";

await initMerman();

const svg = renderSvg("flowchart TD\nA[Hello] --> B[World]", {
  svg: { pipeline: "readable" },
});
```

The options object is serialized to the shared merman binding options JSON contract documented in
`docs/bindings/OPTIONS_JSON.md`.

Choose a profile from the shared [resource decision table](https://github.com/Latias94/merman/blob/main/docs/bindings/OPTIONS_JSON.md#resource-options), then use the generated helper:

```ts
import { initMerman, renderSvg, resourceOptions, withResourceOptions } from "@mermanjs/web";

await initMerman();
const options = withResourceOptions(
  {},
  resourceOptions("constrained", { max_source_bytes: 1_048_576 }),
);
const svg = renderSvg("flowchart TD\nA[Hello] --> B[World]", options);
```

Use `constrained` for untrusted, public, or multi-tenant input; `interactive` is for cooperative
local editing. The native CLI's default is intentionally separate (`trusted-native`).

Host/editor theme presets are separate from Mermaid's native `theme` names:

```ts
import { initMerman, renderSvg, supportedHostThemePresets } from "@mermanjs/web";

await initMerman();

const presets = supportedHostThemePresets();
const svg = renderSvg("flowchart TD\nA[Hello] --> B[World]", {
  host_theme: { preset: "one-dark" },
});
```

## Browser DOM helper

For non-framework browser integrations, render directly into a host element:

```ts
import { initMerman, renderSvgToElement } from "@mermanjs/web";

await initMerman();

renderSvgToElement(document.querySelector("#preview")!, "sequenceDiagram\nA->>B: hello", {
  svg: { diagram_id: "preview" },
});
```

Framework integrations can use `renderSvg()` and mount the returned SVG string with their normal
HTML/SVG insertion path.

When inserting raw SVG into a browser DOM, keep `assertSafeSvgForDom()` in the path or use the
wrapper helpers that call it. The shared DOM insertion policy is documented in
[`docs/security/RENDERING_SECURITY.md`](https://github.com/Latias94/merman/blob/main/docs/security/RENDERING_SECURITY.md).

## Browser text measurement

Headless rendering cannot know the exact browser font fallback that will display the final SVG.
This can show up as clipped trailing characters or slightly different wrapping when a browser,
WebView, or user font stack resolves text differently from merman's built-in headless metrics.

For browser previews where label geometry must match the displayed font stack, provide a host text
measurer. The helper below measures text with an offscreen DOM probe and falls back to merman's
vendored measurer when the DOM is unavailable or a request is not handled:

```ts
import {
  createBrowserTextMeasurementSession,
  initMerman,
  renderSvgWithTextMeasurer,
} from "@mermanjs/web";

await initMerman();

const measurement = createBrowserTextMeasurementSession();
try {
  const svg = renderSvgWithTextMeasurer(
    "flowchart TD\nA[Start] --> B{Condition?}",
    measurement.measure,
    {
      site_config: {
        fontFamily: '"trebuchet ms", verdana, arial, sans-serif',
        themeVariables: {
          fontFamily: '"trebuchet ms", verdana, arial, sans-serif',
        },
      },
    }
  );
} finally {
  measurement.dispose();
}
```

Use the same font family in both the binding options and your surrounding UI/CSS. If rendering in a
Web Worker, keep using `renderSvg()` with the headless measurer, or send measurement requests to the
main thread through your own worker protocol.

`createBrowserTextMeasurementSession()` creates no DOM nodes until its `measure` callback is first
used. Reuse that callback for related renders, then call `dispose()` to remove its HTML/SVG probes
and release Canvas state. A disposed session remains disposed. The callback measures the natural
no-wrap width for HTML-like labels before it applies `maxWidth`; custom measurers should keep that
behavior because returning `maxWidth` for a short label can make the diagram wider than Mermaid
would make it in the browser.
Requests carry the exact primitive in `operation`, including SVG `getBBox()`,
`getComputedTextLength()`, and `getBoundingClientRect()` variants. Custom callbacks return a
TypeScript discriminated union with `kind: "metrics"`, `"length"`, `"horizontal-extents"`, or
`"wrapped-with-raw-width"`. Wrong-kind results are invalid and use the configured fallback; the
wrapper never infers a result shape from optional fields.
The browser measurer also implements `mermaid-calculate-text-dimensions` with Mermaid's
body-attached `calculateTextDimensions()` SVG probe and `canvas-measure-text-width` with
Cytoscape's Canvas2D `measureText()` font string. These remain distinct because their font fallback
and shaping behavior can differ from ordinary SVG text measurement.

The current wrapper requires WASM ABI 2 and exposes 19 text-measurement operations with contiguous
codes `0..18`. Operation 18, `raw-bbox-height`, returns the non-negative height from a direct SVG
`<text>.getBBox()` probe. Operation 17, `create-text-middle-bbox-y-offset`, is an isolated
formatted-text DOM probe for Architecture's inherited `dominant-baseline="middle"`; it returns a
signed `length`. Operation 14,
`create-text-bbox-y-offset`, measures ordinary createText and is not a valid replacement because
the middle-baseline shift depends on the resolved font's baseline and x-height. Custom callbacks
that cannot reproduce the exact middle-baseline DOM should return `undefined` for operation 17 and
allow the configured fallback to answer it.

`analyze()` returns the diagnostics payload JSON object for a standalone Mermaid diagram.
`analyzeDocument(source, options, uri)` uses the shared document source model to analyze standalone
`.mmd`, Markdown, or MDX documents and returns diagnostics, related locations, and fixes in
host-document coordinates. Downstream lint tools should use `analyzeDocument()` when they scan
Markdown files and want Merman as an optional analysis engine without adopting the LSP.

`analysisFacts(source, options)` and `analyzeDocumentFacts(source, options, uri)` return the richer
analysis facts payload. Use these when an integration needs parser provenance, per-diagram
document/body spans, semantic items, references, expected syntax, or typed Flowchart facts. The
diagnostics shape remains compatible with `analyze()` / `analyzeDocument()`; the additional
`diagrams[].syntax` data is for editor, lint, and preview integrations that want Merman's parser
facts without speaking LSP.

Successful syntax facts add `effective_layout` from the canonical parsed effective configuration.
The field is part of facts schema `2`; consumers must reject a facts payload whose version is not
`2`. `detectDiagramFacts(source, options)` validates that payload and maps its raw parser syntax id
through the runtime family catalog. It returns either
`{ status: "available", diagramType, syntaxId, effectiveLayoutId }` or an explicit unavailable
result. The projection is neutral metadata: choosing or loading Mermaid JS external packages remains
the host application's responsibility.

The diagnostics and facts endpoints expose separate payload contracts. The diagnostics-only
payload remains version 1. The parser-only facts payload is version 2 and rejects facts v1 at the
version boundary before its body is decoded. It uses explicit `fact_source: "unavailable"`
provenance when body semantics are unavailable, and every current `semantic_items[]` entry includes
a `rename_policy` field.

The TextScan-capable alpha shape shipped in `0.8.0-alpha.3` with the same numeric discriminator. The
web package does not retain a legacy decoder or dual facts path; consumers must update their schema
handling and generated types for the current facts v2 contract. This version is independent from
LSP document revisions, Mermaid `*-v2` diagram ids, and the wasm-bindgen/package ABI surface.

This web surface is an integration bridge, not a request that external linters copy Merman policy.
Adapters should preserve `merman.*` rule ids for Merman diagnostics and layer their own style rules
under their own namespaces.

## Custom wasm loading

By default, `initMerman()` dynamically imports `../pkg/merman_wasm.js`. If a bundler or CDN setup
needs to provide the wasm-bindgen module or wasm URL explicitly, pass initialization options:

```ts
import type { MermanWasmModule } from "@mermanjs/web";

await initMerman({
  loader: async () =>
    (await import("@mermanjs/web/pkg/merman_wasm.js")) as MermanWasmModule,
  wasm: new URL("@mermanjs/web/pkg/merman_wasm_bg.wasm", import.meta.url),
});
```

Pass the WASM source itself through `wasm` (a URL, `Response`, bytes, compiled
`WebAssembly.Module`, or a promise of one). Do not wrap it in wasm-bindgen's internal
`{ module_or_path }` object; `initMerman()` owns that generated glue contract. Concurrent calls
share the same in-flight initialization promise.

## WASM loading best practices

The published `@mermanjs/web` package currently ships the `browser-full` artifact. It is intended for
playgrounds, diagram editors, documentation previews, and applications that need headless Mermaid
rendering in the browser. Treat it as a feature module, not as first-paint UI code:

- Call `initMerman()` lazily when analysis, the preview pane, or the first diagram render is needed.
- Use `@mermanjs/web/editor` inside a dedicated module Worker when an editor needs the full family
  catalog and language intelligence without the renderer artifact.
- Preload on route hover, editor open, or `requestIdleCallback` when you know rendering is likely.
- Keep one initialized module per page; `initMerman()` is asynchronous, idempotent, and shares
  concurrent initialization work.
- Serve content-hashed WASM with `Content-Type: application/wasm`, gzip or brotli compression, and
  long-lived immutable HTTP caching. HTML should revalidate. Do not add Cache Storage unless the
  product has an explicit offline lifecycle and freshness contract.
- Use `renderSvg()` in framework code and mount the returned SVG string through your normal
  framework path. Use `renderSvgElement()` / `renderSvgToElement()` only on the main thread because
  they require `DOMParser` and `document`.

The package publishes subpaths for the core, render, ASCII, editor, and full browser artifacts. Call
`bindingCapabilities()` after initialization before relying on optional `render`, `ascii`,
`analysis`, `cytoscape_layout`, `elk_layout`, `ratex_math`, or `editor_language` capabilities. Read
`system_adapter_ids` separately when an operation requires a compiled clock, timezone, random, or
timing adapter; browser artifacts do not claim native system adapters.
The slim subpaths are capability-specific entry points, not full API aliases. They type-re-export
the shared public option/result types and stable helper values, then export only the runtime
wrappers that make sense for that surface. Use `@mermanjs/web/full` or the default import when you
want one module namespace with render, ASCII, and editor-language wrappers together.
`diagramFamilyCapabilities()` reports the complete pinned Mermaid family catalog shared by every
artifact: logical/render identities, detector and semantic/editor parser support, authoring
headers, config namespaces, and typed-render availability. Artifacts with
`bindingCapabilities().analysis === true` also expose `lintRuleCatalog()`
for analyzer rule ids, evidence references, default profiles, origins, configurability, and
fixability.

Each published subpath has its own runtime state. Initializing `@mermanjs/web/core` in the same
process as `@mermanjs/web/full` does not reuse or contaminate the default full module's capability
cache.

## Web Worker integration

`@mermanjs/web` does not bundle an opinionated worker protocol. Worker queues, cancellation,
document versioning, timeouts, and framework integration belong to the host application. The
recommended pattern is to initialize one capability-specific subpath inside a module Worker. For
language intelligence, use `@mermanjs/web/editor`, keep a URI plus monotonically increasing
document version, create one `BrowserEditorSession` per open document, and discard stale query
results. Call `update()` only with a newer document version and always call `dispose()` when the
Worker or document closes. For off-main-thread rendering, initialize the full or render subpath and
send SVG strings back to the main thread:

```ts
// merman.worker.ts
import { initMerman, renderSvg, type SvgBindingOptions } from "@mermanjs/web";

type RenderRequest = {
  id: string;
  source: string;
  options?: SvgBindingOptions;
};

let ready: Promise<unknown> | null = null;

self.onmessage = async (event: MessageEvent<RenderRequest>) => {
  const { id, source, options } = event.data;
  try {
    ready ??= initMerman();
    await ready;
    self.postMessage({ id, ok: true, svg: renderSvg(source, options) });
  } catch (error) {
    self.postMessage({
      id,
      ok: false,
      error: error instanceof Error ? error.message : String(error),
    });
  }
};
```

```ts
// main thread
const worker = new Worker(new URL("./merman.worker.ts", import.meta.url), {
  type: "module",
});

worker.postMessage({
  id: "diagram-1",
  source: "flowchart TD\nA[Hello] --> B[World]",
  options: { svg: { pipeline: "readable" } },
});
```

Use a worker for large documents, repeated batch rendering, or editor keystroke previews where
synchronous rendering could block input. For occasional single-diagram renders, lazy main-thread
initialization is usually simpler.

## API surface

The default `@mermanjs/web` entry point and `@mermanjs/web/full` expose the full wrapper set:

- `initMerman()`, `getMerman()`, `isMermanInitialized()`
- `renderSvg()`, `renderSvgWithTextMeasurer()`, `renderSvgElement()`, `renderSvgToElement()`
- `renderAscii()`
- `parseJson()`, `parseObject()`
- `layoutJson()`, `layoutJsonWithTextMeasurer()`, `layoutObject()`
- `analyze()`, `analyzeJson()`, `analyzeDocument()`, `analysisFacts()`, `detectDiagramFacts()`,
  `analyzeDocumentFacts()`, `validate()`
- `createEditorSession()`, `editorDiagramDetection()`, `editorDiagnostics()`,
  `editorCodeActions()`, `editorCompletions()`, `editorHover()`,
  `editorDocumentSymbols()`, `editorWorkspaceSymbols()`, `editorDefinition()`,
  `editorReferences()`, `editorPrepareRename()`, `editorRename()`,
  `editorSemanticTokenDescriptor()`, `editorSemanticTokens()`
- `supportedDiagrams()`, `asciiSupportedDiagrams()`, `supportedThemes()`, `supportedHostThemePresets()`
- `SUPPORTED_DIAGRAMS`, `SUPPORTED_ASCII_DIAGRAMS`, `isDiagramType()`, `isAsciiDiagramType()`
- `createBrowserTextMeasurementSession()`, `bindingCapabilities()`, `runtimeContract()`, `diagramFamilyCapabilities()`, `lintRuleCatalog()`
- `abiVersion()`, `packageVersion()`, `encodeOptions()`

`@mermanjs/web/core` exports initialization, analysis/facts, validation, metadata, ABI/package
metadata, shared types, stable constants, type guards, and `encodeOptions()`.
`@mermanjs/web/render` adds the SVG, parse, layout, DOM SVG, and browser text-measurement helpers.
`@mermanjs/web/render-only` exposes the same render helpers without the diagnostics analysis
wrappers.
`@mermanjs/web/ascii` adds `renderAscii()`, `asciiSupportedDiagrams()`, and
`asciiCapabilities()` without the diagnostics analysis wrappers. Unsupported wrappers are absent
from slim entry points rather than exported as throwing stubs.
`@mermanjs/web/editor` exports analysis/facts, validation, metadata, and all editor-language
queries over the full 35-family catalog. `createEditorSession()` owns one analyzed document and
reuses it across diagnostics, completion, navigation, rename, and semantic-token queries until
`update()` replaces it or `dispose()` releases it. Its native browser ABI is 2; editor diagnostics
remain schema 1 and shared analysis facts are schema 2.

All render, parse, layout, analysis, validation, editor, and metadata functions require
`initMerman()` first. The stateless editor functions remain available for one-shot consumers.
Interactive editors should use `createEditorSession()` so every query reads one versioned analyzed
snapshot instead of rebuilding it from source. A disposed session fails closed and cannot be
reused. Both forms are backed by `merman-editor-core` and return UTF-16 positions/ranges so Monaco
and LSP adapters can project the same completion, diagnostics, hover, symbol, code-action, rename,
and semantic-token semantics.
Editor query results expose semantic fact provenance where applicable, matching the
`ParserComplete`, `ParserRecovered`, and `Unavailable` boundary used by `merman-editor-core`.
`SEMANTIC_TOKEN_DESCRIPTOR` and its generated digest, legend, and packing constants define the
shared token identity. `editorSemanticTokens()` returns the validated LSP-relative `Uint32Array`
in five-word records; consumers must not reorder the records or translate token names.
`supportedDiagrams()`, `asciiSupportedDiagrams()`, `supportedThemes()`, and
`supportedHostThemePresets()` return typed metadata and fail fast if the generated WebAssembly
metadata drifts from the TypeScript surface. `lintRuleCatalog()` is available only on
analysis-capable artifacts. ASCII support is typed and admitted separately from SVG diagram
metadata so the two rendering surfaces can evolve independently without implying capability from
family membership alone.

## Benchmarking against Mermaid JS

Do not compare one engine after initialization with the other's load plus first render. A valid
browser comparison uses equivalent isolated Window realms and reports acquisition, initialization,
valid SVG, and presentation as separate observations. A cold realm is not necessarily a
network-cold request; retain Resource Timing evidence without inferring cache provenance the
browser did not expose.

Use identical frozen source/options, await fonts, apply equal real-source warmups, balance AB/BA
order with a recorded seed, retain raw samples and failures, and omit ratios when either side is
invalid. Hidden/frozen/navigation boundaries invalidate the run. The Playground implements this as
benchmark protocol 1 and trace schema 1; Compare's interactive render duration is deliberately not
the benchmark. Native `merman-cli` benchmarks remain separate because they do not include the same
realm, module, DOM, or presentation costs.

## License

This package is dual-licensed under either Apache-2.0 or MIT. See `LICENSE` for the full license
texts. Mermaid compatibility and upstream Mermaid MIT attribution are documented in
[`THIRD_PARTY_NOTICES.md`](https://github.com/Latias94/merman/blob/main/THIRD_PARTY_NOTICES.md).
