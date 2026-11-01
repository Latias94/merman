fn main() {
    println!("cargo:rerun-if-changed=build.rs");
    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() == Ok("windows")
        && std::env::var("CARGO_CFG_TARGET_ENV").as_deref() == Ok("gnu")
    {
        // cargo-zigbuild skips rustc's -Wl,list.def. A positional DEF preserves
        // the single-entry ABI without auto-exporting Rust implementation symbols.
        // Apply it only to the DLL; static libraries retain their normal behavior.
        let exports = std::path::PathBuf::from(std::env::var_os("OUT_DIR").unwrap())
            .join("merman-native-abi.def");
        std::fs::write(&exports, "EXPORTS\nmerman_get_native_api\n")
            .expect("write the Windows native ABI export definition");
        println!("cargo:rustc-cdylib-link-arg={}", exports.display());
    }
}
