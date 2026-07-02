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
- `ascii` enables ASCII/Unicode text rendering.
- `raster` enables PNG/JPG/PDF conversion through the main facade.
- `ratex-math` enables the RaTeX math label backend.

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

`binding_capabilities()` reports compiled output and host profiles. `selected_registry_profile()`
reports the active Mermaid registry profile (`"full"` or `"tiny"`), and
`diagram_family_capabilities()` exposes the diagram parser/render facts selected by that profile.
Use this diagnostic surface when a host needs to reason about slim WASM/native artifacts without
assuming capabilities from a package name.

For product scope, diagram coverage, and compatibility policy, see the
[project README](https://github.com/Latias94/merman#readme) and
[alignment status](https://github.com/Latias94/merman/blob/main/docs/alignment/STATUS.md).
