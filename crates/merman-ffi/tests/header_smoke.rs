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
#include "merman_resource_contract.h"
#include <string.h>

#if MERMAN_NATIVE_ABI_VERSION != 3
#error "unexpected native ABI version"
#endif

#if MERMAN_NATIVE_RESULT_SCHEMA_VERSION != 1
#error "unexpected native result schema version"
#endif

int merman_header_smoke(void) {
    MermanNativeSlice slice = {0};
    MermanNativeBuffer buffer = {0};
    MermanNativeTextMeasureRequest measure_request = {0};
    MermanNativeTextMeasureResult measure_result = {0};
    MermanNativeEngineConfig config = {0};
    MermanNativeOperationRequest operation_request = {0};
    MermanNativeResult result = MERMAN_NATIVE_RESULT_INIT;
    MermanNativeApiRequest request = {0};
    MermanNativeApi api = {0};
    MermanNativeIconPack icon_pack = {0};
    MermanNativeEngineServicesConfig services_config = {0};
    MermanNativeStatus (*get_api)(const MermanNativeApiRequest *, MermanNativeApi *) = &merman_get_native_api;
    MermanNativeTextMeasureCallback measure = 0;

    slice.struct_size = MERMAN_NATIVE_STRUCT_SIZE(MermanNativeSlice);
    buffer.struct_size = MERMAN_NATIVE_STRUCT_SIZE(MermanNativeBuffer);
    measure_request.struct_size = MERMAN_NATIVE_STRUCT_SIZE(MermanNativeTextMeasureRequest);
    measure_request.text_measurement_protocol_version = MERMAN_TEXT_MEASUREMENT_PROTOCOL_VERSION;
    measure_result.struct_size = MERMAN_NATIVE_STRUCT_SIZE(MermanNativeTextMeasureResult);
    measure_result.result_kind = MERMAN_TEXT_MEASUREMENT_RESULT_KIND_METRICS;
    config.struct_size = MERMAN_NATIVE_STRUCT_SIZE(MermanNativeEngineConfig);
    config.text_measure = measure;
    operation_request.struct_size = MERMAN_NATIVE_STRUCT_SIZE(MermanNativeOperationRequest);
    operation_request.operation = MERMAN_NATIVE_OPERATION_SEMANTIC_JSON;
    if (result.struct_size != MERMAN_NATIVE_STRUCT_SIZE(MermanNativeResult)) {
        return 15;
    }
    request.struct_size = MERMAN_NATIVE_STRUCT_SIZE(MermanNativeApiRequest);
    request.expected_abi_version = MERMAN_NATIVE_ABI_VERSION;
    api.struct_size = MERMAN_NATIVE_STRUCT_SIZE(MermanNativeApi);
    icon_pack.struct_size = MERMAN_NATIVE_STRUCT_SIZE(MermanNativeIconPack);
    services_config.struct_size = MERMAN_NATIVE_STRUCT_SIZE(MermanNativeEngineServicesConfig);
    services_config.engine_config = config;

    if (MERMAN_NATIVE_OPERATION_DOCUMENT_ANALYSIS_JSON != 11) {
        return 10;
    }
    if (MERMAN_TEXT_MEASUREMENT_OPERATION_WRAPPED_WITH_RAW_WIDTH != 12) {
        return 11;
    }
    if (MERMAN_TEXT_MEASUREMENT_RESULT_KIND_WRAPPED_WITH_RAW_WIDTH != 3) {
        return 12;
    }
    if (MERMAN_NATIVE_FUNCTION_METADATA_COLLECT != 5) {
        return 13;
    }
    if (MERMAN_NATIVE_FUNCTION_ENGINE_NEW_WITH_SERVICES != 6) {
        return 18;
    }
    if (
        MERMAN_NATIVE_API_METADATA_COLLECT_PREFIX_SIZE <=
            MERMAN_NATIVE_API_MINIMUM_PREFIX_SIZE ||
        MERMAN_NATIVE_API_METADATA_COLLECT_PREFIX_SIZE > sizeof(MermanNativeApi)
    ) {
        return 16;
    }
    if (
        MERMAN_NATIVE_API_ENGINE_NEW_WITH_SERVICES_PREFIX_SIZE <=
            MERMAN_NATIVE_API_METADATA_COLLECT_PREFIX_SIZE ||
        MERMAN_NATIVE_API_ENGINE_NEW_WITH_SERVICES_PREFIX_SIZE != sizeof(MermanNativeApi)
    ) {
        return 19;
    }
    if (
        strcmp(MERMAN_NATIVE_ERROR_KIND_GENERIC, "generic") != 0 ||
        strcmp(MERMAN_NATIVE_ERROR_KIND_UNKNOWN_OPERATION, "unknown-operation") != 0 ||
        strcmp(MERMAN_NATIVE_ERROR_KIND_MISSING_CAPABILITY, "missing-capability") != 0
    ) {
        return 14;
    }
    if (
        strcmp(MERMAN_RESOURCE_PROFILE_INTERACTIVE, "interactive") != 0 ||
        strcmp(MERMAN_RESOURCE_LIMIT_MAX_SOURCE_BYTES, "max_source_bytes") != 0 ||
        MERMAN_RESOURCE_LIMIT_MAX_SOURCE_BYTES_MINIMUM != 1 ||
        MERMAN_RESOURCE_LIMIT_MAX_SOURCE_BYTES_OVERRIDABLE != 1 ||
        MERMAN_RESOURCE_LIMIT_SVG_BACKEND_TREE_NODES_OVERRIDABLE != 0
    ) {
        return 17;
    }

    (void)get_api;
    (void)slice;
    (void)buffer;
    (void)measure_request;
    (void)measure_result;
    (void)config;
    (void)operation_request;
    (void)result;
    (void)request;
    (void)api;
    (void)icon_pack;
    (void)services_config;
    return 0;
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

    cc::Build::new()
        .target(target)
        .host(target)
        .opt_level(0)
        .include(manifest_dir.join("include"))
        .file(manifest_dir.join("examples/render_svg_engine.c"))
        .out_dir(&out_dir)
        .try_compile("merman_render_svg_engine_example")
        .expect("service-constructor C example should compile");

    let cpp_source = out_dir.join("header_smoke.cc");
    fs::write(
        &cpp_source,
        r#"
#include "merman.h"
#include "merman_resource_contract.h"
#include <type_traits>

static_assert(MERMAN_NATIVE_ABI_VERSION == 3u, "unexpected native ABI version");
static_assert(MERMAN_NATIVE_RESULT_SCHEMA_VERSION == 1u, "unexpected result schema version");
static_assert(MERMAN_NATIVE_FUNCTION_METADATA_COLLECT == 5, "unexpected metadata slot");
static_assert(
    MERMAN_NATIVE_FUNCTION_ENGINE_NEW_WITH_SERVICES == 6,
    "unexpected service constructor slot"
);
static_assert(
    MERMAN_NATIVE_API_METADATA_COLLECT_PREFIX_SIZE > MERMAN_NATIVE_API_MINIMUM_PREFIX_SIZE,
    "metadata slot must remain outside the frozen prefix"
);
static_assert(
    MERMAN_NATIVE_API_METADATA_COLLECT_PREFIX_SIZE <= sizeof(MermanNativeApi),
    "metadata slot boundary must fit the current table"
);
static_assert(
    MERMAN_NATIVE_API_ENGINE_NEW_WITH_SERVICES_PREFIX_SIZE >
        MERMAN_NATIVE_API_METADATA_COLLECT_PREFIX_SIZE,
    "service constructor must append after the published six-slot prefix"
);
static_assert(
    MERMAN_NATIVE_API_ENGINE_NEW_WITH_SERVICES_PREFIX_SIZE == sizeof(MermanNativeApi),
    "service constructor boundary must be the current complete table"
);
static_assert(
    MERMAN_RESOURCE_LIMIT_MAX_SOURCE_BYTES_MINIMUM == 1 &&
        MERMAN_RESOURCE_LIMIT_MAX_SOURCE_BYTES_OVERRIDABLE == 1 &&
        MERMAN_RESOURCE_LIMIT_SVG_BACKEND_TREE_NODES_OVERRIDABLE == 0,
    "resource contract must expose stable string macros and override metadata"
);
static_assert(
    std::is_nothrow_invocable_r_v<
        MermanNativeStatus,
        MermanNativeTextMeasureCallback,
        const MermanNativeTextMeasureRequest *,
        MermanNativeTextMeasureResult *,
        void *
    >,
    "C++ callback type must be noexcept"
);
static_assert(
    std::is_nothrow_invocable_r_v<
        MermanNativeStatus,
        MermanNativeMetadataCollectFn,
        MermanNativeSlice,
        MermanNativeResult *
    >,
    "C++ metadata function type must be noexcept"
);
static_assert(
    std::is_nothrow_invocable_r_v<
        MermanNativeStatus,
        MermanNativeEngineNewWithServicesFn,
        const MermanNativeEngineServicesConfig *,
        MermanNativeEngineToken *,
        MermanNativeResult *
    >,
    "C++ service constructor function type must be noexcept"
);

int merman_cpp_header_smoke() {
    MermanNativeApiRequest request{};
    MermanNativeApi api{};
    MermanNativeResult result = MERMAN_NATIVE_RESULT_INIT;
    MermanNativeIconPack icon_pack{};
    MermanNativeEngineServicesConfig services_config{};
    auto discover = &merman_get_native_api;

    request.struct_size = MERMAN_NATIVE_STRUCT_SIZE(MermanNativeApiRequest);
    request.expected_abi_version = MERMAN_NATIVE_ABI_VERSION;
    api.struct_size = MERMAN_NATIVE_STRUCT_SIZE(MermanNativeApi);
    icon_pack.struct_size = MERMAN_NATIVE_STRUCT_SIZE(MermanNativeIconPack);
    services_config.struct_size = MERMAN_NATIVE_STRUCT_SIZE(MermanNativeEngineServicesConfig);

    return discover == nullptr ||
        result.struct_size != MERMAN_NATIVE_STRUCT_SIZE(MermanNativeResult) ||
        icon_pack.struct_size != MERMAN_NATIVE_STRUCT_SIZE(MermanNativeIconPack) ||
        services_config.struct_size != MERMAN_NATIVE_STRUCT_SIZE(MermanNativeEngineServicesConfig)
        ? 1 : 0;
}
"#,
    )
    .expect("write C++ header smoke source");
    cc::Build::new()
        .cpp(true)
        .flag_if_supported("-std=c++17")
        .flag_if_supported("/std:c++17")
        .target(target)
        .host(target)
        .opt_level(0)
        .include(manifest_dir.join("include"))
        .file(&cpp_source)
        .out_dir(&out_dir)
        .try_compile("merman_cpp_header_smoke")
        .expect("C++ header should compile");
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
