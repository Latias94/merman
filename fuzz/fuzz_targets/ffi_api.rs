#![no_main]

use libfuzzer_sys::fuzz_target;
use merman_ffi::*;
use std::mem::MaybeUninit;
use std::ptr;

const MAX_FFI_INPUT_BYTES: usize = 16 * 1024;
const MAX_OPTIONS_BYTES: usize = 256;
const DEFAULT_URI: &[u8] = b"file:///fuzz.mmd";
const FIXED_OPTIONS: &[u8] = br#"{"version":2,"fixed_today":"2025-01-01","fixed_local_offset_minutes":0,"resources":{"limits":{"max_source_bytes":16384,"max_svg_bytes":1048576,"max_model_items":2048,"max_model_text_bytes":65536,"max_layout_work_units":250000}}}"#;
const MALFORMED_OPTIONS: &[u8] =
    br#"{"version":2,"resources":{"limits":{"max_source_bytes":"bad"}}}"#;
const STALE_OPTIONS: &[u8] = br#"{"version":1}"#;

fuzz_target!(|data: &[u8]| {
    if data.len() > MAX_FFI_INPUT_BYTES {
        return;
    }

    let input = decode_input(data);
    let api = discover_api();
    let selector = input.selector % 19;

    match selector {
        12 => consume_runtime_catalog(&api),
        17 => verify_discovery_rejects_a_stale_version(),
        18 => consume_metadata(&api, input.source),
        _ => execute_operation(&api, input, selector),
    }
});

struct FuzzInput<'a> {
    selector: u8,
    source: &'a [u8],
    options: &'a [u8],
    uri: &'a [u8],
}

fn decode_input(data: &[u8]) -> FuzzInput<'_> {
    if let Some(source) = data.strip_prefix(b"parse\n") {
        return text_seed(0, source, &[]);
    }
    if let Some(source) = data.strip_prefix(b"render-fixed-options\n") {
        return text_seed(4, source, FIXED_OPTIONS);
    }
    if let Some(source) = data.strip_prefix(b"invalid-options\n") {
        return text_seed(0, source, MALFORMED_OPTIONS);
    }
    if let Some(source) = data.strip_prefix(b"stale-options-version\n") {
        return text_seed(0, source, STALE_OPTIONS);
    }

    let Some((&selector, source)) = data.split_first() else {
        return text_seed(12, &[], &[]);
    };
    let Some((&options_len, framed)) = source.split_first() else {
        return text_seed(selector, &[], &[]);
    };
    let options_len = usize::from(options_len)
        .min(MAX_OPTIONS_BYTES)
        .min(framed.len());
    let (options, framed) = framed.split_at(options_len);
    let (uri, source) = split_uri(selector, framed);
    FuzzInput {
        selector,
        source,
        options,
        uri,
    }
}

fn text_seed<'a>(selector: u8, source: &'a [u8], options: &'a [u8]) -> FuzzInput<'a> {
    FuzzInput {
        selector,
        source,
        options,
        uri: DEFAULT_URI,
    }
}

fn split_uri(selector: u8, data: &[u8]) -> (&[u8], &[u8]) {
    if selector & 0b1000_0000 == 0 {
        return (DEFAULT_URI, data);
    }
    let Some((&uri_len, framed)) = data.split_first() else {
        return (&[], &[]);
    };
    let uri_len = usize::from(uri_len)
        .min(MAX_OPTIONS_BYTES)
        .min(framed.len());
    framed.split_at(uri_len)
}

fn borrowed_slice(bytes: &[u8]) -> MermanNativeSlice {
    MermanNativeSlice {
        struct_size: native_struct_size::<MermanNativeSlice>(),
        data: if bytes.is_empty() {
            ptr::null()
        } else {
            bytes.as_ptr()
        },
        len: bytes.len(),
    }
}

fn empty_result() -> MermanNativeResult {
    MermanNativeResult {
        struct_size: native_struct_size::<MermanNativeResult>(),
        allocation_token: 0,
        status: 0,
        operation: 0,
        media_type: MermanNativeSlice {
            struct_size: 0,
            data: ptr::null(),
            len: 0,
        },
        data: MermanNativeBuffer {
            struct_size: 0,
            data: ptr::null_mut(),
            len: 0,
        },
        metadata_or_error_json: MermanNativeBuffer {
            struct_size: 0,
            data: ptr::null_mut(),
            len: 0,
        },
    }
}

fn native_struct_size<T>() -> u32 {
    u32::try_from(std::mem::size_of::<T>()).expect("native record sizes fit in u32")
}

