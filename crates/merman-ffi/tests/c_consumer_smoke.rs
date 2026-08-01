use libloading::Library;
use serde_json::Value;
use std::mem::{MaybeUninit, align_of, offset_of, size_of};
use std::path::{Path, PathBuf};
use std::process::Command;

type NativeGetApi = unsafe extern "C" fn(
    *const merman_ffi::MermanNativeApiRequest,
    *mut merman_ffi::MermanNativeApi,
) -> merman_ffi::MermanNativeStatus;

#[test]
fn c_consumer_smoke() {
    let library_path = compile_c_consumer();

    unsafe {
        let library = Library::new(&library_path).unwrap_or_else(|error| {
            panic!(
                "failed to load C consumer smoke library {}: {error}",
                library_path.display()
            )
        });
        let smoke: libloading::Symbol<unsafe extern "C" fn(NativeGetApi, i32) -> i32> = library
            .get(b"merman_c_consumer_smoke")
            .expect("load merman_c_consumer_smoke symbol");

        let result = smoke(
            merman_ffi::merman_get_native_api,
            i32::from(has_native_sdk_operation_features()),
        );
        assert_eq!(result, 0, "C consumer smoke returned {result}");

        let c_layout: libloading::Symbol<unsafe extern "C" fn() -> u64> = library
            .get(b"merman_c_layout_fingerprint")
            .expect("load merman_c_layout_fingerprint symbol");
        assert_eq!(
            c_layout(),
            rust_layout_fingerprint(),
            "the generated C header and Rust repr(C) projection disagree"
        );
    }

    if matches_native_sdk_artifact_profile() {
        assert_c_abi_native_runtime_catalog();
    }
}

#[test]
fn abi3_minimum_header_consumer_smoke() {
    let library_path = compile_c_library(
        "tests/abi3_minimum_consumer.c",
        "merman_abi3_minimum_consumer",
        "tests/fixtures/abi3-minimum",
    );

    unsafe {
        let library = Library::new(&library_path).unwrap_or_else(|error| {
            panic!(
                "failed to load ABI3 minimum consumer {}: {error}",
                library_path.display()
            )
        });
        let smoke: libloading::Symbol<unsafe extern "C" fn(NativeGetApi) -> i32> = library
            .get(b"merman_abi3_minimum_consumer_smoke")
            .expect("load ABI3 minimum consumer symbol");
        let result = smoke(merman_ffi::merman_get_native_api);
        assert_eq!(result, 0, "ABI3 minimum consumer returned {result}");
    }
}

fn has_native_sdk_operation_features() -> bool {
    cfg!(all(
        feature = "svg",
        feature = "analysis",
        feature = "ascii",
        feature = "png",
        feature = "jpeg",
        feature = "pdf",
        feature = "layout-cytoscape",
        feature = "layout-elk",
        feature = "math",
        feature = "system-clock",
        feature = "system-timezone",
        feature = "system-random",
    ))
}

fn matches_native_sdk_artifact_profile() -> bool {
    has_native_sdk_operation_features()
}

fn hash_size_t(mut hash: u64, value: usize) -> u64 {
    for byte in value.to_ne_bytes() {
        hash ^= u64::from(byte);
        hash = hash.wrapping_mul(1_099_511_628_211);
    }
    hash
}

