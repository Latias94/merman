# Merman Node transports

This private crate builds the native N-API transport for `@mermanjs/node` and the explicit
Node-targeted WASM transport for `@mermanjs/node-wasm`. It is a nested Cargo workspace and is not
connected to the repository root workspace.

Applications install the matching npm package; this Rust crate is not published separately.

Both candidate artifact recipes explicitly select `all-diagrams`. Family selectors are independent
of outputs; custom source subsets must be validated against the actual consumer graph.
Both `transport-napi` and `transport-wasm` require the direct `svg`, `layout-cytoscape`, and
`layout-elk` features. The ordinary SVG recipe also selects the positive `math` feature. These
leaves forward to `merman-bindings-core`; both transports call the
same `BindingEngine::execute(BindingOperationRequest)` path. These static-SVG artifacts
therefore have explicit EPL-2.0 ELK and RaTeX notice/provenance obligations. Expert source builds retain
additive feature selection; `math` is advertised only when this crate selects it. Neither
transport accepts a JavaScript text-measurement callback.

The build owner is `platforms/node/scripts/build-candidate.mjs`. Do not build this crate with an ambient default feature set or reuse the browser package's WASM output.