fn discover_api() -> MermanNativeApi {
    let request = MermanNativeApiRequest {
        struct_size: native_struct_size::<MermanNativeApiRequest>(),
        expected_abi_version: MERMAN_NATIVE_ABI_VERSION,
        expected_minimum_prefix_layout_digest: borrowed_slice(
            MERMAN_NATIVE_ABI_MINIMUM_PREFIX_LAYOUT_DIGEST.as_bytes(),
        ),
    };
    let mut raw_api = MaybeUninit::<MermanNativeApi>::uninit();
    unsafe {
        ptr::addr_of_mut!((*raw_api.as_mut_ptr()).struct_size)
            .write(native_struct_size::<MermanNativeApi>());
    }
    assert_eq!(
        unsafe { merman_get_native_api(&request, raw_api.as_mut_ptr()) },
        MERMAN_NATIVE_STATUS_OK
    );
    let api = unsafe { raw_api.assume_init() };
    assert_eq!(api.abi_version, MERMAN_NATIVE_ABI_VERSION);
    assert_eq!(api.struct_size, native_struct_size::<MermanNativeApi>());
    assert!(api.runtime_catalog.is_some());
    assert!(api.engine_new.is_some());
    assert!(api.engine_try_close.is_some());
    assert!(api.execute_collect.is_some());
    assert!(api.result_free.is_some());
    assert!(api.metadata_collect.is_some());
    api
}

fn verify_discovery_rejects_a_stale_version() {
    let request = MermanNativeApiRequest {
        struct_size: native_struct_size::<MermanNativeApiRequest>(),
        expected_abi_version: MERMAN_NATIVE_ABI_VERSION.saturating_sub(1),
        expected_minimum_prefix_layout_digest: borrowed_slice(
            MERMAN_NATIVE_ABI_MINIMUM_PREFIX_LAYOUT_DIGEST.as_bytes(),
        ),
    };
    let mut raw_api = MaybeUninit::<MermanNativeApi>::uninit();
    unsafe {
        ptr::addr_of_mut!((*raw_api.as_mut_ptr()).struct_size)
            .write(native_struct_size::<MermanNativeApi>());
    }
    assert_eq!(
        unsafe { merman_get_native_api(&request, raw_api.as_mut_ptr()) },
        MERMAN_NATIVE_STATUS_ABI_MISMATCH
    );
}

fn execute_operation(api: &MermanNativeApi, input: FuzzInput<'_>, selector: u8) {
    let with_text_callback = selector == 13;
    let options_in_engine = selector == 11;
    let config = MermanNativeEngineConfig {
        struct_size: native_struct_size::<MermanNativeEngineConfig>(),
        options_json: borrowed_slice(if options_in_engine {
            input.options
        } else {
            &[]
        }),
        text_measure: with_text_callback.then_some(fuzz_measure_text),
        text_measure_user_data: ptr::null_mut(),
    };
    let mut engine = 0;
    let mut result = empty_result();
    let status = unsafe { api.engine_new.unwrap()(&config, &mut engine, &mut result) };
    consume_result(api, &mut result, status);
    if status != MERMAN_NATIVE_STATUS_OK {
        assert_eq!(engine, 0);
        return;
    }
    assert_ne!(engine, 0);

    let (operation, needs_uri) = operation_for_selector(selector);
    let request = MermanNativeOperationRequest {
        struct_size: native_struct_size::<MermanNativeOperationRequest>(),
        operation,
        source: borrowed_slice(input.source),
        uri: borrowed_slice(if needs_uri { input.uri } else { &[] }),
        options_json: borrowed_slice(if options_in_engine {
            &[]
        } else {
            input.options
        }),
    };
    let mut output = empty_result();
    let output_status = unsafe { api.execute_collect.unwrap()(engine, &request, &mut output) };
    consume_result(api, &mut output, output_status);
    assert_eq!(
        unsafe { api.engine_try_close.unwrap()(engine) },
        MERMAN_NATIVE_STATUS_OK
    );
}

