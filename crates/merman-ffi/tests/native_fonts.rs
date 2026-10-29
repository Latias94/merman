use merman_ffi::*;
use std::mem::{align_of, size_of};
use std::ptr;

fn slice(bytes: &[u8]) -> MermanNativeSlice {
    MermanNativeSlice {
        struct_size: size_of::<MermanNativeSlice>() as u32,
        data: bytes.as_ptr(),
        len: bytes.len(),
    }
}

fn result() -> MermanNativeResult {
    let mut result: MermanNativeResult = unsafe { std::mem::zeroed() };
    result.struct_size = size_of::<MermanNativeResult>() as u32;
    result
}

fn api() -> MermanNativeApiWithFonts {
    let request = MermanNativeApiRequest {
        struct_size: size_of::<MermanNativeApiRequest>() as u32,
        expected_abi_version: MERMAN_NATIVE_ABI_VERSION,
        expected_minimum_prefix_layout_digest: slice(
            MERMAN_NATIVE_ABI_MINIMUM_PREFIX_LAYOUT_DIGEST.as_bytes(),
        ),
    };
    let mut api = MermanNativeApiWithFonts::for_discovery();
    assert_eq!(
        unsafe { merman_get_native_api(&request, api.as_mut_api_ptr()) },
        MERMAN_NATIVE_STATUS_OK
    );
    assert!(api.engine_new_with_fonts.is_some());
    api
}

fn config(fonts: &[MermanNativeSlice]) -> MermanNativeFontEngineConfig {
    MermanNativeFontEngineConfig {
        struct_size: size_of::<MermanNativeFontEngineConfig>() as u32,
        options_json: slice(b""),
        fonts: fonts.as_ptr(),
        font_count: fonts.len(),
        use_bundled_fonts: 0,
    }
}

fn assert_failure(
    config: &MermanNativeFontEngineConfig,
    expected: MermanNativeStatus,
) -> serde_json::Value {
    let api = api();
    let mut engine = 0;
    let mut output = result();
    let status = unsafe { api.engine_new_with_fonts.unwrap()(config, &mut engine, &mut output) };
    assert_eq!(status, expected);
    assert_eq!(engine, 0, "failed construction must not publish an engine");
    assert_eq!(output.status, status);
    assert_ne!(output.allocation_token, 0, "ordinary failures own a result");
    let error = serde_json::from_slice(unsafe {
        std::slice::from_raw_parts(
            output.metadata_or_error_json.data,
            output.metadata_or_error_json.len,
        )
    })
    .unwrap();
    unsafe { api.result_free.unwrap()(&mut output) };
    assert_eq!(output.allocation_token, 0);
    error
}

#[test]
fn font_constructor_is_appended_without_changing_the_abi3_minimum() {
    assert_eq!(
        MERMAN_NATIVE_ABI_MINIMUM_PREFIX_LAYOUT_DIGEST,
        "sha256:623c099f91282a88bf4d4e9cc7cdf728fc39c3b71a3ae7392007dd74f2b6ab41"
    );
    let api = api();
    assert_eq!(
        api.struct_size,
        MERMAN_NATIVE_API_ENGINE_NEW_WITH_FONTS_PREFIX_SIZE
    );
    assert!(
        MERMAN_NATIVE_API_ENGINE_NEW_WITH_FONTS_PREFIX_SIZE
            > MERMAN_NATIVE_API_EXECUTE_COLLECT_CONTROLLED_PREFIX_SIZE
    );
}

#[cfg(not(feature = "native-fonts"))]
#[test]
fn absent_native_fonts_does_not_read_or_validate_the_font_array() {
    let mut config = config(&[]);
    config.fonts = ptr::without_provenance(usize::MAX & !(align_of::<MermanNativeSlice>() - 1));
    config.font_count = usize::MAX;
    let error = assert_failure(&config, MERMAN_NATIVE_STATUS_UNSUPPORTED_OPERATION);
    assert_eq!(error["kind"], "missing-capability");
    assert_eq!(error["capability_id"], "native-fonts");
}

