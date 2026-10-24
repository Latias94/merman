#![deny(unsafe_code)]

//! Independent, bounded scalar WASI ABI for the shared safe binding engine.
//!
//! The production transport is a `wasm32-wasip1` cdylib reactor. Its ABI is not the
//! native C function table and does not accept host callbacks or icon services.

use merman_bindings_core::ValidatedArtifactContract;

#[cfg(any(test, all(target_arch = "wasm32", target_os = "wasi")))]
mod abi;
#[cfg(any(test, all(target_arch = "wasm32", target_os = "wasi")))]
mod fonts;
#[cfg(any(test, all(target_arch = "wasm32", target_os = "wasi")))]
mod memory;
#[cfg(any(test, all(target_arch = "wasm32", target_os = "wasi")))]
mod runtime;
#[cfg(any(test, all(target_arch = "wasm32", target_os = "wasi")))]
mod wire;

#[cfg(test)]
mod tests;

/// Compatibility epoch of the independent WASI transport.
pub const ABI_VERSION: u32 = 1;
/// Fixed little-endian result descriptor size, in bytes.
pub const RESULT_DESCRIPTOR_SIZE: usize = 40;

#[allow(unexpected_cfgs)]
static ARTIFACT_CONTRACT: ValidatedArtifactContract =
    merman_bindings_core::native_sdk_artifact_contract!(Wasi);

/// Returns the same exact artifact selection used for engine admission and discovery.
#[must_use]
pub const fn artifact_contract() -> &'static ValidatedArtifactContract {
    &ARTIFACT_CONTRACT
}
