//! Wire-only wrappers for the fixed Typst WebAssembly export surface.

#![allow(
    unreachable_code,
    reason = "Rust 1.100 diagnoses wasm-minimal-protocol 0.2.1's generated Err(Infallible) branch"
)]

// Keep this allowance on generated protocol glue. The public Rust functions and transport
// behavior remain in lib.rs, where ordinary unreachable-code diagnostics still apply.
wasm_minimal_protocol::initiate_protocol!();

#[wasm_minimal_protocol::wasm_func]
pub fn abi_version() -> &'static [u8] {
    super::abi_version()
}

#[wasm_minimal_protocol::wasm_func]
pub fn package_version() -> Vec<u8> {
    super::package_version()
}

#[wasm_minimal_protocol::wasm_func]
pub fn capabilities_json() -> Vec<u8> {
    super::capabilities_json()
}

#[wasm_minimal_protocol::wasm_func]
pub fn theme_catalog_json() -> Vec<u8> {
    super::theme_catalog_json()
}

#[wasm_minimal_protocol::wasm_func]
pub fn render_svg_json(source: &[u8], options_json: &[u8]) -> Vec<u8> {
    super::render_svg_json(source, options_json)
}

#[wasm_minimal_protocol::wasm_func]
pub fn analyze_json(source: &[u8], options_json: &[u8]) -> Vec<u8> {
    super::analyze_json(source, options_json)
}

#[wasm_minimal_protocol::wasm_func]
pub fn theme_operation_json(operation_id: &[u8], input: &[u8], options_json: &[u8]) -> Vec<u8> {
    super::theme_operation_json(operation_id, input, options_json)
}
