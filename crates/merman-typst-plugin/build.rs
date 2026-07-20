use serde_json::Value;
use std::{env, fs, path::PathBuf};

const PROFILE_DESCRIPTOR: &str = "wasm-profiles.json";

fn main() {
    println!("cargo:rerun-if-changed={PROFILE_DESCRIPTOR}");

    let descriptor = fs::read_to_string(PROFILE_DESCRIPTOR)
        .expect("failed to read the Typst WASM profile descriptor");
    let descriptor: Value = serde_json::from_str(&descriptor)
        .expect("failed to parse the Typst WASM profile descriptor");
    let abi_version = descriptor
        .get("plugin_abi_version")
        .and_then(Value::as_u64)
        .and_then(|version| u32::try_from(version).ok())
        .filter(|version| *version > 0)
        .expect("plugin_abi_version must be a positive u32");

    let generated = format!(
        "pub const TYPST_PLUGIN_ABI_VERSION: u32 = {abi_version};\n\
         pub const TYPST_PLUGIN_ABI_VERSION_BYTES: &[u8] = b\"{abi_version}\";\n"
    );
    let output = PathBuf::from(env::var_os("OUT_DIR").expect("OUT_DIR is set by Cargo"))
        .join("typst_plugin_abi.rs");
    fs::write(output, generated).expect("failed to write generated Typst plugin ABI constants");
}