fn operation_for_selector(selector: u8) -> (MermanNativeOperationCode, bool) {
    match selector {
        0 => (MERMAN_NATIVE_OPERATION_SEMANTIC_JSON, false),
        1 => (MERMAN_NATIVE_OPERATION_VALIDATION_JSON, false),
        2 => (MERMAN_NATIVE_OPERATION_ANALYSIS_JSON, false),
        3 => (MERMAN_NATIVE_OPERATION_LAYOUT_JSON, false),
        4 | 11 | 13 => (MERMAN_NATIVE_OPERATION_SVG, false),
        5 => (MERMAN_NATIVE_OPERATION_ASCII, false),
        6 => (MERMAN_NATIVE_OPERATION_DOCUMENT_ANALYSIS_JSON, true),
        7 => (MERMAN_NATIVE_OPERATION_DOCUMENT_ANALYSIS_FACTS_JSON, true),
        8 => (MERMAN_NATIVE_OPERATION_PNG, false),
        9 => (MERMAN_NATIVE_OPERATION_JPEG, false),
        10 => (MERMAN_NATIVE_OPERATION_PDF, false),
        14 => (i32::MAX, false),
        15 => (MERMAN_NATIVE_OPERATION_SVG_PLAN_JSON, false),
        16 => (MERMAN_NATIVE_OPERATION_ANALYSIS_FACTS_JSON, false),
        _ => (MERMAN_NATIVE_OPERATION_SEMANTIC_JSON, false),
    }
}

fn consume_runtime_catalog(api: &MermanNativeApi) {
    let mut result = empty_result();
    let status = unsafe { api.runtime_catalog.unwrap()(&mut result) };
    consume_result(api, &mut result, status);
}

fn consume_metadata(api: &MermanNativeApi, metadata_id: &[u8]) {
    let mut result = empty_result();
    let status = unsafe {
        api.metadata_collect.unwrap()(borrowed_slice(metadata_id), &mut result)
    };
    consume_result(api, &mut result, status);
}

fn consume_result(
    api: &MermanNativeApi,
    result: &mut MermanNativeResult,
    status: MermanNativeStatus,
) {
    assert_ne!(
        status, MERMAN_NATIVE_STATUS_PANIC,
        "native ABI crossed panic boundary"
    );
    assert_eq!(result.status, status);
    assert_eq!(
        result.struct_size,
        native_struct_size::<MermanNativeResult>()
    );
    assert_ne!(result.allocation_token, 0);
    assert_native_buffer(result.data);
    assert_native_buffer(result.metadata_or_error_json);
    unsafe { api.result_free.unwrap()(result) };
    assert_eq!(result.allocation_token, 0);
    assert!(result.data.data.is_null());
    assert!(result.metadata_or_error_json.data.is_null());
}

fn assert_native_buffer(buffer: MermanNativeBuffer) {
    assert_eq!(
        buffer.struct_size,
        native_struct_size::<MermanNativeBuffer>()
    );
    if buffer.len == 0 {
        assert!(buffer.data.is_null(), "empty native buffer had data");
    } else {
        assert!(
            !buffer.data.is_null(),
            "non-empty native buffer had null data"
        );
        let bytes = unsafe { std::slice::from_raw_parts(buffer.data, buffer.len) };
        std::hint::black_box(bytes);
    }
}

unsafe extern "C" fn fuzz_measure_text(
    request: *const MermanNativeTextMeasureRequest,
    out_result: *mut MermanNativeTextMeasureResult,
    _user_data: *mut std::ffi::c_void,
) -> MermanNativeStatus {
    assert!(
        !request.is_null(),
        "native text measurement received null request"
    );
    assert!(
        !out_result.is_null(),
        "native text measurement received null result"
    );
    let request = unsafe { &*request };
    assert_eq!(
        request.struct_size,
        native_struct_size::<MermanNativeTextMeasureRequest>()
    );
    assert_native_slice(request.text);
    assert_native_slice(request.font_family);
    assert_native_slice(request.font_weight);
    assert_native_slice(request.font_style);

    unsafe {
        ptr::write(
            out_result,
            MermanNativeTextMeasureResult {
                struct_size: native_struct_size::<MermanNativeTextMeasureResult>(),
                handled: 0,
                has_raw_width: 0,
                result_kind: MERMAN_TEXT_MEASUREMENT_RESULT_KIND_METRICS,
                width: 0.0,
                height: 0.0,
                length: 0.0,
                bbox_left: 0.0,
                bbox_right: 0.0,
                raw_width: 0.0,
                line_count: 0,
            },
        );
    }
    MERMAN_NATIVE_STATUS_OK
}

fn assert_native_slice(slice: MermanNativeSlice) {
    assert_eq!(slice.struct_size, native_struct_size::<MermanNativeSlice>());
    if slice.len == 0 {
        assert!(slice.data.is_null(), "empty native slice had data");
    } else {
        assert!(
            !slice.data.is_null(),
            "non-empty native slice had null data"
        );
        let bytes = unsafe { std::slice::from_raw_parts(slice.data, slice.len) };
        std::hint::black_box(bytes);
    }
}
