#![cfg_attr(not(feature = "transport-napi"), forbid(unsafe_code))]
#![cfg_attr(feature = "transport-napi", deny(unsafe_code))]

//! Native `@mermanjs/node` and explicit `@mermanjs/node-wasm` transports.
//!
//! The native npm package selects N-API; the separate Node-WASM package selects WASM explicitly.
//! Both delegate every operation, option, resource profile,
//! and typed error to `merman-bindings-core`.

#[cfg(all(feature = "transport-napi", feature = "transport-wasm"))]
compile_error!("select exactly one Node transport candidate");

#[cfg(all(feature = "transport-napi", target_arch = "wasm32"))]
compile_error!("the napi candidate cannot target wasm32");

#[cfg(all(feature = "transport-wasm", not(target_arch = "wasm32")))]
compile_error!("the Node-targeted WASM candidate must target wasm32");

// A workspace build without a transport has no Node artifact to validate.
#[cfg(any(feature = "transport-napi", feature = "transport-wasm"))]
mod wire;

#[cfg(feature = "transport-napi")]
mod napi_transport;
#[cfg(feature = "transport-wasm")]
mod wasm_transport;
