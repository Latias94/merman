use std::fs;
use std::path::PathBuf;

#[test]
fn header_smoke() {
    let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let out_dir =
        std::env::temp_dir().join(format!("merman-ffi-header-smoke-{}", std::process::id()));
    fs::create_dir_all(&out_dir).expect("create header smoke out dir");

    let source = out_dir.join("header_smoke.c");
    fs::write(
        &source,
        r#"
#include "merman.h"

int merman_header_smoke(void) {
    MermanBuffer buffer = {0};
    MermanResult result = {MERMAN_OK, buffer};
    return result.code + (int)result.data.len;
}
"#,
    )
    .expect("write header smoke source");

    let target = current_target();
    cc::Build::new()
        .target(target)
        .host(target)
        .opt_level(0)
        .include(manifest_dir.join("include"))
        .file(&source)
        .out_dir(&out_dir)
        .try_compile("merman_header_smoke")
        .expect("C header should compile");
}

fn current_target() -> &'static str {
    if cfg!(all(
        target_arch = "x86_64",
        target_os = "windows",
        target_env = "msvc"
    )) {
        "x86_64-pc-windows-msvc"
    } else if cfg!(all(
        target_arch = "x86_64",
        target_os = "windows",
        target_env = "gnu"
    )) {
        "x86_64-pc-windows-gnu"
    } else if cfg!(all(target_arch = "x86_64", target_os = "linux")) {
        "x86_64-unknown-linux-gnu"
    } else if cfg!(all(target_arch = "aarch64", target_os = "linux")) {
        "aarch64-unknown-linux-gnu"
    } else if cfg!(all(target_arch = "x86_64", target_os = "macos")) {
        "x86_64-apple-darwin"
    } else if cfg!(all(target_arch = "aarch64", target_os = "macos")) {
        "aarch64-apple-darwin"
    } else {
        panic!("unsupported header smoke target");
    }
}