fn rust_layout_fingerprint() -> u64 {
    macro_rules! hash_record {
        ($hash:ident, $type:ty) => {
            $hash = hash_size_t($hash, size_of::<$type>());
            $hash = hash_size_t($hash, align_of::<$type>());
        };
    }
    macro_rules! hash_field {
        ($hash:ident, $type:ty, $field:ident) => {
            $hash = hash_size_t($hash, offset_of!($type, $field));
        };
    }

    let mut hash = 14_695_981_039_346_656_037_u64;

    hash_record!(hash, merman_ffi::MermanNativeSlice);
    hash_field!(hash, merman_ffi::MermanNativeSlice, struct_size);
    hash_field!(hash, merman_ffi::MermanNativeSlice, data);
    hash_field!(hash, merman_ffi::MermanNativeSlice, len);

    hash_record!(hash, merman_ffi::MermanNativeBuffer);
    hash_field!(hash, merman_ffi::MermanNativeBuffer, struct_size);
    hash_field!(hash, merman_ffi::MermanNativeBuffer, data);
    hash_field!(hash, merman_ffi::MermanNativeBuffer, len);

    hash_record!(hash, merman_ffi::MermanNativeTextMeasureRequest);
    hash_field!(
        hash,
        merman_ffi::MermanNativeTextMeasureRequest,
        struct_size
    );
    hash_field!(
        hash,
        merman_ffi::MermanNativeTextMeasureRequest,
        text_measurement_protocol_version
    );
    hash_field!(hash, merman_ffi::MermanNativeTextMeasureRequest, text);
    hash_field!(
        hash,
        merman_ffi::MermanNativeTextMeasureRequest,
        font_family
    );
    hash_field!(hash, merman_ffi::MermanNativeTextMeasureRequest, font_size);
    hash_field!(
        hash,
        merman_ffi::MermanNativeTextMeasureRequest,
        font_weight
    );
    hash_field!(hash, merman_ffi::MermanNativeTextMeasureRequest, font_style);
    hash_field!(hash, merman_ffi::MermanNativeTextMeasureRequest, max_width);
    hash_field!(
        hash,
        merman_ffi::MermanNativeTextMeasureRequest,
        line_height
    );
    hash_field!(
        hash,
        merman_ffi::MermanNativeTextMeasureRequest,
        letter_spacing
    );
    hash_field!(
        hash,
        merman_ffi::MermanNativeTextMeasureRequest,
        word_spacing
    );
    hash_field!(hash, merman_ffi::MermanNativeTextMeasureRequest, wrap_mode);
    hash_field!(hash, merman_ffi::MermanNativeTextMeasureRequest, direction);
    hash_field!(
        hash,
        merman_ffi::MermanNativeTextMeasureRequest,
        white_space
    );
    hash_field!(
        hash,
        merman_ffi::MermanNativeTextMeasureRequest,
        has_max_width
    );
    hash_field!(hash, merman_ffi::MermanNativeTextMeasureRequest, phase);
    hash_field!(hash, merman_ffi::MermanNativeTextMeasureRequest, operation);

    hash_record!(hash, merman_ffi::MermanNativeTextMeasureResult);
    hash_field!(hash, merman_ffi::MermanNativeTextMeasureResult, struct_size);
    hash_field!(hash, merman_ffi::MermanNativeTextMeasureResult, handled);
    hash_field!(
        hash,
        merman_ffi::MermanNativeTextMeasureResult,
        has_raw_width
    );
    hash_field!(hash, merman_ffi::MermanNativeTextMeasureResult, result_kind);
    hash_field!(hash, merman_ffi::MermanNativeTextMeasureResult, width);
    hash_field!(hash, merman_ffi::MermanNativeTextMeasureResult, height);
    hash_field!(hash, merman_ffi::MermanNativeTextMeasureResult, length);
    hash_field!(hash, merman_ffi::MermanNativeTextMeasureResult, bbox_left);
    hash_field!(hash, merman_ffi::MermanNativeTextMeasureResult, bbox_right);
    hash_field!(hash, merman_ffi::MermanNativeTextMeasureResult, raw_width);
    hash_field!(hash, merman_ffi::MermanNativeTextMeasureResult, line_count);

    hash_record!(hash, merman_ffi::MermanNativeEngineConfig);
    hash_field!(hash, merman_ffi::MermanNativeEngineConfig, struct_size);
    hash_field!(hash, merman_ffi::MermanNativeEngineConfig, options_json);
    hash_field!(hash, merman_ffi::MermanNativeEngineConfig, text_measure);
    hash_field!(
        hash,
        merman_ffi::MermanNativeEngineConfig,
        text_measure_user_data
    );

    hash_record!(hash, merman_ffi::MermanNativeOperationRequest);
    hash_field!(hash, merman_ffi::MermanNativeOperationRequest, struct_size);
    hash_field!(hash, merman_ffi::MermanNativeOperationRequest, operation);
    hash_field!(hash, merman_ffi::MermanNativeOperationRequest, source);
    hash_field!(hash, merman_ffi::MermanNativeOperationRequest, uri);
    hash_field!(hash, merman_ffi::MermanNativeOperationRequest, options_json);

    hash_record!(hash, merman_ffi::MermanNativeResult);
    hash_field!(hash, merman_ffi::MermanNativeResult, struct_size);
    hash_field!(hash, merman_ffi::MermanNativeResult, allocation_token);
    hash_field!(hash, merman_ffi::MermanNativeResult, status);
    hash_field!(hash, merman_ffi::MermanNativeResult, operation);
    hash_field!(hash, merman_ffi::MermanNativeResult, media_type);
    hash_field!(hash, merman_ffi::MermanNativeResult, data);
    hash_field!(hash, merman_ffi::MermanNativeResult, metadata_or_error_json);

    hash_record!(hash, merman_ffi::MermanNativeApiRequest);
    hash_field!(hash, merman_ffi::MermanNativeApiRequest, struct_size);
    hash_field!(
        hash,
        merman_ffi::MermanNativeApiRequest,
        expected_abi_version
    );
    hash_field!(
        hash,
        merman_ffi::MermanNativeApiRequest,
        expected_minimum_prefix_layout_digest
    );

    hash_record!(hash, merman_ffi::MermanNativeApi);
    hash_field!(hash, merman_ffi::MermanNativeApi, struct_size);
    hash_field!(hash, merman_ffi::MermanNativeApi, abi_version);
    hash_field!(
        hash,
        merman_ffi::MermanNativeApi,
        minimum_prefix_layout_digest
    );
    hash_field!(hash, merman_ffi::MermanNativeApi, full_descriptor_digest);
    hash_field!(hash, merman_ffi::MermanNativeApi, capability_catalog_digest);
    hash_field!(hash, merman_ffi::MermanNativeApi, package_version);
    hash_field!(hash, merman_ffi::MermanNativeApi, runtime_catalog);
    hash_field!(hash, merman_ffi::MermanNativeApi, engine_new);
    hash_field!(hash, merman_ffi::MermanNativeApi, engine_try_close);
    hash_field!(hash, merman_ffi::MermanNativeApi, execute_collect);
    hash_field!(hash, merman_ffi::MermanNativeApi, result_free);
    hash_field!(hash, merman_ffi::MermanNativeApi, metadata_collect);

    hash
}

