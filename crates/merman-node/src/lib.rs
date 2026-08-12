#![cfg_attr(not(feature = "transport-napi"), forbid(unsafe_code))]
#![cfg_attr(feature = "transport-napi", deny(unsafe_code))]

//! Native `@mermanjs/node` transport and internal comparison transport.
//!
//! The public npm package selects the N-API transport. The Node-targeted WASM transport remains an
//! internal comparison implementation. Both delegate every operation, option, resource profile,
//! and typed error to `merman-bindings-core`.

#[cfg(not(feature = "svg"))]
compile_error!("the Node candidates must be built with the direct static SVG feature set");

#[cfg(not(feature = "layout-cytoscape"))]
compile_error!("the Node candidates require the static layout-cytoscape capability recipe");

#[cfg(not(feature = "layout-elk"))]
compile_error!("the Node candidates require the static layout-elk capability recipe");

#[cfg(all(feature = "transport-napi", feature = "transport-wasm"))]
compile_error!("select exactly one Node transport candidate");

#[cfg(not(any(feature = "transport-napi", feature = "transport-wasm")))]
compile_error!("select either `transport-napi` or `transport-wasm`");

#[cfg(all(feature = "transport-napi", target_arch = "wasm32"))]
compile_error!("the napi candidate cannot target wasm32");

#[cfg(all(feature = "transport-wasm", not(target_arch = "wasm32")))]
compile_error!("the Node-targeted WASM candidate must target wasm32");

mod wire;

#[cfg(feature = "transport-napi")]
mod napi_transport;
#[cfg(feature = "transport-wasm")]
mod wasm_transport;
