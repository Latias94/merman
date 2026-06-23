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
    MermanEngineResult engine_result = {MERMAN_OK, 0, buffer};
    MermanHostTextMeasureRequest measure_request = {0};
    MermanHostTextMeasureResult measure_result = {0, 0.0, 0.0, 0};
    uint32_t (*abi_version)(void) = &merman_abi_version;
    const char* (*package_version)(void) = &merman_package_version;
    size_t (*buffer_struct_size)(void) = &merman_buffer_struct_size;
    size_t (*result_struct_size)(void) = &merman_result_struct_size;
    size_t (*engine_result_struct_size)(void) = &merman_engine_result_struct_size;
    size_t (*host_text_measure_request_struct_size)(void) = &merman_host_text_measure_request_struct_size;
    size_t (*host_text_measure_result_struct_size)(void) = &merman_host_text_measure_result_struct_size;
    MermanEngineResult (*engine_new)(const uint8_t*, size_t) = &merman_engine_new;
    void (*engine_free)(MermanEngine*) = &merman_engine_free;
    MermanResult (*engine_set_text_measure_callback)(MermanEngine*, MermanHostTextMeasureCallback, void*) = &merman_engine_set_text_measure_callback;
    MermanResult (*engine_render_svg)(const MermanEngine*, const uint8_t*, size_t) = &merman_engine_render_svg;
    MermanResult (*engine_render_ascii)(const MermanEngine*, const uint8_t*, size_t) = &merman_engine_render_ascii;
    MermanResult (*engine_analyze_json)(const MermanEngine*, const uint8_t*, size_t) = &merman_engine_analyze_json;
    MermanResult (*engine_parse_json)(const MermanEngine*, const uint8_t*, size_t) = &merman_engine_parse_json;
    MermanResult (*engine_layout_json)(const MermanEngine*, const uint8_t*, size_t) = &merman_engine_layout_json;
    MermanResult (*engine_validate_json)(const MermanEngine*, const uint8_t*, size_t) = &merman_engine_validate_json;
    MermanResult (*render_svg)(const uint8_t*, size_t, const uint8_t*, size_t) = &merman_render_svg;
    MermanResult (*render_ascii)(const uint8_t*, size_t, const uint8_t*, size_t) = &merman_render_ascii;
    MermanResult (*analyze_json)(const uint8_t*, size_t, const uint8_t*, size_t) = &merman_analyze_json;
    MermanResult (*parse_json)(const uint8_t*, size_t, const uint8_t*, size_t) = &merman_parse_json;
    MermanResult (*layout_json)(const uint8_t*, size_t, const uint8_t*, size_t) = &merman_layout_json;
    MermanResult (*validate_json)(const uint8_t*, size_t, const uint8_t*, size_t) = &merman_validate_json;
    MermanResult (*supported_diagrams_json)(void) = &merman_supported_diagrams_json;
    MermanResult (*ascii_supported_diagrams_json)(void) = &merman_ascii_supported_diagrams_json;
    MermanResult (*diagram_family_capabilities_json)(void) = &merman_diagram_family_capabilities_json;
    MermanResult (*supported_themes_json)(void) = &merman_supported_themes_json;
    MermanResult (*supported_host_theme_presets_json)(void) = &merman_supported_host_theme_presets_json;
    void (*free_buffer)(MermanBuffer) = &merman_buffer_free;
    measure_request.font_style = (const uint8_t*)"normal";
    measure_request.font_style_len = 6;
    measure_request.line_height = 24.0;
    measure_request.letter_spacing = 0.0;
    measure_request.word_spacing = 0.0;
    measure_request.direction = MERMAN_TEXT_DIRECTION_AUTO;
    measure_request.white_space = MERMAN_TEXT_WHITE_SPACE_NOWRAP;
    if (MERMAN_WRAP_MODE_HTML_LIKE != 2) {
        return 10;
    }
    if (MERMAN_TEXT_DIRECTION_RTL != 2) {
        return 11;
    }
    if (MERMAN_TEXT_WHITE_SPACE_BREAK_SPACES != 2) {
        return 12;
    }
    if (MERMAN_RESOURCE_LIMIT_EXCEEDED != 10) {
        return 13;
    }
    (void)abi_version;
    (void)package_version;
    (void)buffer_struct_size;
    (void)result_struct_size;
    (void)engine_result_struct_size;
    (void)host_text_measure_request_struct_size;
    (void)host_text_measure_result_struct_size;
    (void)engine_new;
    (void)engine_free;
    (void)engine_set_text_measure_callback;
    (void)engine_render_svg;
    (void)engine_render_ascii;
    (void)engine_analyze_json;
    (void)engine_parse_json;
    (void)engine_layout_json;
    (void)engine_validate_json;
    (void)render_svg;
    (void)render_ascii;
    (void)analyze_json;
    (void)parse_json;
    (void)layout_json;
    (void)validate_json;
    (void)supported_diagrams_json;
    (void)ascii_supported_diagrams_json;
    (void)diagram_family_capabilities_json;
    (void)supported_themes_json;
    (void)supported_host_theme_presets_json;
    (void)free_buffer;
    (void)measure_request;
    (void)measure_result;
    return result.code + engine_result.code + (int)result.data.len;
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