fn assert_c_abi_native_runtime_catalog() {
    let catalog = runtime_catalog_through_function_table();
    assert_runtime_output_contracts(&catalog);
    let profiles: Value = serde_json::from_str(include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../capabilities/artifact-profiles-v1.json"
    )))
    .expect("artifact profile descriptor must be valid JSON");
    let capability_surface: Value = serde_json::from_str(include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../capabilities/feature-surface-v1.json"
    )))
    .expect("capability descriptor must be valid JSON");
    let profiles = profiles["profiles"]
        .as_array()
        .expect("artifact profiles must be an array");

    for profile_id in [
        "c-abi-native",
        "flutter-android-native",
        "flutter-desktop-native",
        "flutter-ios-native",
    ] {
        let profile = profiles
            .iter()
            .find(|profile| profile["id"] == profile_id)
            .unwrap_or_else(|| panic!("missing {profile_id} artifact profile"));
        let expected = &profile["expected"];

        assert_eq!(
            string_ids(&catalog["capabilities"]["capability_ids"]),
            string_ids(&expected["capabilities"]),
            "the real C ABI runtime capabilities drifted from {profile_id}"
        );
        assert_eq!(
            string_ids(&catalog["capabilities"]["capability_ids"]),
            string_ids(&expected["runtime_ids"]),
            "the real C ABI runtime IDs drifted from {profile_id}"
        );
        assert_eq!(
            string_ids(&catalog["capabilities"]["output_ids"]),
            string_ids(&expected["outputs"]),
            "the real C ABI output report drifted from {profile_id}"
        );
        assert_eq!(
            string_ids(&catalog["capabilities"]["operation_ids"]),
            expected_native_operation_ids(&capability_surface, expected),
            "the real C ABI operation report drifted from {profile_id}"
        );
    }
}

