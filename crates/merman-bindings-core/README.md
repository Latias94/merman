# merman-bindings-core

Safe shared binding facade for Merman native bindings.

`merman-bindings-core` is an implementation crate used by the C ABI and UniFFI binding crates. It
keeps the JSON options contract, error mapping, and feature-gated render entry points in one place
so platform bindings expose the same behavior.
It also owns metadata discovery for Mermaid core themes separately from host/editor theme presets.

Most applications should use one of the public packages instead:

- Rust: [`merman`](https://crates.io/crates/merman)
- C ABI and native hosts: [`merman-ffi`](https://crates.io/crates/merman-ffi)
- Python/UniFFI packaging: [`merman-uniffi`](https://crates.io/crates/merman-uniffi)

## Features

- `render` enables SVG rendering through the main Merman facade.
- `analysis` enables diagnostics analysis, validation JSON, document facts, and lint rule catalog
  helpers.
- `ascii` enables ASCII/Unicode text rendering.
- `raster` enables PNG/JPG/PDF conversion through the main facade.
- `math` enables the RaTeX math label backend.

## SVG Output Contract

`render_svg` and the cached engine `render_svg` entry point return SVG bytes. With empty options,
that SVG is the Mermaid-parity contract. Hosts that pass SVG to strict SVG renderers or
rasterizers should request the export contract with:

```json
{ "svg": { "pipeline": "resvg-safe" } }
```

Editor previews that inject host CSS can also use `host_theme` presets, or enable
`drop_native_duplicate_fallbacks` when duplicate native/fallback labels are visible in the host
surface.

Hosts that inline SVG in a browser and want fallback text while retaining the original
`<foreignObject>` nodes can use `"readable"` instead. Raster byte outputs are intentionally not part
of the shared low-level binding contract; use this SVG pipeline option or the higher-level Rust/CLI
raster helpers.

## Capability Metadata

`binding_capabilities()` reports compiled output and host capabilities.
`diagram_family_capabilities()` exposes the one complete pinned Mermaid language catalog: detector,
semantic/editor parser, typed render, authoring-header, and config-namespace facts. This catalog is
not selected by a build profile. Use the output capability bits to determine whether a particular
artifact can render, analyze, or emit ASCII without assuming behavior from a package name.
The `analysis` capability bit is independent from `render` and `ascii`; slim artifacts can expose
ASCII or render output without compiling diagnostics and lint catalog support.

Diagnostics payloads and rich parser-only document facts are independent contracts: diagnostics
remain schema v1, while facts are schema v2. Facts v1 is rejected at the version boundary before
its body is decoded. Current facts writers always emit `rename_policy`; this does not revive the
removed TextScan-capable alpha decoder, executor, or parallel binding path. Transport and platform
ABI versions are independent from these JSON schema versions.

With `render` enabled, this crate centralizes the host text-measurement result-shape contract. The
current alpha transports expose 19 exact operations with contiguous codes `0..18`; operation 18 is
`raw-bbox-height` and requires a length result. C, UniFFI, and WASM transports report ABI 2 while the
alpha contract evolves.

For product scope, diagram coverage, and compatibility policy, see the
[project README](https://github.com/Latias94/merman#readme) and
[alignment status](https://github.com/Latias94/merman/blob/main/docs/alignment/STATUS.md).
