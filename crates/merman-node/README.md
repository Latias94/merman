# Merman Node transport candidates

This private crate contains the two U14 transport candidates. It is deliberately a nested Cargo workspace and is not connected to the repository root workspace.

It is not a public Node binding. Applications that need a supported Node or static-site path should invoke `merman-cli` as a child process until one in-process transport passes the repository's admission gates.

Both `transport-napi` and `transport-wasm` require the direct `svg`, `layout-cytoscape`, `layout-elk`, and `math` features. Those leaves forward to `merman-bindings-core`; both candidates call the same `BindingEngine::execute(BindingOperationRequest)` path. Neither transport accepts a JavaScript text-measurement callback.

The build owner is `platforms/node/scripts/build-candidate.mjs`. Do not build this crate with an ambient default feature set or reuse the browser package's WASM output.
