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

#if MERMAN_ABI_VERSION != 2
#error "unexpected merman ABI version"
#endif

int merman_header_smoke(void) {
    MermanBuffer buffer = {0};
    MermanResult result = {MERMAN_OK, buffer};
    uint32_t (*abi_version)(void) = &merman_abi_version;
    const char* (*package_version)(void) = &merman_package_version;
    size_t (*buffer_struct_size)(void) = &merman_buffer_struct_size;
    size_t (*result_struct_size)(void) = &merman_result_struct_size;
    MermanResult (*render_svg)(const uint8_t*, size_t, const uint8_t*, size_t) = &merman_render_svg;
    MermanResult (*render_ascii)(const uint8_t*, size_t, const uint8_t*, size_t) = &merman_render_ascii;
    MermanResult (*parse_json)(const uint8_t*, size_t, const uint8_t*, size_t) = &merman_parse_json;
    MermanResult (*layout_json)(const uint8_t*, size_t, const uint8_t*, size_t) = &merman_layout_json;
    MermanResult (*validate_json)(const uint8_t*, size_t, const uint8_t*, size_t) = &merman_validate_json;
    MermanResult (*supported_diagrams_json)(void) = &merman_supported_diagrams_json;
    MermanResult (*ascii_supported_diagrams_json)(void) = &merman_ascii_supported_diagrams_json;
    MermanResult (*themes_json)(void) = &merman_themes_json;
    void (*free_buffer)(MermanBuffer) = &merman_buffer_free;
    (void)abi_version;
    (void)package_version;
    (void)buffer_struct_size;
    (void)result_struct_size;
    (void)render_svg;
    (void)render_ascii;
    (void)parse_json;
    (void)layout_json;
    (void)validate_json;
    (void)supported_diagrams_json;
    (void)ascii_supported_diagrams_json;
    (void)themes_json;
    (void)free_buffer;
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