fn assert_runtime_output_contracts(catalog: &Value) {
    let output_ids =
        sorted_unique_string_ids(&catalog["capabilities"]["output_ids"], "runtime output IDs");
    let contracts = catalog["output_contracts"]
        .as_array()
        .expect("runtime output contracts must be an array");
    let contract_ids = contracts
        .iter()
        .map(|contract| {
            let object = contract
                .as_object()
                .expect("runtime output contract must be an object");
            assert_required_fields(
                object,
                &["id", "media_type", "system_fonts", "embedded_images"],
                "runtime output contract",
            );
            let id = contract["id"]
                .as_str()
                .filter(|id| !id.is_empty())
                .expect("runtime output contract ID must be a non-empty string");
            contract["media_type"]
                .as_str()
                .filter(|media_type| !media_type.is_empty())
                .expect("runtime output media type must be a non-empty string");
            assert_optional_system_font_contract(&contract["system_fonts"], id);
            assert_optional_embedded_image_contract(&contract["embedded_images"], id);
            assert_native_output_environment_facts(contract, id);
            id
        })
        .collect::<Vec<_>>();
    assert!(
        contract_ids.windows(2).all(|window| window[0] < window[1]),
        "runtime output contract IDs must be sorted and unique"
    );
    assert_eq!(
        contract_ids, output_ids,
        "runtime output contract IDs must exactly match runtime output IDs"
    );
}

fn assert_optional_system_font_contract(value: &Value, output_id: &str) {
    if value.is_null() {
        return;
    }
    let contract = value
        .as_object()
        .unwrap_or_else(|| panic!("{output_id} system font contract must be an object or null"));
    assert_required_fields(
        contract,
        &[
            "source_id",
            "discovery",
            "cache_scope",
            "host_dependent",
            "caller_configurable",
            "resource_bounded",
        ],
        &format!("{output_id} system font contract"),
    );
    for key in ["source_id", "discovery", "cache_scope"] {
        contract[key]
            .as_str()
            .filter(|value| !value.is_empty())
            .unwrap_or_else(|| panic!("{output_id} system font field `{key}` must be a string"));
    }
    for key in ["host_dependent", "caller_configurable", "resource_bounded"] {
        contract[key]
            .as_bool()
            .unwrap_or_else(|| panic!("{output_id} system font field `{key}` must be a boolean"));
    }
}

fn assert_optional_embedded_image_contract(value: &Value, output_id: &str) {
    if value.is_null() {
        return;
    }
    let contract = value
        .as_object()
        .unwrap_or_else(|| panic!("{output_id} embedded image contract must be an object or null"));
    assert_required_fields(
        contract,
        &[
            "source_ids",
            "filesystem_access",
            "network_access",
            "caller_configurable",
            "limits",
        ],
        &format!("{output_id} embedded image contract"),
    );
    sorted_unique_string_ids(
        &contract["source_ids"],
        &format!("{output_id} embedded image source IDs"),
    );
    for key in ["filesystem_access", "network_access", "caller_configurable"] {
        contract[key].as_bool().unwrap_or_else(|| {
            panic!("{output_id} embedded image field `{key}` must be a boolean")
        });
    }

    let limits = contract["limits"]
        .as_object()
        .unwrap_or_else(|| panic!("{output_id} embedded image limits must be an object"));
    let limit_keys = [
        "max_bytes_per_image",
        "max_total_bytes",
        "max_pixels_per_image",
        "max_total_pixels",
    ];
    assert_required_fields(
        limits,
        &limit_keys,
        &format!("{output_id} embedded image limits"),
    );
    for key in limit_keys {
        let value = &limits[key];
        assert!(
            value.is_null() || value.as_u64().is_some_and(|limit| limit > 0),
            "{output_id} embedded image limit `{key}` must be a positive integer or null"
        );
    }
}