#[cfg(feature = "native-fonts")]
#[test]
fn invalid_font_bytes_and_boolean_return_owned_failures() {
    let malformed = [slice(b"not an OpenType font")];
    assert_failure(&config(&malformed), MERMAN_NATIVE_STATUS_INVALID_ARGUMENT);
    assert_failure(&config(&[]), MERMAN_NATIVE_STATUS_INVALID_ARGUMENT);
    let mut invalid = config(&malformed);
    invalid.use_bundled_fonts = 2;
    assert_failure(&invalid, MERMAN_NATIVE_STATUS_INVALID_ARGUMENT);
    let null_bytes = [MermanNativeSlice {
        data: ptr::null(),
        len: 1,
        ..slice(b"")
    }];
    assert_failure(&config(&null_bytes), MERMAN_NATIVE_STATUS_INVALID_ARGUMENT);
    let mut null_array = config(&[]);
    null_array.fonts = ptr::null();
    null_array.font_count = 1;
    assert_failure(&null_array, MERMAN_NATIVE_STATUS_INVALID_ARGUMENT);
}

#[cfg(feature = "native-fonts")]
#[test]
fn font_count_and_bytes_are_bounded_before_array_reads_and_copies() {
    let limits = merman::svg::export::NativeFontLimits::default();
    let mut excessive = config(&[]);
    excessive.fonts = ptr::without_provenance(usize::MAX & !(align_of::<MermanNativeSlice>() - 1));
    excessive.font_count = limits.max_files + 1;
    assert_failure(&excessive, MERMAN_NATIVE_STATUS_RESOURCE_LIMIT_EXCEEDED);
    let oversized = [MermanNativeSlice {
        data: ptr::without_provenance(1),
        len: limits.max_bytes + 1,
        ..slice(b"")
    }];
    assert_failure(
        &config(&oversized),
        MERMAN_NATIVE_STATUS_RESOURCE_LIMIT_EXCEEDED,
    );
}

#[cfg(feature = "native-fonts")]
#[test]
fn overlapping_font_storage_leaves_both_outputs_untouched() {
    let api = api();
    let mut engine = 0;
    let mut output = result();
    let aliased = [MermanNativeSlice {
        data: ptr::addr_of!(output).cast(),
        len: size_of::<MermanNativeResult>(),
        ..slice(b"")
    }];
    let status =
        unsafe { api.engine_new_with_fonts.unwrap()(&config(&aliased), &mut engine, &mut output) };
    assert_eq!(status, MERMAN_NATIVE_STATUS_INVALID_ARGUMENT);
    assert_eq!(engine, 0);
    assert_eq!(output.allocation_token, 0);
    let mut aliased_array = config(&[]);
    aliased_array.fonts = ptr::addr_of!(output).cast();
    aliased_array.font_count = 1;
    let status =
        unsafe { api.engine_new_with_fonts.unwrap()(&aliased_array, &mut engine, &mut output) };
    assert_eq!(status, MERMAN_NATIVE_STATUS_INVALID_ARGUMENT);
    assert_eq!(engine, 0);
    assert_eq!(output.allocation_token, 0);
}

#[cfg(feature = "native-fonts")]
const FONT: &[u8] = include_bytes!("../../merman-export/tests/fonts/Cousine-Regular.ttf");

#[cfg(feature = "native-fonts")]
#[test]
fn explicit_measurement_selector_conflicts_are_owned_by_the_shared_service() {
    let fonts = [slice(FONT)];
    for options in [
        br#"{"version":2,"environment":{"text_measurement":"deterministic"}}"#.as_slice(),
        br#"{"version":2,"environment":{"text_measurement":null}}"#.as_slice(),
    ] {
        let mut config = config(&fonts);
        config.options_json = slice(options);
        let error = assert_failure(&config, MERMAN_NATIVE_STATUS_INVALID_ARGUMENT);
        assert!(
            error["message"]
                .as_str()
                .unwrap()
                .contains("native-font-context")
        );
        assert!(
            error["message"]
                .as_str()
                .unwrap()
                .contains("environment.text_measurement")
        );
    }
}

