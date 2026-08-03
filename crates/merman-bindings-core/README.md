# merman-bindings-core

Safe shared binding facade for Merman native bindings.

`merman-bindings-core` is an implementation crate used by the C ABI and UniFFI binding crates. It keeps the JSON options contract, error mapping, and feature-gated render entry points in one place so platform bindings expose the same behavior. It also owns metadata discovery for Mermaid core themes separately from host/editor theme presets.

Most applications should use one of the public packages instead:

- Rust: [`merman`](https://crates.io/crates/merman)
- C ABI and native hosts: [`merman-ffi`](https://crates.io/crates/merman-ffi)
- Python and Apple Swift binding generation: [`merman-uniffi`](https://crates.io/crates/merman-uniffi)

## Features

- `svg` enables SVG rendering through the main Merman facade.
- `analysis` enables diagnostics analysis, validation JSON, document facts, and lint rule catalog helpers.
- `ascii` enables ASCII/Unicode text rendering.
- `layout-cytoscape` and `layout-elk` enable their named SVG layout engines.
- `math` enables the RaTeX math label backend.

The ABI 3 operation catalog exposes real `png`, `jpeg`, and `pdf` byte outputs when their corresponding capabilities are compiled. A build that omits one returns the same structured missing-capability error as every other unavailable operation; it does not advertise a phantom backend.

## Runtime Policy

`BindingEngine::new()` is always deterministic. Transport bindings use `BindingEngine::from_options()`, where an omitted `runtime_policy` also selects the deterministic clock, UTC time zone, and fixed random seed even when system adapter features are compiled. Native host state is opt-in:

```json
{ "runtime_policy": "native" }
```

The native policy requires the `system-clock`, `system-timezone`, and `system-random` capabilities; a missing adapter is reported with the `unsupported-operation` status and `missing-capability` kind. Generic operation metadata records the selected policy as `runtime_policy`, so hosts can attest the environment that produced an output. Custom Rust operation contexts remain constructor-owned and cannot be combined with the JSON selector.

## SVG Output Contract

`render_svg` and the cached engine `render_svg` entry point return SVG bytes. With empty options, that SVG is the Mermaid-parity contract. Hosts that pass SVG to strict SVG renderers or rasterizers should request the export contract with:

```json
{ "svg": { "pipeline": "resvg-safe" } }
```

Editor previews can supply a semantic host theme under `presentation.theme`, select an optional first-party product profile under `presentation.profile`, or enable `drop_native_duplicate_fallbacks` when duplicate native/fallback labels are visible in the host surface. Raw Mermaid overrides remain top-level `site_config`, while SVG pipeline and postprocessing choices remain under `svg`.

Hosts that inline SVG in a browser and want fallback text while retaining the original `<foreignObject>` nodes can use `"readable"` instead. PNG, JPEG, and PDF are available through the same generic binding operation route when their output capability is compiled; their format-specific resource limits remain part of the selected resource policy.

## Capability Metadata

`runtime_catalog()` reports one artifact's supported options and binding-payload schemas, named metadata IDs, sorted capability/output/operation/system-adapter IDs, text-measurement providers, registry facts, and resource-to-operation mappings. Consumers validate its flat schema and local relations while tolerating newly introduced stable IDs. Artifact owners must construct that catalog from callable endpoints rather than package names or transport-local booleans. `diagram_family_capabilities()` exposes the one complete pinned Mermaid language catalog: detector, semantic/editor parser, typed render, authoring-header, and config-namespace facts. This catalog is not selected by a build profile. The `analysis` capability is independent from `svg` and `ascii`; slim artifacts can expose ASCII or render output without compiling diagnostics and lint catalog support.

Diagnostics payloads and rich parser-only document facts are independent contracts whose final shapes both use schema v1. Other facts versions are rejected at the boundary before the body is decoded. Current facts writers always emit `rename_policy`; this does not revive the removed TextScan-capable decoder, executor, or parallel binding path. Transport and platform ABI versions are independent from these JSON schema versions.

With `svg` enabled, this crate centralizes the host text-measurement result-shape contract. The current transports expose 19 exact operations with contiguous codes `0..18`; operation 18 is `raw-bbox-height` and requires a length result. The text-measurement protocol remains independently versioned at 1, while the native C ABI, UniFFI binding API, and browser WASM transport each report their own API version through the runtime catalog.

For product scope, diagram coverage, and compatibility policy, see the [project README](https://github.com/Latias94/merman#readme) and [alignment status](https://github.com/Latias94/merman/blob/main/docs/alignment/STATUS.md).