fn assert_native_output_environment_facts(contract: &Value, output_id: &str) {
    let expected_media_type = match output_id {
        "ascii" => "text/plain; charset=utf-8",
        "jpeg" => "image/jpeg",
        "pdf" => "application/pdf",
        "png" => "image/png",
        "svg" => "image/svg+xml",
        _ => panic!("native SDK smoke does not own output `{output_id}`"),
    };
    assert_eq!(contract["media_type"], expected_media_type);

    if matches!(output_id, "ascii" | "svg") {
        assert!(contract["system_fonts"].is_null());
        assert!(contract["embedded_images"].is_null());
        return;
    }

    let fonts = contract["system_fonts"]
        .as_object()
        .expect("binary output system font contract");
    assert_eq!(fonts["source_id"], "host-system");
    assert_eq!(fonts["discovery"], "first-use");
    assert_eq!(fonts["cache_scope"], "process-global");
    assert_eq!(fonts["host_dependent"], true);
    assert_eq!(fonts["caller_configurable"], false);
    assert_eq!(fonts["resource_bounded"], false);

    let images = contract["embedded_images"]
        .as_object()
        .expect("binary output embedded image contract");
    assert_eq!(string_ids(&images["source_ids"]), ["data-url"]);
    assert_eq!(images["filesystem_access"], false);
    assert_eq!(images["network_access"], false);
    assert_eq!(images["caller_configurable"], true);
    let limits = images["limits"]
        .as_object()
        .expect("binary output embedded image limits");
    assert_eq!(limits["max_bytes_per_image"], 16 * 1024 * 1024);
    assert_eq!(limits["max_total_bytes"], 32 * 1024 * 1024);
    assert_eq!(limits["max_pixels_per_image"], 16 * 1024 * 1024);
    assert_eq!(limits["max_total_pixels"], 32 * 1024 * 1024);
}

fn assert_required_fields(object: &serde_json::Map<String, Value>, required: &[&str], label: &str) {
    for key in required {
        assert!(
            object.contains_key(*key),
            "{label} is missing field `{key}`"
        );
    }
}

fn sorted_unique_string_ids<'a>(value: &'a Value, label: &str) -> Vec<&'a str> {
    let values = string_ids(value);
    assert!(
        values.iter().all(|value| !value.is_empty()),
        "{label} must contain non-empty strings"
    );
    assert!(
        values.windows(2).all(|window| window[0] < window[1]),
        "{label} must be sorted and unique"
    );
    values
}

fn expected_native_operation_ids<'a>(
    capability_surface: &'a Value,
    expected: &'a Value,
) -> Vec<&'a str> {
    let capabilities = string_ids(&expected["capabilities"]);
    let mut operations = capability_surface["binding_operations"]
        .as_array()
        .expect("binding operations must be an array")
        .iter()
        .filter(|operation| {
            string_ids(&operation["targets"]).contains(&"native")
                && operation["capability"]
                    .as_str()
                    .is_none_or(|capability| capabilities.contains(&capability))
        })
        .map(|operation| {
            operation["id"]
                .as_str()
                .expect("operation ID must be a string")
        })
        .collect::<Vec<_>>();
    operations.sort_unstable();
    operations
}

fn string_ids(value: &Value) -> Vec<&str> {
    value
        .as_array()
        .expect("expected ID list")
        .iter()
        .map(|value| value.as_str().expect("expected string ID"))
        .collect()
}

