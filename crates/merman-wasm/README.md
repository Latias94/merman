# merman-wasm

[![Crates.io](https://img.shields.io/crates/v/merman-wasm.svg)](https://crates.io/crates/merman-wasm) [![Documentation](https://docs.rs/merman-wasm/badge.svg)](https://docs.rs/merman-wasm)

The internal `wasm-bindgen` transport behind Merman's public browser packages.

Browser and TypeScript applications should install an `@mermanjs/web*` package instead of depending on this crate or loading its raw WASM output. This crate owns the Rust-to-JavaScript transport; the packages own initialization, TypeScript types, text measurement, artifact loading, provenance, and lifecycle.

It is browser-only. It is not a Node.js/SSR transport and is not the import-free Typst WebAssembly plugin.

## Use The Public Packages

See the [browser package guide](https://github.com/Latias94/merman/tree/main/platforms/web#readme) for version-matched installation, initialization, and package selection. The guide distinguishes the current source candidate from published npm artifacts; do not combine this crate's `main`-branch transport contract with an older package release.

## Transport Contract

The generated transport exposes generic operation dispatch plus stable convenience functions for the capabilities compiled into its artifact profile. It can project SVG, semantic JSON, layout JSON, ASCII/Unicode, validation, diagnostics, document facts, metadata, and editor operations.

The browser transport currently reports:

- transport API version `5`;
- runtime catalog schema `1`;
- text-measurement protocol version `1`;
- diagnostics payload schema `1`;
- analysis-facts payload schema `2` (generic semantic facts; Flowchart-only rich graph facts were removed).

These contracts are independently versioned. Native ABI numbers, Typst plugin ABI numbers, Mermaid diagram IDs, and JSON payload versions are not interchangeable.

Transport API `5` removes the parser-owned semantic-token exports. Browser syntax highlighting now
uses the canonical Tree-sitter Mermaid grammar; Merman continues to provide diagnostics,
navigation, completion, rename, analysis, and rendering operations.

Transport API `4` changed the `ascii-capabilities` metadata payload: it replaced
`summary_fallback` with `structured_text_fallback`, added `semantic_coverage` and
`primary_projection`, and retained `support_level` only as a derived compatibility label.

Call `runtimeCatalog()` after initialization to discover the loaded artifact's exact capability, operation, output, system-adapter, resource, and text-measurement IDs. Do not infer availability from exported function names, package names, or Cargo feature names. A stable function whose backend is absent returns a typed `missing-capability` error.

All profiles retain the same pinned Mermaid language catalog. Slim artifacts remove callable rendering, analysis, ASCII, editor, or layout capabilities, not diagram parsers.

## Cooperative cancellation and deadlines

One-shot operation options may include a transport-owned top-level `timeout_ms` field:

```json
{"timeout_ms":250,"resources":{"profile":"interactive"}}
```

`timeout_ms` must be an integer from `0` through `4294967295` milliseconds. The portable upper
bound prevents monotonic-clock arithmetic overflow. The WASM transport removes this field before
passing the remaining object to the shared binding options schema, then installs the corresponding
relative monotonic deadline on `OperationControl`. A deadline is observed only at the renderer's
cooperative checkpoints; an expired call returns a structured `MERMAN_CANCELLED` error with
`details.cancellation.reason = "deadline_exceeded"` and a phase identifier.

The synchronous, single-threaded WASM exports do not expose a mid-call `AbortSignal` and cannot
forcefully interrupt Rust or a host text-measurement callback that is already running. A browser
`AbortSignal` may stop work before a call is entered or after it returns, but it does not change
this cooperative boundary. Hosts that require hard termination must run the WASM artifact in an
isolated Worker (or another process boundary) and terminate that isolation unit; use deadline and
checkpoint cancellation for in-call cooperative stopping.

## Browser Text Measurement

The public wrapper owns `renderSvgWithTextMeasurer`, `layoutJsonWithTextMeasurer`, and `createBrowserTextMeasurementSession`. Host callbacks receive one of 19 exact measurement operations (`0..18`). Dispose the owned measurement session when its browser realm or rendering workflow ends.

Use browser measurement when geometry should follow the page's actual font stack. CI, fixture, and offline workflows use Merman's built-in deterministic, font-agnostic measurer. A host callback should decline work it cannot answer faithfully so that deterministic fallback remains authoritative for the request.

## Maintainer Build

Do not invoke `wasm-pack` with this crate's empty default feature set. Normal builds must go through the browser surface descriptor, which selects one exact artifact profile and feature closure for each package:

```sh
npm install --prefix platforms/web
npm run build --prefix platforms/web
npm run smoke --prefix platforms/web
```

Build only one owned WASM profile when working on a specific package:

```sh
npm run build:wasm:analysis --prefix platforms/web
npm run build:wasm:ascii --prefix platforms/web
npm run build:wasm:editor --prefix platforms/web
npm run build:wasm:render --prefix platforms/web
npm run build:wasm:full --prefix platforms/web
```

`web-surface-descriptor.json` and the canonical artifact-profile registry own those mappings. A hand-written feature list is useful only for local dependency experiments; it is not a package identity or release recipe.

## Capability Boundaries

- `analysis` owns diagnostics, validation, facts, and lint metadata.
- `editor` implies `analysis` and adds protocol-neutral editor sessions.
- `ascii` adds supported terminal output.
- `svg` adds browser SVG operations and JavaScript interop.
- `layout-cytoscape`, `layout-elk`, and `math` imply `svg`.

The complete `@mermanjs/web` package uses the `web-full` profile. Other public packages map to their
own descriptor-owned profiles and contain exactly one matching WASM artifact. Profiles that list
`layout-elk` intentionally carry the EPL-2.0 ELK closure; the profile-specific notice bundle is
part of the package contract.

## License And Notices

Merman-owned code is available under MIT or Apache-2.0. A WASM package's license and notice set is
profile-specific: packages with `layout-elk` carry the EPL-2.0 ELK closure, and packages with
`math` may carry the OFL-1.1 RaTeX font closure. Use the generated package manifest and bundled
notices as the distribution authority.

## Size Verification

```sh
cargo run -p xtask -- wasm-size-matrix \
  --surface web \
  --budget-file docs/release/WASM_SIZE_BUDGETS.json
```

The matrix reports raw, stripped, gzip, and brotli sizes. Committed budgets always use the stripped artifact for compression measurements.

## Related Documentation

- [Browser packages](https://github.com/Latias94/merman/tree/main/platforms/web#readme)
- [Binding options and resource profiles](https://github.com/Latias94/merman/blob/main/docs/bindings/OPTIONS_JSON.md)
- [Host text measurement](https://github.com/Latias94/merman/blob/main/docs/bindings/HOST_TEXT_MEASUREMENT.md)
- [Package and release surfaces](https://github.com/Latias94/merman/blob/main/docs/release/PACKAGE_SURFACES.md)
