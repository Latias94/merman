use std::{env, process::Command};

fn main() {
    println!("cargo::rerun-if-changed=build.rs");
    println!("cargo::rustc-check-cfg=cfg(wasi_legacy_initialize)");

    if env::var("CARGO_CFG_TARGET_ARCH").as_deref() != Ok("wasm32")
        || env::var("CARGO_CFG_TARGET_OS").as_deref() != Ok("wasi")
    {
        return;
    }

    let output = Command::new(env::var_os("RUSTC").expect("Cargo provides RUSTC"))
        .arg("--version")
        .output()
        .expect("query the Rust compiler version");
    assert!(output.status.success(), "rustc --version failed");
    let version = std::str::from_utf8(&output.stdout)
        .expect("rustc version output is UTF-8")
        .split_whitespace()
        .nth(1)
        .expect("rustc reports a version");
    let mut parts = version.split('.');
    let major: u32 = parts
        .next()
        .and_then(|part| part.parse().ok())
        .expect("Rust major version");
    let minor: u32 = parts
        .next()
        .and_then(|part| part.parse().ok())
        .expect("Rust minor version");

    // Rust 1.100 started linking crt1-reactor.o for WASI cdylibs.
    if (major, minor) < (1, 100) {
        println!("cargo::rustc-cfg=wasi_legacy_initialize");
    }
}