fn runtime_catalog_through_function_table() -> Value {
    let digest = merman_ffi::MERMAN_NATIVE_ABI_MINIMUM_PREFIX_LAYOUT_DIGEST.as_bytes();
    let request = merman_ffi::MermanNativeApiRequest {
        struct_size: u32::try_from(size_of::<merman_ffi::MermanNativeApiRequest>()).unwrap(),
        expected_abi_version: merman_ffi::MERMAN_NATIVE_ABI_VERSION,
        expected_minimum_prefix_layout_digest: merman_ffi::MermanNativeSlice {
            struct_size: u32::try_from(size_of::<merman_ffi::MermanNativeSlice>()).unwrap(),
            data: digest.as_ptr(),
            len: digest.len(),
        },
    };
    let mut api = MaybeUninit::<merman_ffi::MermanNativeApi>::uninit();
    unsafe {
        api.as_mut_ptr()
            .cast::<u32>()
            .write(u32::try_from(size_of::<merman_ffi::MermanNativeApi>()).unwrap());
    }
    assert_eq!(
        unsafe { merman_ffi::merman_get_native_api(&request, api.as_mut_ptr()) },
        merman_ffi::MERMAN_NATIVE_STATUS_OK
    );
    let api = unsafe { api.assume_init() };

    let mut result = MaybeUninit::<merman_ffi::MermanNativeResult>::zeroed();
    unsafe {
        result
            .as_mut_ptr()
            .cast::<u32>()
            .write(u32::try_from(size_of::<merman_ffi::MermanNativeResult>()).unwrap());
    }
    assert_eq!(
        unsafe { api.runtime_catalog.unwrap()(result.as_mut_ptr()) },
        merman_ffi::MERMAN_NATIVE_STATUS_OK
    );
    let mut result = unsafe { result.assume_init() };
    let bytes = unsafe {
        std::slice::from_raw_parts(
            result.metadata_or_error_json.data,
            result.metadata_or_error_json.len,
        )
    };
    let catalog = serde_json::from_slice(bytes).expect("runtime catalog JSON");
    unsafe { api.result_free.unwrap()(&mut result) };
    catalog
}

fn compile_c_consumer() -> PathBuf {
    compile_c_library(
        "tests/c_consumer_smoke.c",
        "merman_c_consumer_smoke",
        "include",
    )
}

fn compile_c_library(source: &str, stem: &str, include_dir: &str) -> PathBuf {
    let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let out_dir = std::env::temp_dir().join(format!("{stem}-{}", std::process::id()));
    std::fs::create_dir_all(&out_dir).expect("create C consumer smoke output directory");

    let source = manifest_dir.join(source);
    let include_dir = manifest_dir.join(include_dir);
    let library_path = out_dir.join(shared_library_name(stem));
    let mut build = cc::Build::new();
    let target = current_target();
    build.opt_level(0).target(target).host(target);
    let compiler = build.get_compiler();
    let mut command = compiler.to_command();

    if compiler.is_like_msvc() {
        command
            .arg("/LD")
            .arg("/nologo")
            .arg(format!("/I{}", include_dir.display()))
            .arg(format!("/Fe:{}", library_path.display()))
            .arg(format!(
                "/Fo:{}",
                out_dir.join(format!("{stem}.obj")).display()
            ))
            .arg(&source);
    } else {
        command
            .arg("-std=c11")
            .arg("-Wall")
            .arg("-Wextra")
            .arg("-Werror")
            .arg("-shared")
            .arg("-fPIC")
            .arg("-I")
            .arg(&include_dir)
            .arg(&source)
            .arg("-o")
            .arg(&library_path);
    }

    run_compile_command(command, &library_path);
    library_path
}

fn shared_library_name(stem: &str) -> String {
    if cfg!(target_os = "windows") {
        format!("{stem}.dll")
    } else if cfg!(target_os = "macos") {
        format!("lib{stem}.dylib")
    } else {
        format!("lib{stem}.so")
    }
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
        panic!("unsupported C consumer smoke target");
    }
}

fn run_compile_command(mut command: Command, library_path: &Path) {
    let output = command
        .output()
        .unwrap_or_else(|error| panic!("failed to run C compiler: {error}"));
    if !output.status.success() {
        panic!(
            "failed to compile C consumer smoke library {}\nstatus: {}\nstdout:\n{}\nstderr:\n{}",
            library_path.display(),
            output.status,
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        );
    }
}
