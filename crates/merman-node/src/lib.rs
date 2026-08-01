#![cfg_attr(not(feature = "transport-napi"), forbid(unsafe_code))]
#![cfg_attr(feature = "transport-napi", deny(unsafe_code))]

//! Private Node/SSG transport candidates.
//!
//! Both transports delegate every operation, option, resource profile, and typed error to
//! `merman-bindings-core`. This crate is intentionally not a member of the repository workspace
//! until U14 evidence selects or rejects a transport.

#[cfg(not(feature = "svg"))]
compile_error!("the Node candidates must be built with the direct static SVG feature set");

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