#[cfg(all(feature = "native-fonts", not(feature = "bundled-fonts")))]
#[test]
fn unavailable_bundled_fallback_is_an_owned_missing_capability() {
    let fonts = [slice(FONT)];
    let mut config = config(&fonts);
    config.use_bundled_fonts = 1;
    let error = assert_failure(&config, MERMAN_NATIVE_STATUS_UNSUPPORTED_OPERATION);
    assert_eq!(error["kind"], "missing-capability");
}

#[cfg(feature = "bundled-fonts")]
#[test]
fn bundled_fonts_can_supply_the_complete_closed_font_set() {
    let api = api();
    let mut config = config(&[]);
    config.use_bundled_fonts = 1;
    let mut engine = 0;
    let mut output = result();
    assert_eq!(
        unsafe { api.engine_new_with_fonts.unwrap()(&config, &mut engine, &mut output) },
        MERMAN_NATIVE_STATUS_OK
    );
    assert_ne!(engine, 0);
    unsafe { api.result_free.unwrap()(&mut output) };
    assert_eq!(
        unsafe { api.engine_try_close.unwrap()(engine) },
        MERMAN_NATIVE_STATUS_OK
    );
}

#[cfg(all(
    feature = "native-fonts",
    feature = "png",
    feature = "diagram-flowchart"
))]
#[test]
fn real_c_consumer_releases_constructor_inputs_before_svg_and_png() {
    use std::path::PathBuf;
    let manifest = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let out_dir =
        std::env::temp_dir().join(format!("merman-c-font-consumer-{}", std::process::id()));
    std::fs::create_dir_all(&out_dir).unwrap();
    let library_path = out_dir.join(if cfg!(target_os = "windows") {
        "merman_c_fonts.dll"
    } else if cfg!(target_os = "macos") {
        "libmerman_c_fonts.dylib"
    } else {
        "libmerman_c_fonts.so"
    });
    let mut build = cc::Build::new();
    let target = if cfg!(all(target_arch = "aarch64", target_os = "macos")) {
        "aarch64-apple-darwin"
    } else if cfg!(all(target_arch = "x86_64", target_os = "macos")) {
        "x86_64-apple-darwin"
    } else if cfg!(all(target_arch = "aarch64", target_os = "linux")) {
        "aarch64-unknown-linux-gnu"
    } else if cfg!(all(target_arch = "x86_64", target_os = "linux")) {
        "x86_64-unknown-linux-gnu"
    } else if cfg!(all(
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
    } else {
        panic!("unsupported C font consumer target");
    };
    build.opt_level(0).target(target).host(target);
    let compiler = build.get_compiler();
    let mut command = compiler.to_command();
    if compiler.is_like_msvc() {
        command
            .arg("/LD")
            .arg("/nologo")
            .arg(format!("/I{}", manifest.join("include").display()))
            .arg(format!("/Fe:{}", library_path.display()))
            .arg(format!("/Fo:{}", out_dir.join("consumer.obj").display()));
    } else {
        command
            .args([
                "-std=c11", "-Wall", "-Wextra", "-Werror", "-shared", "-fPIC", "-I",
            ])
            .arg(manifest.join("include"))
            .arg("-o")
            .arg(&library_path);
    }
    let compilation = command
        .arg(manifest.join("tests/c_consumer_smoke.c"))
        .output()
        .unwrap();
    assert!(
        compilation.status.success(),
        "{}",
        String::from_utf8_lossy(&compilation.stderr)
    );
    let api = api();
    unsafe {
        let library = libloading::Library::new(&library_path).unwrap();
        let smoke: libloading::Symbol<
            unsafe extern "C" fn(*const MermanNativeApiWithFonts, *const u8, usize, u8) -> i32,
        > = library.get(b"merman_c_font_consumer_smoke").unwrap();
        assert_eq!(smoke(&api, FONT.as_ptr(), FONT.len(), 0), 0);
        #[cfg(feature = "bundled-fonts")]
        assert_eq!(smoke(&api, FONT.as_ptr(), FONT.len(), 1), 0);
    }
}
