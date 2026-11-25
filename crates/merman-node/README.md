# Merman Node transports

This private crate builds the native N-API transport for `@mermanjs/node` and the explicit
Node-targeted WASM transport for `@mermanjs/node-wasm`. The crate inherits the root Cargo
workspace version, profiles, and lockfile.

Applications install the matching npm package; this Rust crate is not published separately.

Both candidate artifact recipes explicitly select `all-diagrams`. Family selectors are independent
of outputs; custom source subsets must be validated against the actual consumer graph.
Both `transport-napi` and `transport-wasm` require the direct `svg`, `layout-cytoscape`,
`layout-dagre`, and `layout-elk` features. The ordinary SVG recipe also selects `math`. These
leaves forward to `merman-bindings-core`; both transports share `BindingEngine` execution.
Node transport API `3` exposes the Theme v2 operation IDs and SVG theme metadata. The N-API SVG
methods pass source and optional Options JSON separately. The generic operation and Node-WASM
paths retain their bounded JSON request documents.
These static-SVG artifacts therefore have explicit EPL-2.0 ELK and RaTeX notice/provenance
obligations. Expert source builds retain
additive feature selection; `math` is advertised only when this crate selects it. Neither
transport accepts a JavaScript text-measurement callback.

`platforms/node/scripts/build-candidate.mjs` builds N-API with the `native-sdk` profile and
Node-WASM with the `release` profile. It selects an explicit transport and verifies the root
`Cargo.lock` against that transport's target-specific dependency tree. A workspace build with no
transport features exports no Node transport.

Use `--local-shoal` on either recipe to add the `layout-shoal` capability to a private N-API
package pair or Node-WASM package. [Build and package the local comparison](../../platforms/node/README.md#local-shoal-comparison).
