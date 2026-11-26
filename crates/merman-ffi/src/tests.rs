use super::*;
use std::collections::BTreeSet;
use std::panic::{AssertUnwindSafe, catch_unwind};

#[repr(C)]
struct StructSizeOnly {
    struct_size: u32,
}

#[repr(C, align(16))]
#[cfg(feature = "svg")]
struct AlignedStructSizeOnly {
    struct_size: u32,
}

fn empty_api() -> MermanNativeApi {
    MermanNativeApi {
        struct_size: native_struct_size::<MermanNativeApi>(),
        abi_version: 0,
        minimum_prefix_layout_digest: static_slice(&[]),
        full_descriptor_digest: static_slice(&[]),
        capability_catalog_digest: static_slice(&[]),
        package_version: static_slice(&[]),
        runtime_catalog: None,
        engine_new: None,
        engine_try_close: None,
        execute_collect: None,
        result_free: None,
        metadata_collect: None,
        engine_new_with_services: None,
        operation_control_new: None,
        operation_control_cancel: None,
        operation_control_release: None,
        execute_collect_controlled: None,
        engine_new_with_fonts: None,
    }
}

fn api_table() -> MermanNativeApi {
    let mut api = empty_api();
    let request = MermanNativeApiRequest {
        struct_size: native_struct_size::<MermanNativeApiRequest>(),
        expected_abi_version: MERMAN_NATIVE_ABI_VERSION,
        expected_minimum_prefix_layout_digest: borrowed_slice(
            MERMAN_NATIVE_ABI_MINIMUM_PREFIX_LAYOUT_DIGEST.as_bytes(),
        ),
    };
    let status = unsafe { merman_get_native_api(&request, &mut api) };
    assert_eq!(status, MERMAN_NATIVE_STATUS_OK);
    api
}

fn native_result() -> MermanNativeResult {
    initialized_native_result()
}

fn result_allocation_is_live(token: u64) -> bool {
    ALLOCATION_REGISTRY
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
        .get(token >> MERMAN_NATIVE_TOKEN_COUNTER_SHIFT)
        .is_some()
}

fn overflowing_native_slice() -> MermanNativeSlice {
    MermanNativeSlice {
        struct_size: native_struct_size::<MermanNativeSlice>(),
        data: ptr::without_provenance(usize::MAX - 1),
        len: 4,
    }
}

fn misaligned_record<T>(value: T) -> (Vec<u8>, *mut T) {
    assert!(align_of::<T>() > 1, "test record must require alignment");
    let mut storage = vec![0_u8; size_of::<T>() + align_of::<T>()];
    let offset = (0..align_of::<T>())
        .find(|offset| !(storage.as_ptr() as usize + offset).is_multiple_of(align_of::<T>()))
        .expect("an offset within one alignment quantum must be misaligned");
    let record = unsafe { storage.as_mut_ptr().add(offset).cast::<T>() };
    unsafe { ptr::write_unaligned(record, value) };
    (storage, record)
}

fn result_json(result: &MermanNativeResult) -> serde_json::Value {
    let bytes = if result.metadata_or_error_json.data.is_null() {
        &[]
    } else {
        unsafe {
            std::slice::from_raw_parts(
                result.metadata_or_error_json.data,
                result.metadata_or_error_json.len,
            )
        }
    };
    serde_json::from_slice(bytes).expect("native result metadata must be valid JSON")
}

fn assert_constructor_failure_left_result_unwritten(
    api: &MermanNativeApi,
    result: &mut MermanNativeResult,
) {
    let result_was_writable = unsafe { result_is_writable(result) };

    unsafe { api.result_free.unwrap()(result) };

    assert!(
        result_was_writable.is_ok(),
        "constructor failure must leave out_result initialized and unwritten"
    );
}

fn assert_structured_invalid_out_engine_failure(
    api: &MermanNativeApi,
    result: &mut MermanNativeResult,
    expected_message: &str,
) {
    assert_eq!(result.status, MERMAN_NATIVE_STATUS_INVALID_ARGUMENT);
    assert_ne!(result.allocation_token, 0);
    let failure = result_json(result);
    assert_eq!(
        failure["status"].as_i64(),
        Some(i64::from(MERMAN_NATIVE_STATUS_INVALID_ARGUMENT))
    );
    assert_eq!(failure["message"], expected_message);

    unsafe { api.result_free.unwrap()(result) };
    assert_eq!(result.allocation_token, 0);
}

#[test]
fn native_failure_stays_below_large_result_threshold() {
    assert!(size_of::<NativeFailure>() < 128);
}

#[test]
fn native_error_json_preserves_structured_resource_details() {
    let failure = native_failure_from_binding(BindingError::resource_limit(
        "embedded_image_decode",
        "max_embedded_image_bytes",
        5,
        4,
        "constrained",
        "embedded image is too large",
    ));
    let payload: serde_json::Value =
        serde_json::from_slice(&native_error_json(&failure)).expect("native error JSON");

    assert_eq!(payload["status_name"], "resource-limit-exceeded");
    assert_eq!(
        payload["details"]["resource"]["limit_id"],
        "max_embedded_image_bytes"
    );
    assert_eq!(
        payload["details"]["resource"]["phase"],
        "embedded_image_decode"
    );
    assert_eq!(payload["details"]["resource"]["actual"], 5);
    assert_eq!(payload["details"]["resource"]["max"], 4);
    assert_eq!(payload["details"]["resource"]["profile"], "constrained");
    assert_eq!(payload["details"]["resource"]["cause"], "ceiling");

    let failure = native_failure_from_binding(BindingError::resource_limit_with_cause(
        merman_bindings_core::BindingResourceLimitCause::ArithmeticOverflow,
        "layout_model",
        "max_layout_work_units",
        u64::MAX,
        800_000,
        "interactive",
        "layout work accounting overflowed",
    ));
    let payload: serde_json::Value =
        serde_json::from_slice(&native_error_json(&failure)).expect("native error JSON");
    assert_eq!(
        payload["details"]["resource"]["cause"],
        "arithmetic_overflow"
    );
    assert_eq!(
        payload["details"]["resource"]["actual"],
        u64::MAX.to_string()
    );
    assert_eq!(payload["details"]["resource"]["max"], 800_000);
}

#[test]
fn native_error_json_preserves_structured_diagnostic_details() {
    let error = BindingError::new(BindingStatus::ParseError, "invalid edge")
        .with_diagnostic_details(
            merman_bindings_core::BindingDiagnosticErrorDetails::new("flowchart.edge.invalid")
                .with_span(merman_bindings_core::BindingDiagnosticSpan::new(
                    3, 8, "exact",
                ))
                .with_field("edge")
                .with_diagram_type("flowchart"),
        );
    let failure = native_failure_from_binding(error);
    let payload: serde_json::Value =
        serde_json::from_slice(&native_error_json(&failure)).expect("native error JSON");

    assert_eq!(
        payload["details"]["diagnostic"]["code"],
        "flowchart.edge.invalid"
    );
    assert_eq!(payload["details"]["diagnostic"]["span"]["start"], 3);
    assert_eq!(payload["details"]["diagnostic"]["span"]["end"], 8);
    assert_eq!(payload["details"]["diagnostic"]["span"]["kind"], "exact");
    assert_eq!(payload["details"]["diagnostic"]["field"], "edge");
    assert_eq!(
        payload["details"]["diagnostic"]["diagram_type"],
        "flowchart"
    );
}

fn native_config() -> MermanNativeEngineConfig {
    MermanNativeEngineConfig {
        struct_size: native_struct_size::<MermanNativeEngineConfig>(),
        options_json: borrowed_slice(&[]),
        text_measure: None,
        text_measure_user_data: ptr::null_mut(),
    }
}

#[cfg(feature = "svg")]
fn native_icon_pack(json: &[u8], registration_name: &[u8]) -> MermanNativeIconPack {
    MermanNativeIconPack {
        struct_size: native_struct_size::<MermanNativeIconPack>(),
        json: borrowed_slice(json),
        registration_name: borrowed_slice(registration_name),
    }
}

fn native_services_config(
    engine_config: MermanNativeEngineConfig,
    icon_packs: &[MermanNativeIconPack],
) -> MermanNativeEngineServicesConfig {
    MermanNativeEngineServicesConfig {
        struct_size: native_struct_size::<MermanNativeEngineServicesConfig>(),
        engine_config,
        icon_packs: if icon_packs.is_empty() {
            ptr::null()
        } else {
            icon_packs.as_ptr()
        },
        icon_pack_count: icon_packs.len(),
    }
}

fn native_request(
    operation: MermanNativeOperationCode,
    source: &[u8],
) -> MermanNativeOperationRequest {
    native_request_with_options(operation, source, &[])
}

fn native_request_with_options(
    operation: MermanNativeOperationCode,
    source: &[u8],
    options_json: &[u8],
) -> MermanNativeOperationRequest {
    MermanNativeOperationRequest {
        struct_size: native_struct_size::<MermanNativeOperationRequest>(),
        operation,
        source: borrowed_slice(source),
        uri: borrowed_slice(&[]),
        options_json: borrowed_slice(options_json),
    }
}

#[cfg(feature = "svg")]
struct ReentrantTextMeasureContext {
    token: MermanNativeEngineToken,
    other_token: MermanNativeEngineToken,
    nested_status: MermanNativeStatus,
    nested_result_status: MermanNativeStatus,
    free_status: MermanNativeStatus,
    other_engine_status: MermanNativeStatus,
    nested_error_was_typed: bool,
}

#[cfg(feature = "svg")]
struct ConcurrentFreeTextMeasureContext {
    blocked_once: std::sync::atomic::AtomicBool,
    entered: std::sync::Barrier,
    proceed: std::sync::Barrier,
}

#[cfg(feature = "svg")]
struct CrossThreadReentrantTextMeasureContext {
    token: MermanNativeEngineToken,
    nested_status: MermanNativeStatus,
}

#[cfg(feature = "svg")]
struct CountingTextMeasureContext {
    calls: std::sync::atomic::AtomicUsize,
}

#[cfg(feature = "svg")]
unsafe extern "C" fn counting_text_measure_callback(
    _request: *const MermanNativeTextMeasureRequest,
    out_result: *mut MermanNativeTextMeasureResult,
    user_data: *mut std::ffi::c_void,
) -> MermanNativeStatus {
    let context = unsafe { &*(user_data.cast::<CountingTextMeasureContext>()) };
    context
        .calls
        .fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    if !out_result.is_null() {
        unsafe { (*out_result).handled = 0 };
    }
    MERMAN_NATIVE_STATUS_OK
}

#[cfg(feature = "svg")]
unsafe extern "C" fn reentrant_text_measure_callback(
    _request: *const MermanNativeTextMeasureRequest,
    out_result: *mut MermanNativeTextMeasureResult,
    user_data: *mut std::ffi::c_void,
) -> MermanNativeStatus {
    let context = unsafe { &mut *(user_data.cast::<ReentrantTextMeasureContext>()) };
    let nested_request = native_request(
        MERMAN_NATIVE_OPERATION_SEMANTIC_JSON,
        b"flowchart TD\nNested --> Call",
    );
    let mut nested_result = native_result();
    context.nested_status =
        unsafe { native_execute_collect(context.token, &nested_request, &mut nested_result) };
    context.nested_result_status = nested_result.status;
    let error_json = if nested_result.metadata_or_error_json.data.is_null() {
        &[]
    } else {
        unsafe {
            std::slice::from_raw_parts(
                nested_result.metadata_or_error_json.data,
                nested_result.metadata_or_error_json.len,
            )
        }
    };
    context.nested_error_was_typed = error_json
        .windows(b"\"status_name\":\"reentrant-call\"".len())
        .any(|window| window == b"\"status_name\":\"reentrant-call\"");
    unsafe { native_result_free(&mut nested_result) };
    context.free_status = unsafe { native_engine_try_close(context.token) };

    let mut other_result = native_result();
    context.other_engine_status =
        unsafe { native_execute_collect(context.other_token, &nested_request, &mut other_result) };
    unsafe { native_result_free(&mut other_result) };

    if !out_result.is_null() {
        unsafe { (*out_result).handled = 0 };
    }
    MERMAN_NATIVE_STATUS_OK
}

#[cfg(feature = "svg")]
unsafe extern "C" fn concurrent_free_text_measure_callback(
    _request: *const MermanNativeTextMeasureRequest,
    out_result: *mut MermanNativeTextMeasureResult,
    user_data: *mut std::ffi::c_void,
) -> MermanNativeStatus {
    let context = unsafe { &*(user_data.cast::<ConcurrentFreeTextMeasureContext>()) };
    if !context
        .blocked_once
        .swap(true, std::sync::atomic::Ordering::AcqRel)
    {
        context.entered.wait();
        context.proceed.wait();
    }
    if !out_result.is_null() {
        unsafe { (*out_result).handled = 0 };
    }
    MERMAN_NATIVE_STATUS_OK
}

#[cfg(feature = "svg")]
unsafe extern "C" fn cross_thread_reentrant_text_measure_callback(
    _request: *const MermanNativeTextMeasureRequest,
    out_result: *mut MermanNativeTextMeasureResult,
    user_data: *mut std::ffi::c_void,
) -> MermanNativeStatus {
    let context = unsafe { &mut *(user_data.cast::<CrossThreadReentrantTextMeasureContext>()) };
    let token = context.token;
    context.nested_status = std::thread::spawn(move || {
        let request = native_request(
            MERMAN_NATIVE_OPERATION_SEMANTIC_JSON,
            b"flowchart TD\nNested --> Call",
        );
        let mut nested_result = native_result();
        let status = unsafe { native_execute_collect(token, &request, &mut nested_result) };
        unsafe { native_result_free(&mut nested_result) };
        status
    })
    .join()
    .expect("cross-thread reentrant callback task");

    if !out_result.is_null() {
        unsafe { (*out_result).handled = 0 };
    }
    MERMAN_NATIVE_STATUS_OK
}

#[test]
fn api_discovery_requires_exact_version_and_digest() {
    let mut api = empty_api();
    // The released v0.11.0 header requests ABI 3 with this minimum-prefix digest.
    let published_abi3 = MermanNativeApiRequest {
        struct_size: native_struct_size::<MermanNativeApiRequest>(),
        expected_abi_version: 3,
        expected_minimum_prefix_layout_digest: borrowed_slice(
            b"sha256:623c099f91282a88bf4d4e9cc7cdf728fc39c3b71a3ae7392007dd74f2b6ab41",
        ),
    };
    assert_eq!(
        unsafe { merman_get_native_api(&published_abi3, &mut api) },
        MERMAN_NATIVE_STATUS_ABI_MISMATCH
    );
    assert_eq!(api.abi_version, 0);
    assert!(api.runtime_catalog.is_none());

    let wrong_digest = MermanNativeApiRequest {
        struct_size: native_struct_size::<MermanNativeApiRequest>(),
        expected_abi_version: MERMAN_NATIVE_ABI_VERSION,
        expected_minimum_prefix_layout_digest: borrowed_slice(b"sha256:wrong"),
    };
    assert_eq!(
        unsafe { merman_get_native_api(&wrong_digest, &mut api) },
        MERMAN_NATIVE_STATUS_ABI_LAYOUT_MISMATCH
    );

    let api = api_table();
    assert_eq!(api.abi_version, MERMAN_NATIVE_ABI_VERSION);
    assert!(api.engine_new.is_some());
    assert!(api.execute_collect.is_some());
    assert!(api.result_free.is_some());
    assert!(api.metadata_collect.is_some());
    assert!(api.engine_new_with_services.is_some());
    assert!(api.operation_control_new.is_some());
    assert!(api.operation_control_cancel.is_some());
    assert!(api.operation_control_release.is_some());
    assert!(api.execute_collect_controlled.is_some());
    assert!(api.engine_new_with_fonts.is_some());
}

#[test]
fn operation_request_preserves_the_frozen_abi3_layout() {
    #[repr(C)]
    struct FrozenAbi3OperationRequest {
        struct_size: u32,
        operation: MermanNativeOperationCode,
        source: MermanNativeSlice,
        uri: MermanNativeSlice,
        options_json: MermanNativeSlice,
    }

    assert_eq!(
        std::mem::size_of::<MermanNativeOperationRequest>(),
        std::mem::size_of::<FrozenAbi3OperationRequest>()
    );
    assert_eq!(
        std::mem::align_of::<MermanNativeOperationRequest>(),
        std::mem::align_of::<FrozenAbi3OperationRequest>()
    );
    assert_eq!(
        std::mem::offset_of!(MermanNativeOperationRequest, struct_size),
        std::mem::offset_of!(FrozenAbi3OperationRequest, struct_size)
    );
    assert_eq!(
        std::mem::offset_of!(MermanNativeOperationRequest, operation),
        std::mem::offset_of!(FrozenAbi3OperationRequest, operation)
    );
    assert_eq!(
        std::mem::offset_of!(MermanNativeOperationRequest, source),
        std::mem::offset_of!(FrozenAbi3OperationRequest, source)
    );
    assert_eq!(
        std::mem::offset_of!(MermanNativeOperationRequest, uri),
        std::mem::offset_of!(FrozenAbi3OperationRequest, uri)
    );
    assert_eq!(
        std::mem::offset_of!(MermanNativeOperationRequest, options_json),
        std::mem::offset_of!(FrozenAbi3OperationRequest, options_json)
    );
}

#[test]
fn discovery_requires_the_current_table_and_preserves_tail_storage() {
    #[repr(C)]
    struct ExtendedApiBuffer {
        api: MermanNativeApi,
        suffix: [u8; 32],
    }

    let request = MermanNativeApiRequest {
        struct_size: native_struct_size::<MermanNativeApiRequest>(),
        expected_abi_version: MERMAN_NATIVE_ABI_VERSION,
        expected_minimum_prefix_layout_digest: borrowed_slice(
            MERMAN_NATIVE_ABI_MINIMUM_PREFIX_LAYOUT_DIGEST.as_bytes(),
        ),
    };
    let mut buffer = ExtendedApiBuffer {
        api: empty_api(),
        suffix: [0xa5; 32],
    };
    buffer.api.struct_size = native_struct_size::<ExtendedApiBuffer>();

    assert_eq!(
        unsafe { merman_get_native_api(&request, &mut buffer.api) },
        MERMAN_NATIVE_STATUS_OK
    );
    assert_eq!(
        buffer.api.struct_size,
        native_struct_size::<MermanNativeApi>()
    );
    assert!(MERMAN_NATIVE_API_MINIMUM_PREFIX_SIZE < native_struct_size::<MermanNativeApi>());
    assert_eq!(
        MERMAN_NATIVE_API_ENGINE_NEW_WITH_FONTS_PREFIX_SIZE,
        native_struct_size::<MermanNativeApi>()
    );
    assert!(buffer.api.metadata_collect.is_some());
    assert!(buffer.api.engine_new_with_services.is_some());
    assert!(buffer.api.engine_new_with_fonts.is_some());
    assert_eq!(buffer.suffix, [0xa5; 32]);
    let prefix_digest = unsafe {
        std::slice::from_raw_parts(
            buffer.api.minimum_prefix_layout_digest.data,
            buffer.api.minimum_prefix_layout_digest.len,
        )
    };
    let full_digest = unsafe {
        std::slice::from_raw_parts(
            buffer.api.full_descriptor_digest.data,
            buffer.api.full_descriptor_digest.len,
        )
    };
    assert_eq!(
        prefix_digest,
        MERMAN_NATIVE_ABI_MINIMUM_PREFIX_LAYOUT_DIGEST.as_bytes()
    );
    assert_eq!(
        full_digest,
        MERMAN_NATIVE_ABI_FULL_DESCRIPTOR_DIGEST.as_bytes()
    );

    assert_eq!(
        MERMAN_NATIVE_API_COMPLETE_PREFIX_SIZES,
        &[
            MERMAN_NATIVE_API_MINIMUM_PREFIX_SIZE,
            MERMAN_NATIVE_API_OPERATION_CONTROL_NEW_PREFIX_SIZE,
            MERMAN_NATIVE_API_OPERATION_CONTROL_CANCEL_PREFIX_SIZE,
            MERMAN_NATIVE_API_OPERATION_CONTROL_RELEASE_PREFIX_SIZE,
            MERMAN_NATIVE_API_EXECUTE_COLLECT_CONTROLLED_PREFIX_SIZE,
            MERMAN_NATIVE_API_ENGINE_NEW_WITH_FONTS_PREFIX_SIZE,
        ]
    );

    // The returned table size is itself safe input capacity for rediscovery.
    buffer.api.struct_size = MERMAN_NATIVE_API_MINIMUM_PREFIX_SIZE;
    assert_eq!(
        unsafe { merman_get_native_api(&request, &mut buffer.api) },
        MERMAN_NATIVE_STATUS_OK
    );
    assert_eq!(
        buffer.api.struct_size,
        MERMAN_NATIVE_API_MINIMUM_PREFIX_SIZE
    );
    assert_eq!(buffer.suffix, [0xa5; 32]);

    let mut undersized = ExtendedApiBuffer {
        api: empty_api(),
        suffix: [0xa5; 32],
    };
    undersized.api.struct_size = MERMAN_NATIVE_API_MINIMUM_PREFIX_SIZE - 1;
    assert_eq!(
        unsafe { merman_get_native_api(&request, &mut undersized.api) },
        MERMAN_NATIVE_STATUS_INVALID_ARGUMENT
    );
    assert_eq!(
        undersized.api.struct_size,
        MERMAN_NATIVE_API_MINIMUM_PREFIX_SIZE - 1
    );
    assert!(undersized.api.runtime_catalog.is_none());
    assert!(undersized.api.engine_new_with_services.is_none());
    assert_eq!(undersized.suffix, [0xa5; 32]);
}

#[test]
fn operation_control_tokens_cancel_release_and_preserve_structured_failure() {
    let api = api_table();
    let mut control = 0;
    let mut control_result = native_result();
    assert_eq!(
        unsafe { api.operation_control_new.unwrap()(0, 0, &mut control, &mut control_result) },
        MERMAN_NATIVE_STATUS_OK
    );
    assert!(token_has_domain(
        control,
        MERMAN_NATIVE_OPERATION_CONTROL_TOKEN_DOMAIN_TAG
    ));
    assert_eq!(control_result.operation, MERMAN_NATIVE_OPERATION_NONE);
    assert_eq!(control_result.data.len, 0);
    unsafe { api.result_free.unwrap()(&mut control_result) };

    assert_eq!(
        unsafe { api.operation_control_cancel.unwrap()(control) },
        MERMAN_NATIVE_STATUS_OK
    );

    let mut engine = 0;
    let mut engine_result = native_result();
    assert_eq!(
        unsafe { api.engine_new.unwrap()(&native_config(), &mut engine, &mut engine_result) },
        MERMAN_NATIVE_STATUS_OK
    );
    unsafe { api.result_free.unwrap()(&mut engine_result) };

    let request = native_request(
        MERMAN_NATIVE_OPERATION_SEMANTIC_JSON,
        b"flowchart TD\nCancelled --> Request",
    );
    let mut result = native_result();
    assert_eq!(
        unsafe { api.execute_collect_controlled.unwrap()(engine, control, &request, &mut result) },
        MERMAN_NATIVE_STATUS_CANCELLED
    );
    assert_eq!(result.status, MERMAN_NATIVE_STATUS_CANCELLED);
    assert_eq!(
        result.data.len, 0,
        "cancelled requests publish no partial output"
    );
    let failure: serde_json::Value = serde_json::from_slice(unsafe {
        std::slice::from_raw_parts(
            result.metadata_or_error_json.data,
            result.metadata_or_error_json.len,
        )
    })
    .expect("cancelled native error JSON");
    assert_eq!(failure["details"]["cancellation"]["reason"], "requested");
    assert!(failure["details"]["cancellation"]["phase"].is_string());
    unsafe { api.result_free.unwrap()(&mut result) };

    assert_eq!(
        unsafe { api.engine_try_close.unwrap()(engine) },
        MERMAN_NATIVE_STATUS_OK
    );
    assert_eq!(
        unsafe { api.operation_control_release.unwrap()(control) },
        MERMAN_NATIVE_STATUS_OK
    );
    assert_eq!(
        unsafe { api.operation_control_cancel.unwrap()(control) },
        MERMAN_NATIVE_STATUS_INVALID_ARGUMENT
    );
    assert_eq!(
        unsafe { api.operation_control_release.unwrap()(control) },
        MERMAN_NATIVE_STATUS_INVALID_ARGUMENT
    );
}

#[test]
fn undersized_records_are_rejected_before_full_record_reads() {
    let mut prefix = StructSizeOnly {
        struct_size: native_struct_size::<StructSizeOnly>(),
    };
    let mut api = api_table();
    assert_eq!(
        unsafe {
            merman_get_native_api(
                (&prefix as *const StructSizeOnly).cast::<MermanNativeApiRequest>(),
                &mut api,
            )
        },
        MERMAN_NATIVE_STATUS_INVALID_ARGUMENT
    );

    let discovery = MermanNativeApiRequest {
        struct_size: native_struct_size::<MermanNativeApiRequest>(),
        expected_abi_version: MERMAN_NATIVE_ABI_VERSION,
        expected_minimum_prefix_layout_digest: borrowed_slice(
            MERMAN_NATIVE_ABI_MINIMUM_PREFIX_LAYOUT_DIGEST.as_bytes(),
        ),
    };
    assert_eq!(
        unsafe {
            merman_get_native_api(
                &discovery,
                (&mut prefix as *mut StructSizeOnly).cast::<MermanNativeApi>(),
            )
        },
        MERMAN_NATIVE_STATUS_INVALID_ARGUMENT
    );

    let mut result = native_result();
    let mut token = 0;
    assert_eq!(
        unsafe {
            api.engine_new.unwrap()(
                (&prefix as *const StructSizeOnly).cast::<MermanNativeEngineConfig>(),
                &mut token,
                &mut result,
            )
        },
        MERMAN_NATIVE_STATUS_INVALID_ARGUMENT
    );
    assert_eq!(token, 0);
    unsafe { api.result_free.unwrap()(&mut result) };

    assert_eq!(
        unsafe {
            api.engine_new_with_services.unwrap()(
                (&prefix as *const StructSizeOnly).cast::<MermanNativeEngineServicesConfig>(),
                &mut token,
                &mut result,
            )
        },
        MERMAN_NATIVE_STATUS_INVALID_ARGUMENT
    );
    assert_eq!(token, 0);
    assert_eq!(result.allocation_token, 0);

    assert_eq!(
        unsafe { api.engine_new.unwrap()(&native_config(), &mut token, &mut result) },
        MERMAN_NATIVE_STATUS_OK
    );
    unsafe { api.result_free.unwrap()(&mut result) };

    assert_eq!(
        unsafe {
            api.execute_collect.unwrap()(
                token,
                (&prefix as *const StructSizeOnly).cast::<MermanNativeOperationRequest>(),
                &mut result,
            )
        },
        MERMAN_NATIVE_STATUS_INVALID_ARGUMENT
    );
    unsafe { api.result_free.unwrap()(&mut result) };

    unsafe {
        api.result_free.unwrap()((&mut prefix as *mut StructSizeOnly).cast::<MermanNativeResult>())
    };
    assert_eq!(
        unsafe { api.engine_try_close.unwrap()(token) },
        MERMAN_NATIVE_STATUS_OK
    );
}

#[test]
fn safely_allocated_misaligned_records_are_rejected_before_typed_access() {
    let request = MermanNativeApiRequest {
        struct_size: native_struct_size::<MermanNativeApiRequest>(),
        expected_abi_version: MERMAN_NATIVE_ABI_VERSION,
        expected_minimum_prefix_layout_digest: borrowed_slice(
            MERMAN_NATIVE_ABI_MINIMUM_PREFIX_LAYOUT_DIGEST.as_bytes(),
        ),
    };
    let (_request_storage, request) = misaligned_record(request);
    let mut api = empty_api();
    assert_eq!(
        unsafe { merman_get_native_api(request, &mut api) },
        MERMAN_NATIVE_STATUS_INVALID_ARGUMENT
    );

    let request = MermanNativeApiRequest {
        struct_size: native_struct_size::<MermanNativeApiRequest>(),
        expected_abi_version: MERMAN_NATIVE_ABI_VERSION,
        expected_minimum_prefix_layout_digest: borrowed_slice(
            MERMAN_NATIVE_ABI_MINIMUM_PREFIX_LAYOUT_DIGEST.as_bytes(),
        ),
    };
    let (_api_storage, misaligned_api) = misaligned_record(empty_api());
    assert_eq!(
        unsafe { merman_get_native_api(&request, misaligned_api) },
        MERMAN_NATIVE_STATUS_INVALID_ARGUMENT
    );

    let api = api_table();
    let (_config_storage, config) = misaligned_record(native_config());
    let mut token = 0;
    let mut result = native_result();
    assert_eq!(
        unsafe { api.engine_new.unwrap()(config, &mut token, &mut result) },
        MERMAN_NATIVE_STATUS_INVALID_ARGUMENT
    );
    assert_eq!(token, 0);
    unsafe { api.result_free.unwrap()(&mut result) };

    let (_services_storage, services) =
        misaligned_record(native_services_config(native_config(), &[]));
    assert_eq!(
        unsafe { api.engine_new_with_services.unwrap()(services, &mut token, &mut result) },
        MERMAN_NATIVE_STATUS_INVALID_ARGUMENT
    );
    assert_eq!(token, 0);
    assert_eq!(result.allocation_token, 0);

    let (_result_storage, misaligned_result) = misaligned_record(native_result());
    assert_eq!(
        unsafe { api.runtime_catalog.unwrap()(misaligned_result) },
        MERMAN_NATIVE_STATUS_INVALID_ARGUMENT
    );

    let mut live_result = native_result();
    assert_eq!(
        unsafe {
            write_native_result(
                &mut live_result,
                MERMAN_NATIVE_STATUS_OK,
                MERMAN_NATIVE_OPERATION_SEMANTIC_JSON,
                Some("application/json"),
                b"misaligned owner".to_vec(),
                Vec::new(),
            )
        },
        MERMAN_NATIVE_STATUS_OK
    );
    let allocation_token = live_result.allocation_token;
    let (_live_storage, misaligned_live_result) =
        misaligned_record(unsafe { ptr::read(&live_result) });
    live_result = native_result();

    unsafe { api.result_free.unwrap()(misaligned_live_result) };
    assert!(
        result_allocation_is_live(allocation_token),
        "misaligned result_free must be a no-op rather than releasing an unreadable record"
    );

    let mut recovered = unsafe { ptr::read_unaligned(misaligned_live_result) };
    unsafe { api.result_free.unwrap()(&mut recovered) };
    assert!(!result_allocation_is_live(allocation_token));
    unsafe { api.result_free.unwrap()(&mut live_result) };
}

#[test]
fn native_slices_reject_lengths_larger_than_isize_max() {
    let mut slice = MermanNativeSlice {
        struct_size: native_struct_size::<MermanNativeSlice>(),
        data: ptr::NonNull::<u8>::dangling().as_ptr(),
        len: (isize::MAX as usize).saturating_add(1),
    };

    let failure = unsafe { native_slice_bytes(slice, "request.source") }
        .expect_err("oversized native slices must fail before constructing a Rust slice");
    assert_eq!(failure.status, MERMAN_NATIVE_STATUS_INVALID_ARGUMENT);
    assert_eq!(
        failure.message.as_ref(),
        "request.source.len must not exceed isize::MAX"
    );

    slice.struct_size = slice.struct_size.saturating_add(1);
    let failure = unsafe { native_slice_bytes(slice, "request.source") }
        .expect_err("oversized records must not silently negotiate a prefix");
    assert_eq!(failure.status, MERMAN_NATIVE_STATUS_INVALID_ARGUMENT);
    assert!(failure.message.contains("expected exactly"));
}

#[test]
fn native_entry_points_reject_wrapping_slice_ranges_before_dereference() {
    let failure = unsafe { native_slice_bytes(overflowing_native_slice(), "test.slice") }
        .expect_err("wrapping native slice address ranges must be rejected");
    assert_eq!(failure.status, MERMAN_NATIVE_STATUS_INVALID_ARGUMENT);
    assert_eq!(
        failure.message.as_ref(),
        "test.slice address range overflows usize"
    );

    let discovery_request = MermanNativeApiRequest {
        struct_size: native_struct_size::<MermanNativeApiRequest>(),
        expected_abi_version: MERMAN_NATIVE_ABI_VERSION,
        expected_minimum_prefix_layout_digest: overflowing_native_slice(),
    };
    let mut discovered = empty_api();
    assert_eq!(
        unsafe { merman_get_native_api(&discovery_request, &mut discovered) },
        MERMAN_NATIVE_STATUS_INVALID_ARGUMENT
    );

    let api = api_table();
    let mut metadata_result = native_result();
    assert_eq!(
        unsafe { api.metadata_collect.unwrap()(overflowing_native_slice(), &mut metadata_result) },
        MERMAN_NATIVE_STATUS_INVALID_ARGUMENT
    );
    unsafe { api.result_free.unwrap()(&mut metadata_result) };

    let mut config_result = native_result();
    let mut engine = 0;
    assert_eq!(
        unsafe { api.engine_new.unwrap()(&native_config(), &mut engine, &mut config_result) },
        MERMAN_NATIVE_STATUS_OK
    );
    unsafe { api.result_free.unwrap()(&mut config_result) };

    let mut request = native_request(
        MERMAN_NATIVE_OPERATION_SEMANTIC_JSON,
        b"flowchart TD\nA --> B",
    );
    request.source = overflowing_native_slice();
    let mut execute_result = native_result();
    assert_eq!(
        unsafe { api.execute_collect.unwrap()(engine, &request, &mut execute_result) },
        MERMAN_NATIVE_STATUS_INVALID_ARGUMENT
    );
    assert_eq!(execute_result.status, MERMAN_NATIVE_STATUS_INVALID_ARGUMENT);
    unsafe { api.result_free.unwrap()(&mut execute_result) };
    assert_eq!(
        unsafe { api.engine_try_close.unwrap()(engine) },
        MERMAN_NATIVE_STATUS_OK
    );
}

#[test]
fn engine_new_rejects_overlapping_output_and_option_storage() {
    let api = api_table();
    let config = native_config();
    let mut result = native_result();
    let overlapping_engine = ptr::addr_of_mut!(result.allocation_token);

    assert_eq!(
        unsafe { api.engine_new.unwrap()(&config, overlapping_engine, &mut result) },
        MERMAN_NATIVE_STATUS_INVALID_ARGUMENT
    );
    assert_eq!(result.allocation_token, 0);
    assert!(result.metadata_or_error_json.data.is_null());

    let mut token = 0;
    let mut result = native_result();
    let overlapping_config = ptr::addr_of!(result).cast::<MermanNativeEngineConfig>();
    assert_eq!(
        unsafe { api.engine_new.unwrap()(overlapping_config, &mut token, &mut result) },
        MERMAN_NATIVE_STATUS_INVALID_ARGUMENT
    );
    assert_eq!(token, 0);
    assert_eq!(result.allocation_token, 0);
    assert!(result.metadata_or_error_json.data.is_null());

    let mut token = 0;
    let mut result = native_result();
    let mut config = native_config();
    config.options_json = MermanNativeSlice {
        struct_size: native_struct_size::<MermanNativeSlice>(),
        data: ptr::addr_of!(result).cast::<u8>(),
        len: 1,
    };
    assert_eq!(
        unsafe { api.engine_new.unwrap()(&config, &mut token, &mut result) },
        MERMAN_NATIVE_STATUS_INVALID_ARGUMENT
    );
    assert_eq!(token, 0);
    assert_eq!(result.allocation_token, 0);
    assert!(result.metadata_or_error_json.data.is_null());

    let mut token = 0;
    let mut result = native_result();
    let mut config = native_config();
    config.options_json = MermanNativeSlice {
        struct_size: native_struct_size::<MermanNativeSlice>(),
        data: ptr::addr_of!(config).cast::<u8>(),
        len: 1,
    };
    assert_eq!(
        unsafe { api.engine_new.unwrap()(&config, &mut token, &mut result) },
        MERMAN_NATIVE_STATUS_INVALID_ARGUMENT
    );
    assert_eq!(token, 0);
    assert_eq!(result.allocation_token, 0);
    assert!(result.metadata_or_error_json.data.is_null());

    let mut config = native_config();
    let overlapping_engine =
        ptr::addr_of_mut!(config.text_measure_user_data).cast::<MermanNativeEngineToken>();
    let mut result = native_result();
    assert_eq!(
        unsafe { api.engine_new.unwrap()(ptr::addr_of!(config), overlapping_engine, &mut result) },
        MERMAN_NATIVE_STATUS_INVALID_ARGUMENT
    );
    assert!(config.text_measure_user_data.is_null());
    assert_eq!(result.allocation_token, 0);
    assert!(result.metadata_or_error_json.data.is_null());

    let mut token = 0;
    let mut result = native_result();
    let mut config = native_config();
    config.options_json = MermanNativeSlice {
        struct_size: native_struct_size::<MermanNativeSlice>(),
        data: ptr::addr_of!(token).cast::<u8>(),
        len: 1,
    };
    assert_eq!(
        unsafe { api.engine_new.unwrap()(&config, &mut token, &mut result) },
        MERMAN_NATIVE_STATUS_INVALID_ARGUMENT
    );
    assert_eq!(token, 0);
    assert_eq!(result.allocation_token, 0);
    assert!(result.metadata_or_error_json.data.is_null());
}

#[test]
fn engine_new_writes_structured_failure_for_null_out_engine() {
    let api = api_table();
    let config = native_config();
    let mut result = native_result();

    let status = unsafe { api.engine_new.unwrap()(&config, ptr::null_mut(), &mut result) };

    assert_eq!(status, MERMAN_NATIVE_STATUS_INVALID_ARGUMENT);
    assert_structured_invalid_out_engine_failure(&api, &mut result, "out_engine must not be null");
}

#[test]
fn engine_new_writes_structured_failure_for_misaligned_out_engine() {
    let api = api_table();
    let config = native_config();
    let (_engine_storage, out_engine) = misaligned_record(0 as MermanNativeEngineToken);
    let mut result = native_result();

    let status = unsafe { api.engine_new.unwrap()(&config, out_engine, &mut result) };

    assert_eq!(status, MERMAN_NATIVE_STATUS_INVALID_ARGUMENT);
    assert_structured_invalid_out_engine_failure(
        &api,
        &mut result,
        &format!(
            "out_engine must be aligned to {} bytes for its native record type",
            align_of::<MermanNativeEngineToken>()
        ),
    );
}

#[test]
fn engine_new_with_services_writes_structured_failure_for_null_out_engine() {
    let api = api_table();
    let config = native_services_config(native_config(), &[]);
    let mut result = native_result();

    let status =
        unsafe { api.engine_new_with_services.unwrap()(&config, ptr::null_mut(), &mut result) };

    assert_eq!(status, MERMAN_NATIVE_STATUS_INVALID_ARGUMENT);
    assert_structured_invalid_out_engine_failure(&api, &mut result, "out_engine must not be null");
}

#[test]
fn engine_new_with_services_writes_structured_failure_for_misaligned_out_engine() {
    let api = api_table();
    let config = native_services_config(native_config(), &[]);
    let (_engine_storage, out_engine) = misaligned_record(0 as MermanNativeEngineToken);
    let mut result = native_result();

    let status = unsafe { api.engine_new_with_services.unwrap()(&config, out_engine, &mut result) };

    assert_eq!(status, MERMAN_NATIVE_STATUS_INVALID_ARGUMENT);
    assert_structured_invalid_out_engine_failure(
        &api,
        &mut result,
        &format!(
            "out_engine must be aligned to {} bytes for its native record type",
            align_of::<MermanNativeEngineToken>()
        ),
    );
}

#[test]
fn engine_new_output_shape_failures_do_not_overwrite_aliased_options() {
    let api = api_table();

    let mut result = native_result();
    let mut config = native_config();
    config.options_json = MermanNativeSlice {
        struct_size: native_struct_size::<MermanNativeSlice>(),
        data: ptr::addr_of!(result).cast::<u8>(),
        len: 1,
    };
    assert_eq!(
        unsafe { api.engine_new.unwrap()(&config, ptr::null_mut(), &mut result) },
        MERMAN_NATIVE_STATUS_INVALID_ARGUMENT
    );
    assert_constructor_failure_left_result_unwritten(&api, &mut result);

    let mut result = native_result();
    let mut config = native_config();
    config.options_json = MermanNativeSlice {
        struct_size: native_struct_size::<MermanNativeSlice>(),
        data: ptr::addr_of!(result).cast::<u8>(),
        len: 1,
    };
    let (_engine_storage, out_engine) = misaligned_record(0 as MermanNativeEngineToken);
    assert_eq!(
        unsafe { api.engine_new.unwrap()(&config, out_engine, &mut result) },
        MERMAN_NATIVE_STATUS_INVALID_ARGUMENT
    );
    assert_constructor_failure_left_result_unwritten(&api, &mut result);
}

#[test]
fn service_constructor_output_shape_failures_do_not_overwrite_aliased_storage() {
    let api = api_table();

    let mut result = native_result();
    let mut config = native_services_config(native_config(), &[]);
    config.engine_config.options_json = MermanNativeSlice {
        struct_size: native_struct_size::<MermanNativeSlice>(),
        data: ptr::addr_of!(result).cast::<u8>(),
        len: 1,
    };
    assert_eq!(
        unsafe { api.engine_new_with_services.unwrap()(&config, ptr::null_mut(), &mut result) },
        MERMAN_NATIVE_STATUS_INVALID_ARGUMENT
    );
    assert_constructor_failure_left_result_unwritten(&api, &mut result);

    let mut result = native_result();
    let mut config = native_services_config(native_config(), &[]);
    config.engine_config.options_json = MermanNativeSlice {
        struct_size: native_struct_size::<MermanNativeSlice>(),
        data: ptr::addr_of!(result).cast::<u8>(),
        len: 1,
    };
    let (_engine_storage, out_engine) = misaligned_record(0 as MermanNativeEngineToken);
    assert_eq!(
        unsafe { api.engine_new_with_services.unwrap()(&config, out_engine, &mut result) },
        MERMAN_NATIVE_STATUS_INVALID_ARGUMENT
    );
    assert_constructor_failure_left_result_unwritten(&api, &mut result);

    let mut result = native_result();
    let mut config = native_services_config(native_config(), &[]);
    config.icon_packs = ptr::addr_of!(result).cast::<MermanNativeIconPack>();
    config.icon_pack_count = 1;
    assert_eq!(
        unsafe { api.engine_new_with_services.unwrap()(&config, ptr::null_mut(), &mut result) },
        MERMAN_NATIVE_STATUS_INVALID_ARGUMENT
    );
    assert_constructor_failure_left_result_unwritten(&api, &mut result);

    let mut result = native_result();
    let mut config = native_services_config(native_config(), &[]);
    config.icon_packs = ptr::addr_of!(result).cast::<MermanNativeIconPack>();
    config.icon_pack_count = 1;
    let (_engine_storage, out_engine) = misaligned_record(0 as MermanNativeEngineToken);
    assert_eq!(
        unsafe { api.engine_new_with_services.unwrap()(&config, out_engine, &mut result) },
        MERMAN_NATIVE_STATUS_INVALID_ARGUMENT
    );
    assert_constructor_failure_left_result_unwritten(&api, &mut result);
}

#[test]
fn constructor_output_shape_failures_leave_malformed_outer_records_unwritten() {
    let api = api_table();

    let mut config = native_config();
    config.struct_size -= 1;
    let mut result = native_result();
    assert_eq!(
        unsafe { api.engine_new.unwrap()(&config, ptr::null_mut(), &mut result) },
        MERMAN_NATIVE_STATUS_INVALID_ARGUMENT
    );
    assert_constructor_failure_left_result_unwritten(&api, &mut result);

    let mut result = native_result();
    let (_engine_storage, out_engine) = misaligned_record(0 as MermanNativeEngineToken);
    assert_eq!(
        unsafe { api.engine_new.unwrap()(&config, out_engine, &mut result) },
        MERMAN_NATIVE_STATUS_INVALID_ARGUMENT
    );
    assert_constructor_failure_left_result_unwritten(&api, &mut result);

    let mut config = native_services_config(native_config(), &[]);
    config.struct_size -= 1;
    let mut result = native_result();
    assert_eq!(
        unsafe { api.engine_new_with_services.unwrap()(&config, ptr::null_mut(), &mut result) },
        MERMAN_NATIVE_STATUS_INVALID_ARGUMENT
    );
    assert_constructor_failure_left_result_unwritten(&api, &mut result);

    let mut result = native_result();
    let (_engine_storage, out_engine) = misaligned_record(0 as MermanNativeEngineToken);
    assert_eq!(
        unsafe { api.engine_new_with_services.unwrap()(&config, out_engine, &mut result) },
        MERMAN_NATIVE_STATUS_INVALID_ARGUMENT
    );
    assert_constructor_failure_left_result_unwritten(&api, &mut result);
}

#[test]
fn service_constructor_output_shape_failures_precede_oversized_pack_counts() {
    let api = api_table();
    let mut config = native_services_config(native_config(), &[]);
    config.icon_pack_count = usize::MAX;

    let mut result = native_result();
    assert_eq!(
        unsafe { api.engine_new_with_services.unwrap()(&config, ptr::null_mut(), &mut result) },
        MERMAN_NATIVE_STATUS_INVALID_ARGUMENT
    );
    assert_structured_invalid_out_engine_failure(&api, &mut result, "out_engine must not be null");

    let mut result = native_result();
    let (_engine_storage, out_engine) = misaligned_record(0 as MermanNativeEngineToken);
    assert_eq!(
        unsafe { api.engine_new_with_services.unwrap()(&config, out_engine, &mut result) },
        MERMAN_NATIVE_STATUS_INVALID_ARGUMENT
    );
    assert_structured_invalid_out_engine_failure(
        &api,
        &mut result,
        &format!(
            "out_engine must be aligned to {} bytes for its native record type",
            align_of::<MermanNativeEngineToken>()
        ),
    );
}

#[cfg(feature = "svg")]
#[test]
fn service_constructor_output_shape_failures_do_not_overwrite_aliased_pack_slices() {
    let api = api_table();

    let mut result = native_result();
    let pack = MermanNativeIconPack {
        struct_size: native_struct_size::<MermanNativeIconPack>(),
        json: MermanNativeSlice {
            struct_size: native_struct_size::<MermanNativeSlice>(),
            data: ptr::addr_of!(result).cast::<u8>(),
            len: 1,
        },
        registration_name: borrowed_slice(&[]),
    };
    let config = native_services_config(native_config(), std::slice::from_ref(&pack));
    assert_eq!(
        unsafe { api.engine_new_with_services.unwrap()(&config, ptr::null_mut(), &mut result) },
        MERMAN_NATIVE_STATUS_INVALID_ARGUMENT
    );
    assert_constructor_failure_left_result_unwritten(&api, &mut result);

    let mut result = native_result();
    let pack = MermanNativeIconPack {
        struct_size: native_struct_size::<MermanNativeIconPack>(),
        json: borrowed_slice(br#"{"prefix":"test","icons":{}}"#),
        registration_name: MermanNativeSlice {
            struct_size: native_struct_size::<MermanNativeSlice>(),
            data: ptr::addr_of!(result).cast::<u8>(),
            len: 1,
        },
    };
    let config = native_services_config(native_config(), std::slice::from_ref(&pack));
    let (_engine_storage, out_engine) = misaligned_record(0 as MermanNativeEngineToken);
    assert_eq!(
        unsafe { api.engine_new_with_services.unwrap()(&config, out_engine, &mut result) },
        MERMAN_NATIVE_STATUS_INVALID_ARGUMENT
    );
    assert_constructor_failure_left_result_unwritten(&api, &mut result);
}

#[cfg(feature = "svg")]
#[test]
fn service_constructor_output_shape_failures_leave_malformed_pack_records_unwritten() {
    let api = api_table();
    let mut pack = native_icon_pack(br#"{"prefix":"test","icons":{}}"#, &[]);
    pack.struct_size -= 1;
    let config = native_services_config(native_config(), std::slice::from_ref(&pack));

    let mut result = native_result();
    assert_eq!(
        unsafe { api.engine_new_with_services.unwrap()(&config, ptr::null_mut(), &mut result) },
        MERMAN_NATIVE_STATUS_INVALID_ARGUMENT
    );
    assert_constructor_failure_left_result_unwritten(&api, &mut result);

    let mut result = native_result();
    let (_engine_storage, out_engine) = misaligned_record(0 as MermanNativeEngineToken);
    assert_eq!(
        unsafe { api.engine_new_with_services.unwrap()(&config, out_engine, &mut result) },
        MERMAN_NATIVE_STATUS_INVALID_ARGUMENT
    );
    assert_constructor_failure_left_result_unwritten(&api, &mut result);
}

#[test]
fn service_constructor_is_strict_while_basic_construction_remains_service_free() {
    let api = api_table();
    let orphan_user_data = ptr::NonNull::<u8>::dangling().as_ptr().cast();

    let mut basic_config = native_config();
    basic_config.text_measure_user_data = orphan_user_data;
    let mut basic_engine = 0;
    let mut basic_result = native_result();
    assert_eq!(
        unsafe { api.engine_new.unwrap()(&basic_config, &mut basic_engine, &mut basic_result) },
        MERMAN_NATIVE_STATUS_OK
    );
    assert_ne!(basic_engine, 0);
    unsafe { api.result_free.unwrap()(&mut basic_result) };
    assert_eq!(
        unsafe { api.engine_try_close.unwrap()(basic_engine) },
        MERMAN_NATIVE_STATUS_OK
    );

    let services_config = native_services_config(basic_config, &[]);
    let mut services_engine = 0;
    let mut services_result = native_result();
    assert_eq!(
        unsafe {
            api.engine_new_with_services.unwrap()(
                &services_config,
                &mut services_engine,
                &mut services_result,
            )
        },
        MERMAN_NATIVE_STATUS_INVALID_ARGUMENT
    );
    assert_eq!(services_engine, 0);
    assert_eq!(
        result_json(&services_result)["message"],
        "config.engine_config.text_measure_user_data must be null when text_measure is null"
    );
    unsafe { api.result_free.unwrap()(&mut services_result) };
}

#[test]
fn empty_service_constructor_uses_the_same_engine_contract() {
    let api = api_table();
    let config = native_services_config(native_config(), &[]);
    let mut engine = 0;
    let mut result = native_result();
    assert_eq!(
        unsafe { api.engine_new_with_services.unwrap()(&config, &mut engine, &mut result) },
        MERMAN_NATIVE_STATUS_OK
    );
    assert_ne!(engine, 0);
    assert_eq!(
        result_json(&result)["operation"],
        "engine-new-with-services"
    );
    unsafe { api.result_free.unwrap()(&mut result) };

    let request = native_request(
        MERMAN_NATIVE_OPERATION_SEMANTIC_JSON,
        b"flowchart TD\nA --> B",
    );
    assert_eq!(
        unsafe { api.execute_collect.unwrap()(engine, &request, &mut result) },
        MERMAN_NATIVE_STATUS_OK
    );
    unsafe { api.result_free.unwrap()(&mut result) };
    assert_eq!(
        unsafe { api.engine_try_close.unwrap()(engine) },
        MERMAN_NATIVE_STATUS_OK
    );
}

#[test]
fn icon_pack_array_arithmetic_is_checked_before_typed_access() {
    let failure =
        checked_record_array_byte_len::<MermanNativeIconPack>(usize::MAX, "config.icon_packs")
            .expect_err("record-array multiplication must not wrap");
    assert_eq!(failure.status, MERMAN_NATIVE_STATUS_INVALID_ARGUMENT);
    assert!(failure.message.contains("overflows usize"));

    let aligned_near_max = usize::MAX & !(align_of::<MermanNativeIconPack>() - 1);
    let failure = validate_record_array_shape::<MermanNativeIconPack>(
        ptr::without_provenance(aligned_near_max),
        1,
        "config.icon_packs",
    )
    .expect_err("record-array address ranges must not wrap");
    assert_eq!(failure.status, MERMAN_NATIVE_STATUS_INVALID_ARGUMENT);
    assert!(failure.message.contains("address range overflows usize"));
}

#[cfg(feature = "svg")]
#[test]
fn service_constructor_checks_the_fixed_pack_limit_before_reading_the_array() {
    let api = api_table();
    let mut config = native_services_config(native_config(), &[]);
    config.icon_packs = ptr::NonNull::<MermanNativeIconPack>::dangling().as_ptr();
    config.icon_pack_count = NATIVE_ICON_PACK_RECORD_LIMIT + 1;
    let mut engine = 0;
    let mut result = native_result();

    assert_eq!(
        unsafe { api.engine_new_with_services.unwrap()(&config, &mut engine, &mut result) },
        MERMAN_NATIVE_STATUS_RESOURCE_LIMIT_EXCEEDED
    );
    assert_eq!(engine, 0);
    let error = result_json(&result);
    assert_eq!(
        error["details"]["resource"]["limit_id"],
        IconRegistryResourceLimitId::MaxPacks.stable_id()
    );
    assert_eq!(
        error["details"]["resource"]["actual"],
        u64::try_from(NATIVE_ICON_PACK_RECORD_LIMIT + 1).unwrap()
    );
    assert_eq!(
        error["details"]["resource"]["max"],
        IconRegistryResourceLimitId::MaxPacks.fixed_value()
    );
    assert_eq!(
        error["details"]["icon_registry"]["kind_id"],
        "resource_limit_exceeded"
    );
    assert!(error["details"]["icon_registry"]["pack_index"].is_null());
    unsafe { api.result_free.unwrap()(&mut result) };
}

#[cfg(feature = "svg")]
#[test]
fn service_constructor_accepts_the_exact_pack_limit_and_rejects_malformed_records() {
    let api = api_table();
    let json = (0..NATIVE_ICON_PACK_RECORD_LIMIT)
        .map(|index| format!(r#"{{"prefix":"p{index}","icons":{{}}}}"#).into_bytes())
        .collect::<Vec<_>>();
    let packs = json
        .iter()
        .map(|json| native_icon_pack(json, &[]))
        .collect::<Vec<_>>();
    let config = native_services_config(native_config(), &packs);
    let mut engine = 0;
    let mut result = native_result();
    assert_eq!(
        unsafe { api.engine_new_with_services.unwrap()(&config, &mut engine, &mut result) },
        MERMAN_NATIVE_STATUS_OK
    );
    unsafe { api.result_free.unwrap()(&mut result) };
    assert_eq!(
        unsafe { api.engine_try_close.unwrap()(engine) },
        MERMAN_NATIVE_STATUS_OK
    );

    let mut malformed_nested = native_services_config(native_config(), &[]);
    malformed_nested.engine_config.struct_size -= 1;
    engine = 0;
    assert_eq!(
        unsafe {
            api.engine_new_with_services.unwrap()(&malformed_nested, &mut engine, &mut result)
        },
        MERMAN_NATIVE_STATUS_INVALID_ARGUMENT
    );
    assert_eq!(engine, 0);
    unsafe { api.result_free.unwrap()(&mut result) };

    let mut malformed_pack = native_icon_pack(br#"{"prefix":"test","icons":{}}"#, &[]);
    malformed_pack.struct_size -= 1;
    let config = native_services_config(native_config(), std::slice::from_ref(&malformed_pack));
    assert_eq!(
        unsafe { api.engine_new_with_services.unwrap()(&config, &mut engine, &mut result) },
        MERMAN_NATIVE_STATUS_INVALID_ARGUMENT
    );
    assert_eq!(engine, 0);
    unsafe { api.result_free.unwrap()(&mut result) };

    let pack_prefix = AlignedStructSizeOnly {
        struct_size: native_struct_size::<AlignedStructSizeOnly>(),
    };
    let config = MermanNativeEngineServicesConfig {
        struct_size: native_struct_size::<MermanNativeEngineServicesConfig>(),
        engine_config: native_config(),
        icon_packs: ptr::from_ref(&pack_prefix).cast::<MermanNativeIconPack>(),
        icon_pack_count: 1,
    };
    assert_eq!(
        unsafe { api.engine_new_with_services.unwrap()(&config, &mut engine, &mut result) },
        MERMAN_NATIVE_STATUS_INVALID_ARGUMENT
    );
    assert_eq!(engine, 0);
    unsafe { api.result_free.unwrap()(&mut result) };

    let pack = native_icon_pack(br#"{"prefix":"test","icons":{}}"#, &[]);
    let (_pack_storage, misaligned_pack) = misaligned_record(pack);
    let config = MermanNativeEngineServicesConfig {
        struct_size: native_struct_size::<MermanNativeEngineServicesConfig>(),
        engine_config: native_config(),
        icon_packs: misaligned_pack,
        icon_pack_count: 1,
    };
    assert_eq!(
        unsafe { api.engine_new_with_services.unwrap()(&config, &mut engine, &mut result) },
        MERMAN_NATIVE_STATUS_INVALID_ARGUMENT
    );
    assert_eq!(engine, 0);
    unsafe { api.result_free.unwrap()(&mut result) };

    let invalid_utf8_name = [0xff];
    let pack = native_icon_pack(br#"{"prefix":"test","icons":{}}"#, &invalid_utf8_name);
    let config = native_services_config(native_config(), std::slice::from_ref(&pack));
    assert_eq!(
        unsafe { api.engine_new_with_services.unwrap()(&config, &mut engine, &mut result) },
        MERMAN_NATIVE_STATUS_UTF8_ERROR
    );
    assert_eq!(engine, 0);
    let error = result_json(&result);
    assert_eq!(error["details"]["icon_registry"]["kind_id"], "invalid_utf8");
    assert_eq!(error["details"]["icon_registry"]["pack_index"], 0);
    unsafe { api.result_free.unwrap()(&mut result) };
}

#[cfg(feature = "svg")]
#[test]
fn service_constructor_allows_overlapping_read_only_pack_bytes_and_owns_the_registry() {
    let api = api_table();
    let context = Box::new(CountingTextMeasureContext {
        calls: std::sync::atomic::AtomicUsize::new(0),
    });
    let engine;
    {
        let shared_pack = br#"{
                "prefix":"ignored",
                "icons":{
                    "rocket":{"body":"<path data-icon=\"service-rocket\" d=\"M0 0H16V16H0z\"/>"},
                    "ship":{"body":"<circle data-icon=\"service-ship\" cx=\"8\" cy=\"8\" r=\"8\"/>"}
                }
            }"#
        .to_vec();
        let packs = [
            native_icon_pack(&shared_pack, b"alpha"),
            native_icon_pack(&shared_pack, b"fleet"),
        ];
        let mut engine_config = native_config();
        engine_config.text_measure = Some(counting_text_measure_callback);
        engine_config.text_measure_user_data = (&*context as *const CountingTextMeasureContext)
            .cast_mut()
            .cast();
        let config = native_services_config(engine_config, &packs);
        let mut candidate = 0;
        let mut result = native_result();
        assert_eq!(
            unsafe { api.engine_new_with_services.unwrap()(&config, &mut candidate, &mut result) },
            MERMAN_NATIVE_STATUS_OK
        );
        assert_eq!(
            context.calls.load(std::sync::atomic::Ordering::Relaxed),
            0,
            "construction must retain, but never invoke, the callback"
        );
        unsafe { api.result_free.unwrap()(&mut result) };
        engine = candidate;
    }

    let request = native_request(
        MERMAN_NATIVE_OPERATION_SVG,
        br#"flowchart TD
A@{ icon: "alpha:rocket", label: "A" } --> B@{ icon: "fleet:ship", label: "B" }"#,
    );
    let mut result = native_result();
    assert_eq!(
        unsafe { api.execute_collect.unwrap()(engine, &request, &mut result) },
        MERMAN_NATIVE_STATUS_OK
    );
    let svg = unsafe { std::slice::from_raw_parts(result.data.data, result.data.len) };
    let svg = std::str::from_utf8(svg).expect("SVG output is UTF-8");
    assert!(svg.contains(r#"data-icon="service-rocket""#), "{svg}");
    assert!(svg.contains(r#"data-icon="service-ship""#), "{svg}");
    assert!(
        context.calls.load(std::sync::atomic::Ordering::Relaxed) > 0,
        "rendering must be able to invoke the retained callback"
    );
    unsafe { api.result_free.unwrap()(&mut result) };
    assert_eq!(
        unsafe { api.engine_try_close.unwrap()(engine) },
        MERMAN_NATIVE_STATUS_OK
    );
}

#[cfg(feature = "svg")]
#[test]
fn service_constructor_preserves_structured_icon_registry_failures() {
    let api = api_table();
    let invalid_xml = br#"{"prefix":"test","icons":{"rocket":{"body":"<path>"}}}"#;
    let pack = native_icon_pack(invalid_xml, &[]);
    let config = native_services_config(native_config(), std::slice::from_ref(&pack));
    let mut engine = 0;
    let mut result = native_result();
    assert_eq!(
        unsafe { api.engine_new_with_services.unwrap()(&config, &mut engine, &mut result) },
        MERMAN_NATIVE_STATUS_INVALID_ARGUMENT
    );
    assert_eq!(engine, 0);
    let error = result_json(&result);
    assert_eq!(error["details"]["icon_registry"]["kind_id"], "invalid_xml");
    assert_eq!(error["details"]["icon_registry"]["pack_index"], 0);
    unsafe { api.result_free.unwrap()(&mut result) };

    let registration_name = vec![b'a'; 65];
    let pack = native_icon_pack(br#"{"prefix":"test","icons":{}}"#, &registration_name);
    let config = native_services_config(native_config(), std::slice::from_ref(&pack));
    assert_eq!(
        unsafe { api.engine_new_with_services.unwrap()(&config, &mut engine, &mut result) },
        MERMAN_NATIVE_STATUS_RESOURCE_LIMIT_EXCEEDED
    );
    assert_eq!(engine, 0);
    let error = result_json(&result);
    assert_eq!(
        error["details"]["resource"]["limit_id"],
        IconRegistryResourceLimitId::MaxPrefixBytes.stable_id()
    );
    assert_eq!(
        error["details"]["icon_registry"]["kind_id"],
        "resource_limit_exceeded"
    );
    assert_eq!(error["details"]["icon_registry"]["pack_index"], 0);
    unsafe { api.result_free.unwrap()(&mut result) };
}

#[cfg(feature = "svg")]
#[test]
fn service_constructor_preflights_raw_icon_pack_limits_before_utf8_decoding() {
    let max_pack_bytes =
        usize::try_from(IconRegistryResourceLimitId::MaxPackBytes.fixed_value()).unwrap();
    let max_input_bytes =
        usize::try_from(IconRegistryResourceLimitId::MaxInputBytes.fixed_value()).unwrap();
    let raw_slice = |len| MermanNativeSlice {
        struct_size: native_struct_size::<MermanNativeSlice>(),
        data: if len == 0 {
            ptr::null()
        } else {
            ptr::NonNull::<u8>::dangling().as_ptr()
        },
        len,
    };
    let raw_pack = |json_len, registration_name_len| {
        Some(MermanNativeIconPack {
            struct_size: native_struct_size::<MermanNativeIconPack>(),
            json: raw_slice(json_len),
            registration_name: raw_slice(registration_name_len),
        })
    };
    let assert_preflight_failure = |packs: &[Option<MermanNativeIconPack>],
                                    limit: IconRegistryResourceLimitId,
                                    actual: u64,
                                    pack_index: u64| {
        let failure = preflight_native_icon_pack_resource_limits(packs)
            .expect_err("resource preflight must reject the synthetic lengths");
        assert_eq!(failure.status, MERMAN_NATIVE_STATUS_RESOURCE_LIMIT_EXCEEDED);
        let payload: serde_json::Value =
            serde_json::from_slice(&native_error_json(&failure)).unwrap();
        let resource = &payload["details"]["resource"];
        assert_eq!(resource["limit_id"], limit.stable_id());
        assert_eq!(resource["phase"], limit.descriptor().phase);
        assert_eq!(resource["actual"], actual);
        assert_eq!(resource["max"], limit.fixed_value());
        assert_eq!(resource["profile"], "constructor-fixed");
        let details = &payload["details"]["icon_registry"];
        assert_eq!(details["kind_id"], "resource_limit_exceeded");
        assert_eq!(details["pack_index"], pack_index);
        assert_eq!(details["registration_name"], serde_json::Value::Null);
    };

    assert_preflight_failure(
        &[raw_pack(max_pack_bytes + 1, 0)],
        IconRegistryResourceLimitId::MaxPackBytes,
        u64::try_from(max_pack_bytes + 1).unwrap(),
        0,
    );

    let shared_pack_len = max_input_bytes / 3 + 1;
    assert!(shared_pack_len <= max_pack_bytes);
    let aggregate_packs = [
        raw_pack(shared_pack_len, 1),
        raw_pack(shared_pack_len, 0),
        raw_pack(shared_pack_len, 0),
    ];
    assert_preflight_failure(
        &aggregate_packs,
        IconRegistryResourceLimitId::MaxInputBytes,
        u64::try_from(shared_pack_len * aggregate_packs.len()).unwrap(),
        2,
    );

    let max_prefix_bytes =
        usize::try_from(IconRegistryResourceLimitId::MaxPrefixBytes.fixed_value()).unwrap();
    let mut oversized_invalid_utf8_name = vec![b'a'; max_prefix_bytes + 1];
    oversized_invalid_utf8_name[0] = 0xff;
    let pack = native_icon_pack(
        br#"{"prefix":"test","icons":{}}"#,
        &oversized_invalid_utf8_name,
    );
    let api = api_table();
    let config = native_services_config(native_config(), std::slice::from_ref(&pack));
    let mut engine = 0;
    let mut result = native_result();
    assert_eq!(
        unsafe { api.engine_new_with_services.unwrap()(&config, &mut engine, &mut result) },
        MERMAN_NATIVE_STATUS_RESOURCE_LIMIT_EXCEEDED
    );
    assert_eq!(engine, 0);
    let error = result_json(&result);
    assert_eq!(
        error["details"]["resource"]["limit_id"],
        IconRegistryResourceLimitId::MaxPrefixBytes.stable_id()
    );
    assert_eq!(
        error["details"]["resource"]["actual"],
        u64::try_from(oversized_invalid_utf8_name.len()).unwrap()
    );
    assert_eq!(error["details"]["icon_registry"]["pack_index"], 0);
    assert!(error["details"]["icon_registry"]["registration_name"].is_null());
    unsafe { api.result_free.unwrap()(&mut result) };
}

#[cfg(feature = "svg")]
#[test]
fn service_constructor_rejects_structural_and_output_aliasing_before_pack_reads() {
    let api = api_table();

    let mut overlapping_array = native_services_config(native_config(), &[]);
    overlapping_array.icon_packs = ptr::addr_of!(overlapping_array).cast::<MermanNativeIconPack>();
    overlapping_array.icon_pack_count = 1;
    let mut engine = 0;
    let mut result = native_result();
    assert_eq!(
        unsafe {
            api.engine_new_with_services.unwrap()(&overlapping_array, &mut engine, &mut result)
        },
        MERMAN_NATIVE_STATUS_INVALID_ARGUMENT
    );
    assert_eq!(engine, 0);
    assert_eq!(result.allocation_token, 0);

    let mut misaligned_output_array = native_services_config(native_config(), &[]);
    misaligned_output_array.icon_packs = unsafe {
        ptr::addr_of!(result)
            .cast::<u8>()
            .add(1)
            .cast::<MermanNativeIconPack>()
    };
    misaligned_output_array.icon_pack_count = 1;
    assert_eq!(
        unsafe {
            api.engine_new_with_services.unwrap()(
                &misaligned_output_array,
                &mut engine,
                &mut result,
            )
        },
        MERMAN_NATIVE_STATUS_INVALID_ARGUMENT
    );
    assert_eq!(engine, 0);
    assert_eq!(result.allocation_token, 0);

    let mut early_semantic_error = native_services_config(native_config(), &[]);
    early_semantic_error.engine_config.text_measure_user_data =
        ptr::NonNull::<u8>::dangling().as_ptr().cast();
    early_semantic_error.engine_config.options_json = MermanNativeSlice {
        struct_size: native_struct_size::<MermanNativeSlice>(),
        data: ptr::addr_of!(result).cast::<u8>(),
        len: 1,
    };
    assert_eq!(
        unsafe {
            api.engine_new_with_services.unwrap()(&early_semantic_error, &mut engine, &mut result)
        },
        MERMAN_NATIVE_STATUS_INVALID_ARGUMENT
    );
    assert_eq!(engine, 0);
    assert_eq!(result.allocation_token, 0);

    let mut oversized_output_array = native_services_config(native_config(), &[]);
    oversized_output_array.icon_packs = ptr::addr_of!(result).cast::<MermanNativeIconPack>();
    oversized_output_array.icon_pack_count = NATIVE_ICON_PACK_RECORD_LIMIT + 1;
    assert_eq!(
        unsafe {
            api.engine_new_with_services.unwrap()(&oversized_output_array, &mut engine, &mut result)
        },
        MERMAN_NATIVE_STATUS_INVALID_ARGUMENT
    );
    assert_eq!(engine, 0);
    assert_eq!(result.allocation_token, 0);

    let aliasing_config = ptr::addr_of!(result).cast::<MermanNativeEngineServicesConfig>();
    assert_eq!(
        unsafe {
            api.engine_new_with_services.unwrap()(aliasing_config, ptr::null_mut(), &mut result)
        },
        MERMAN_NATIVE_STATUS_INVALID_ARGUMENT
    );
    assert_eq!(result.allocation_token, 0);
    assert_eq!(result.status, 0);
    assert!(result.metadata_or_error_json.data.is_null());

    let pack = MermanNativeIconPack {
        struct_size: native_struct_size::<MermanNativeIconPack>(),
        json: MermanNativeSlice {
            struct_size: native_struct_size::<MermanNativeSlice>(),
            data: ptr::addr_of!(result).cast::<u8>(),
            len: 1,
        },
        registration_name: borrowed_slice(&[]),
    };
    let config = native_services_config(native_config(), std::slice::from_ref(&pack));
    assert_eq!(
        unsafe { api.engine_new_with_services.unwrap()(&config, &mut engine, &mut result) },
        MERMAN_NATIVE_STATUS_INVALID_ARGUMENT
    );
    assert_eq!(engine, 0);
    assert_eq!(result.allocation_token, 0);
}

#[cfg(feature = "svg")]
#[test]
fn service_constructor_completes_detectable_alias_preflight_before_writing_failures() {
    let api = api_table();
    let mut engine = 0;
    let mut result = native_result();

    let mut malformed_nested = native_services_config(native_config(), &[]);
    malformed_nested.engine_config.struct_size -= 1;
    malformed_nested.icon_packs = ptr::addr_of!(result).cast::<MermanNativeIconPack>();
    malformed_nested.icon_pack_count = 1;
    assert_eq!(
        unsafe {
            api.engine_new_with_services.unwrap()(&malformed_nested, &mut engine, &mut result)
        },
        MERMAN_NATIVE_STATUS_INVALID_ARGUMENT
    );
    assert_eq!(engine, 0);
    assert_eq!(result.allocation_token, 0);
    assert_eq!(result.status, 0);

    let mut malformed_alias = native_icon_pack(br#"{"prefix":"self-alias","icons":{}}"#, &[]);
    malformed_alias.struct_size -= 1;
    malformed_alias.json = MermanNativeSlice {
        struct_size: native_struct_size::<MermanNativeSlice>(),
        data: ptr::addr_of!(result).cast::<u8>(),
        len: 1,
    };
    let config = native_services_config(native_config(), std::slice::from_ref(&malformed_alias));
    assert_eq!(
        unsafe { api.engine_new_with_services.unwrap()(&config, &mut engine, &mut result) },
        MERMAN_NATIVE_STATUS_INVALID_ARGUMENT
    );
    assert_eq!(engine, 0);
    assert_eq!(result.allocation_token, 0);
    assert_eq!(result.status, 0);

    let mut malformed_record = native_icon_pack(br#"{"prefix":"bad-record","icons":{}}"#, &[]);
    malformed_record.struct_size -= 1;
    let mut later_alias = native_icon_pack(br#"{"prefix":"later-alias","icons":{}}"#, &[]);
    later_alias.json = MermanNativeSlice {
        struct_size: native_struct_size::<MermanNativeSlice>(),
        data: ptr::addr_of!(result).cast::<u8>(),
        len: 1,
    };
    let packs = [malformed_record, later_alias];
    let config = native_services_config(native_config(), &packs);
    assert_eq!(
        unsafe { api.engine_new_with_services.unwrap()(&config, &mut engine, &mut result) },
        MERMAN_NATIVE_STATUS_INVALID_ARGUMENT
    );
    assert_eq!(engine, 0);
    assert_eq!(result.allocation_token, 0);
    assert_eq!(result.status, 0);

    let mut malformed_slice = native_icon_pack(br#"{"prefix":"bad-slice","icons":{}}"#, &[]);
    malformed_slice.json = MermanNativeSlice {
        struct_size: native_struct_size::<MermanNativeSlice>(),
        data: ptr::null(),
        len: 1,
    };
    let mut later_alias = native_icon_pack(br#"{"prefix":"later-alias","icons":{}}"#, &[]);
    later_alias.registration_name = MermanNativeSlice {
        struct_size: native_struct_size::<MermanNativeSlice>(),
        data: ptr::addr_of!(result).cast::<u8>(),
        len: 1,
    };
    let packs = [malformed_slice, later_alias];
    let config = native_services_config(native_config(), &packs);
    assert_eq!(
        unsafe { api.engine_new_with_services.unwrap()(&config, &mut engine, &mut result) },
        MERMAN_NATIVE_STATUS_INVALID_ARGUMENT
    );
    assert_eq!(engine, 0);
    assert_eq!(result.allocation_token, 0);
    assert_eq!(result.status, 0);
}

#[cfg(not(feature = "svg"))]
#[test]
fn service_constructor_requires_svg_only_when_icon_packs_are_requested() {
    let api = api_table();
    let config = native_services_config(native_config(), &[]);
    let mut engine = 0;
    let mut result = native_result();
    assert_eq!(
        unsafe { api.engine_new_with_services.unwrap()(&config, &mut engine, &mut result) },
        MERMAN_NATIVE_STATUS_OK
    );
    unsafe { api.result_free.unwrap()(&mut result) };
    assert_eq!(
        unsafe { api.engine_try_close.unwrap()(engine) },
        MERMAN_NATIVE_STATUS_OK
    );

    let mut config = native_services_config(native_config(), &[]);
    config.icon_pack_count = 1;
    config.icon_packs = ptr::null();
    engine = 0;
    assert_eq!(
        unsafe { api.engine_new_with_services.unwrap()(&config, &mut engine, &mut result) },
        MERMAN_NATIVE_STATUS_UNSUPPORTED_OPERATION
    );
    assert_eq!(engine, 0);
    let error = result_json(&result);
    assert_eq!(error["kind"], MERMAN_NATIVE_ERROR_KIND_MISSING_CAPABILITY);
    assert_eq!(error["capability_id"], "svg");
    unsafe { api.result_free.unwrap()(&mut result) };
}

#[test]
fn runtime_catalog_is_the_flat_artifact_owned_contract() {
    let api = api_table();
    let mut result = native_result();
    assert_eq!(
        unsafe { api.runtime_catalog.unwrap()(&mut result) },
        MERMAN_NATIVE_STATUS_OK
    );
    assert_eq!(result.status, MERMAN_NATIVE_STATUS_OK);
    assert_eq!(result.operation, MERMAN_NATIVE_OPERATION_NONE);

    let catalog_bytes = unsafe {
        std::slice::from_raw_parts(
            result.metadata_or_error_json.data,
            result.metadata_or_error_json.len,
        )
    };
    let catalog: serde_json::Value = serde_json::from_slice(catalog_bytes).unwrap();
    let root = catalog.as_object().unwrap();
    assert_eq!(
        root.keys().map(String::as_str).collect::<BTreeSet<_>>(),
        [
            "capabilities",
            "constructor_service_contracts",
            "constructor_service_ids",
            "metadata_ids",
            "option_group_ids",
            "options_schema_versions",
            "output_contracts",
            "package_version",
            "payload_schemas",
            "registry",
            "resources",
            "schema_version",
            "transport_api_version",
        ]
        .into_iter()
        .collect()
    );
    assert_eq!(catalog["schema_version"], 1);
    assert_eq!(catalog["transport_api_version"], MERMAN_NATIVE_ABI_VERSION);
    assert_eq!(catalog["package_version"], env!("CARGO_PKG_VERSION"));
    assert_eq!(
        catalog["options_schema_versions"],
        serde_json::json!([merman_bindings_core::BINDING_OPTIONS_SCHEMA_VERSION])
    );
    assert_eq!(
        catalog["payload_schemas"],
        serde_json::json!([
            { "id": "binding-result", "version": merman_bindings_core::BINDING_RESULT_PAYLOAD_VERSION },
            { "id": "operation-metadata", "version": merman_bindings_core::BINDING_OPERATION_SCHEMA_VERSION },
        ])
    );
    let metadata_ids = catalog["metadata_ids"].as_array().unwrap();
    assert!(
        metadata_ids
            .iter()
            .any(|id| id == "diagram-family-capabilities")
    );
    assert_eq!(
        catalog["option_group_ids"],
        serde_json::json!(
            native_artifact_contract()
                .option_group_keys()
                .map(merman_bindings_core::BindingOptionGroupKey::id)
                .collect::<Vec<_>>()
        )
    );
    assert_eq!(
        catalog["constructor_service_ids"],
        serde_json::json!(
            native_artifact_contract()
                .constructor_service_keys()
                .map(merman_bindings_core::ConstructorServiceKey::id)
                .collect::<Vec<_>>()
        )
    );
    #[cfg(feature = "svg")]
    assert_eq!(
        catalog["constructor_service_ids"],
        serde_json::to_value([
            "host-text-measurement",
            "icon-registry",
            #[cfg(feature = "native-fonts")]
            "native-font-context",
        ])
        .unwrap()
    );
    #[cfg(not(feature = "svg"))]
    assert_eq!(catalog["constructor_service_ids"], serde_json::json!([]));
    assert_eq!(
        catalog["constructor_service_contracts"],
        serde_json::to_value(
            native_artifact_contract()
                .runtime_catalog(MERMAN_NATIVE_ABI_VERSION)
                .constructor_service_contracts
        )
        .unwrap()
    );
    assert!(catalog.get("runtime_contract").is_none());
    assert!(catalog.get("capability_vocabulary").is_none());
    assert_eq!(
        catalog["capabilities"],
        serde_json::to_value(native_artifact_contract().runtime_capabilities()).unwrap()
    );
    let operation_ids = catalog["capabilities"]["operation_ids"].as_array().unwrap();
    for (operation_id, expected) in [
        ("analysis-json", cfg!(feature = "analysis")),
        ("ascii", cfg!(feature = "ascii")),
        ("export-theme-preset", cfg!(feature = "svg")),
        ("resolve-theme", cfg!(feature = "svg")),
        ("jpeg", cfg!(feature = "jpeg")),
        ("pdf", cfg!(feature = "pdf")),
        ("png", cfg!(feature = "png")),
        ("semantic-json", true),
        ("svg", cfg!(feature = "svg")),
    ] {
        assert_eq!(
            operation_ids.iter().any(|id| id == operation_id),
            expected,
            "operation {operation_id} must follow merman-ffi features"
        );
    }
    assert!(
        metadata_ids.iter().any(|id| id == "theme-catalog"),
        "theme catalog metadata must remain discoverable for every artifact"
    );
    for operation_id in operation_ids {
        let key = OperationKey::from_id(operation_id.as_str().unwrap()).unwrap();
        let code = merman_native_operation_code(key)
            .expect("every advertised operation needs a native ABI code");
        assert_eq!(merman_native_operation_key(code), Some(key));
        assert_eq!(
            merman_native_operation_descriptor(code)
                .unwrap()
                .operation_id,
            Some(key.id())
        );
    }
    let capability_ids = catalog["capabilities"]["capability_ids"]
        .as_array()
        .unwrap();
    for (capability_id, expected) in [
        ("layout-cytoscape", cfg!(feature = "layout-cytoscape")),
        ("layout-dagre", cfg!(feature = "layout-dagre")),
        ("layout-elk", cfg!(feature = "layout-elk")),
        ("math", cfg!(feature = "math")),
    ] {
        assert_eq!(
            capability_ids.iter().any(|id| id == capability_id),
            expected,
            "capability {capability_id} must follow merman-ffi features"
        );
    }
    let exposes_system_adapters = cfg!(feature = "native-runtime");
    let system_adapter_ids = catalog["capabilities"]["system_adapter_ids"]
        .as_array()
        .unwrap();
    for adapter_id in ["system-clock", "system-random", "system-timezone"] {
        assert_eq!(
            system_adapter_ids.iter().any(|id| id == adapter_id),
            exposes_system_adapters,
            "system adapter {adapter_id} must follow merman-ffi features"
        );
    }
    assert!(
        !catalog["capabilities"]["system_adapter_ids"]
            .as_array()
            .unwrap()
            .iter()
            .any(|id| id == "system-timing")
    );

    for field in [
        "capability_ids",
        "operation_ids",
        "output_ids",
        "system_adapter_ids",
    ] {
        let ids = catalog["capabilities"][field]
            .as_array()
            .unwrap()
            .iter()
            .map(|id| id.as_str().unwrap())
            .collect::<Vec<_>>();
        let mut sorted = ids.clone();
        sorted.sort_unstable();
        sorted.dedup();
        assert_eq!(ids, sorted, "{field} must be sorted and unique");
    }
    assert!(
        catalog["registry"]["diagram_family_count"]
            .as_u64()
            .is_some_and(|count| count > 0)
    );
    assert!(catalog["resources"].get("schema_version").is_none());
    let source_limit = catalog["resources"]["limits"]
        .as_array()
        .unwrap()
        .iter()
        .find(|limit| limit["id"] == "max_source_bytes")
        .expect("source limit");
    let source_operation_ids = operation_ids
        .iter()
        .filter(|operation_id| {
            merman_bindings_core::BindingOperationKind::from_id(operation_id.as_str().unwrap())
                .unwrap()
                .input_kind()
                == "mermaid-source"
        })
        .collect::<Vec<_>>();
    assert_eq!(
        source_limit["operation_ids"],
        serde_json::json!(source_operation_ids)
    );

    let expected_digest = format!(
        "sha256:{}",
        data_encoding::HEXLOWER.encode_display(&Sha256::digest(catalog_bytes))
    );
    let reported_digest = unsafe {
        std::slice::from_raw_parts(
            api.capability_catalog_digest.data,
            api.capability_catalog_digest.len,
        )
    };
    assert_eq!(reported_digest, expected_digest.as_bytes());
    unsafe { api.result_free.unwrap()(&mut result) };
}

#[test]
fn metadata_collect_returns_owned_catalogs_and_typed_failures() {
    let api = api_table();
    let collect = api.metadata_collect.unwrap();

    let mut result = native_result();
    assert_eq!(
        unsafe { collect(borrowed_slice(b"supported-diagrams"), &mut result) },
        MERMAN_NATIVE_STATUS_OK
    );
    assert_eq!(result.operation, MERMAN_NATIVE_OPERATION_NONE);
    assert_eq!(result.data.len, 0);
    assert_ne!(result.allocation_token, 0);
    assert!(result_json(&result).is_array());
    unsafe { api.result_free.unwrap()(&mut result) };

    let mut result = native_result();
    assert_eq!(
        unsafe { collect(borrowed_slice(b"unknown-catalog"), &mut result) },
        MERMAN_NATIVE_STATUS_INVALID_ARGUMENT
    );
    assert_ne!(result.allocation_token, 0);
    assert_eq!(result_json(&result)["status_name"], "invalid-argument");
    unsafe { api.result_free.unwrap()(&mut result) };

    let mut result = native_result();
    assert_eq!(
        unsafe { collect(borrowed_slice(&[0xff]), &mut result) },
        MERMAN_NATIVE_STATUS_UTF8_ERROR
    );
    assert_ne!(result.allocation_token, 0);
    assert_eq!(result_json(&result)["status_name"], "utf8-error");
    unsafe { api.result_free.unwrap()(&mut result) };

    let mut result = native_result();
    let overlapping_id = MermanNativeSlice {
        struct_size: native_struct_size::<MermanNativeSlice>(),
        data: ptr::addr_of!(result).cast::<u8>(),
        len: 1,
    };
    assert_eq!(
        unsafe { collect(overlapping_id, &mut result) },
        MERMAN_NATIVE_STATUS_INVALID_ARGUMENT
    );
    assert_eq!(result.allocation_token, 0);
    assert!(result.metadata_or_error_json.data.is_null());

    for metadata_id in native_artifact_contract()
        .metadata_keys()
        .map(merman_bindings_core::MetadataKey::id)
    {
        let mut result = native_result();
        let status = unsafe { collect(borrowed_slice(metadata_id.as_bytes()), &mut result) };
        match native_artifact_contract().metadata_json(metadata_id) {
            Ok(expected) => {
                assert_eq!(status, MERMAN_NATIVE_STATUS_OK, "{metadata_id}");
                let actual = unsafe {
                    std::slice::from_raw_parts(
                        result.metadata_or_error_json.data,
                        result.metadata_or_error_json.len,
                    )
                };
                assert_eq!(actual, expected, "{metadata_id}");
                if metadata_id == "theme-catalog" && cfg!(feature = "svg") {
                    let catalog: serde_json::Value = serde_json::from_slice(actual).unwrap();
                    assert_eq!(catalog["schema_version"], 2);
                    assert_eq!(catalog["presets"].as_array().unwrap().len(), 13);
                }
            }
            Err(expected) => {
                let expected_status = native_failure_from_binding(expected.clone()).status;
                assert_eq!(status, expected_status, "{metadata_id}");
                assert_ne!(result.allocation_token, 0, "{metadata_id}");
                let error = result_json(&result);
                assert_eq!(error["kind"], expected.kind().id(), "{metadata_id}");
                assert_eq!(
                    error["capability_id"],
                    serde_json::json!(expected.capability_id()),
                    "{metadata_id}"
                );
            }
        }
        unsafe { api.result_free.unwrap()(&mut result) };
    }
}

#[test]
fn result_output_requires_a_fully_zero_initialized_record() {
    let api = api_table();
    let mut result = native_result();
    result.status = MERMAN_NATIVE_STATUS_INTERNAL_ERROR;

    assert_eq!(
        unsafe { api.runtime_catalog.unwrap()(&mut result) },
        MERMAN_NATIVE_STATUS_INVALID_ARGUMENT
    );
    assert_eq!(result.allocation_token, 0);

    result = native_result();
    assert_eq!(
        unsafe { api.runtime_catalog.unwrap()(&mut result) },
        MERMAN_NATIVE_STATUS_OK
    );
    assert_eq!(result.status, MERMAN_NATIVE_STATUS_OK);
    let allocation_token = result.allocation_token;
    assert!(!result.metadata_or_error_json.data.is_null());
    assert_eq!(
        unsafe { api.runtime_catalog.unwrap()(&mut result) },
        MERMAN_NATIVE_STATUS_INVALID_ARGUMENT
    );
    assert_eq!(result.allocation_token, allocation_token);
    unsafe { api.result_free.unwrap()(&mut result) };
}

#[test]
fn moving_a_result_transfers_opaque_token_ownership() {
    let mut original = native_result();
    assert_eq!(
        unsafe {
            write_native_result(
                &mut original,
                MERMAN_NATIVE_STATUS_OK,
                MERMAN_NATIVE_OPERATION_SEMANTIC_JSON,
                Some("application/json"),
                b"owned by Merman".to_vec(),
                b"{\"ok\":true}".to_vec(),
            )
        },
        MERMAN_NATIVE_STATUS_OK
    );
    let allocation_token = original.allocation_token;
    assert_ne!(allocation_token, 0);
    assert!(result_allocation_is_live(allocation_token));

    let mut moved = unsafe { ptr::read(&original) };
    original = native_result();
    let moved_data = unsafe { std::slice::from_raw_parts(moved.data.data, moved.data.len) };
    assert_eq!(moved_data, b"owned by Merman");
    unsafe { native_result_free(&mut moved) };
    assert!(!result_allocation_is_live(allocation_token));
    assert_eq!(moved.allocation_token, 0);
    unsafe { native_result_free(&mut original) };
}

#[test]
fn result_free_clears_a_moved_record_before_releasing_its_backing_allocation() {
    let mut original = native_result();
    let record_alignment = std::mem::align_of::<MermanNativeResult>();
    let owned_data = vec![0; size_of::<MermanNativeResult>() + record_alignment - 1];
    assert_eq!(
        unsafe {
            write_native_result(
                &mut original,
                MERMAN_NATIVE_STATUS_OK,
                MERMAN_NATIVE_OPERATION_SEMANTIC_JSON,
                Some("application/json"),
                owned_data,
                b"{\"ok\":true}".to_vec(),
            )
        },
        MERMAN_NATIVE_STATUS_OK
    );
    let allocation_token = original.allocation_token;
    let moved_record = unsafe { ptr::read(&original) };
    original = native_result();

    let alignment_offset = moved_record.data.data.align_offset(record_alignment);
    assert_ne!(alignment_offset, usize::MAX);
    let self_buffer_result = unsafe {
        moved_record
            .data
            .data
            .add(alignment_offset)
            .cast::<MermanNativeResult>()
    };
    unsafe { ptr::write(self_buffer_result, moved_record) };

    // The record itself now lives inside `data`. Clearing it after dropping the token-owned
    // vectors would write through a freed pointer; Miri/ASan exercise this exact ownership
    // shape while the registry assertion verifies the normal release path.
    unsafe { native_result_free(self_buffer_result) };
    assert!(!result_allocation_is_live(allocation_token));
    unsafe { native_result_free(&mut original) };
}

#[test]
fn result_free_trusts_only_the_token_and_ignores_buffer_fields() {
    let mut current = native_result();
    assert_eq!(
        unsafe {
            write_native_result(
                &mut current,
                MERMAN_NATIVE_STATUS_OK,
                MERMAN_NATIVE_OPERATION_SEMANTIC_JSON,
                Some("application/json"),
                b"current allocation".to_vec(),
                Vec::new(),
            )
        },
        MERMAN_NATIVE_STATUS_OK
    );
    let token = current.allocation_token;

    let mut foreign = b"host allocation".to_vec();
    current.data = MermanNativeBuffer {
        struct_size: native_struct_size::<MermanNativeBuffer>(),
        data: foreign.as_mut_ptr(),
        len: foreign.len(),
    };

    unsafe { native_result_free(&mut current) };
    assert_eq!(foreign, b"host allocation");
    assert_eq!(current.allocation_token, 0);
    assert!(!result_allocation_is_live(token));
}

#[test]
fn stale_and_random_non_live_tokens_release_nothing_and_tokens_never_reuse() {
    let mut first = native_result();
    assert_eq!(
        unsafe {
            write_native_result(
                &mut first,
                MERMAN_NATIVE_STATUS_OK,
                MERMAN_NATIVE_OPERATION_NONE,
                None,
                Vec::new(),
                Vec::new(),
            )
        },
        MERMAN_NATIVE_STATUS_OK
    );
    let first_token = first.allocation_token;
    unsafe { native_result_free(&mut first) };

    let mut second = native_result();
    assert_eq!(
        unsafe {
            write_native_result(
                &mut second,
                MERMAN_NATIVE_STATUS_OK,
                MERMAN_NATIVE_OPERATION_NONE,
                None,
                Vec::new(),
                Vec::new(),
            )
        },
        MERMAN_NATIVE_STATUS_OK
    );
    assert!(second.allocation_token > first_token);

    let mut stale = native_result();
    stale.allocation_token = first_token;
    unsafe { native_result_free(&mut stale) };
    assert!(result_allocation_is_live(second.allocation_token));

    for token in [
        u64::MAX,
        !MERMAN_NATIVE_TOKEN_DOMAIN_MASK | MERMAN_NATIVE_RESULT_TOKEN_DOMAIN_TAG,
    ] {
        let mut random = native_result();
        random.allocation_token = token;
        unsafe { native_result_free(&mut random) };
        assert_eq!(random.allocation_token, 0);
        assert!(result_allocation_is_live(second.allocation_token));
    }
    unsafe { native_result_free(&mut second) };
}

#[test]
fn allocation_token_exhaustion_is_checked_without_reuse() {
    let registry = Mutex::new(NativeResultRegistry::new(1));
    let mut result = native_result();
    assert_eq!(
        unsafe {
            write_native_result_with_registry(
                &registry,
                &mut result,
                MERMAN_NATIVE_STATUS_OK,
                MERMAN_NATIVE_OPERATION_NONE,
                None,
                Vec::new(),
                Vec::new(),
            )
        },
        MERMAN_NATIVE_STATUS_OK
    );
    assert_eq!(result.allocation_token, encode_result_token(1));
    let retired = registry.lock().unwrap().remove(1);
    assert!(retired.is_some());
    drop(retired);

    result = native_result();
    let status = unsafe {
        write_native_result_with_registry(
            &registry,
            &mut result,
            MERMAN_NATIVE_STATUS_OK,
            MERMAN_NATIVE_OPERATION_NONE,
            None,
            b"rejected data".to_vec(),
            b"rejected metadata".to_vec(),
        )
    };

    assert_eq!(status, MERMAN_NATIVE_STATUS_INTERNAL_ERROR);
    let registry = registry.lock().unwrap();
    assert!(registry.was_issued(1));
    assert!(registry.get(1).is_none());
    assert!(!registry.was_issued(2));
    assert_eq!(
        result.struct_size,
        native_struct_size::<MermanNativeResult>()
    );
    assert_eq!(result.allocation_token, 0);
    assert_eq!(result.status, 0);
    assert_eq!(result.operation, 0);
    assert_eq!(result.media_type.struct_size, 0);
    assert!(result.media_type.data.is_null());
    assert_eq!(result.media_type.len, 0);
    assert_eq!(result.data.struct_size, 0);
    assert!(result.data.data.is_null());
    assert_eq!(result.data.len, 0);
    assert_eq!(result.metadata_or_error_json.struct_size, 0);
    assert!(result.metadata_or_error_json.data.is_null());
    assert_eq!(result.metadata_or_error_json.len, 0);
}

fn test_engine_state() -> Arc<NativeEngineState> {
    Arc::new(NativeEngineState {
        engine: native_artifact_contract()
            .create_engine(&[])
            .expect("test engine"),
        admission: BindingEngineAdmission::new(BindingEngineAdmissionMode::Concurrent),
    })
}

fn publish_test_engine(registry: &Mutex<NativeEngineRegistry>) -> MermanNativeEngineToken {
    let Ok(pending) = PendingNativeEngine::reserve(registry, test_engine_state()) else {
        panic!("test engine reservation failed");
    };
    let mut token = 0;
    unsafe { pending.commit(&mut token) };
    token
}

#[test]
fn engine_and_result_tokens_are_disjoint_positive_signed_64_domains() {
    let engines = Mutex::new(NativeEngineRegistry::new(1));
    let results = Mutex::new(NativeResultRegistry::new(1));

    let engine = publish_test_engine(&engines);
    let mut record = native_result();
    assert_eq!(
        unsafe {
            write_native_result_with_registry(
                &results,
                &mut record,
                MERMAN_NATIVE_STATUS_OK,
                MERMAN_NATIVE_OPERATION_NONE,
                None,
                Vec::new(),
                Vec::new(),
            )
        },
        MERMAN_NATIVE_STATUS_OK
    );
    let result = record.allocation_token;
    assert_ne!(engine, result);
    assert_eq!(
        engine >> MERMAN_NATIVE_TOKEN_COUNTER_SHIFT,
        result >> MERMAN_NATIVE_TOKEN_COUNTER_SHIFT,
        "the same counter value must still produce distinct token domains"
    );
    assert_eq!(
        engine & MERMAN_NATIVE_TOKEN_DOMAIN_MASK,
        MERMAN_NATIVE_ENGINE_TOKEN_DOMAIN_TAG
    );
    assert_eq!(
        result & MERMAN_NATIVE_TOKEN_DOMAIN_MASK,
        MERMAN_NATIVE_RESULT_TOKEN_DOMAIN_TAG
    );
    assert!(i64::try_from(engine).is_ok());
    assert!(i64::try_from(result).is_ok());

    let signed_max = i64::MAX as u64;
    assert_eq!(
        MERMAN_NATIVE_TOKEN_COUNTER_MAX,
        signed_max >> MERMAN_NATIVE_TOKEN_COUNTER_SHIFT
    );
    let last_counter_bits = signed_max & !MERMAN_NATIVE_TOKEN_DOMAIN_MASK;
    let final_engine = encode_engine_token(MERMAN_NATIVE_TOKEN_COUNTER_MAX);
    assert_eq!(
        final_engine,
        last_counter_bits | MERMAN_NATIVE_ENGINE_TOKEN_DOMAIN_TAG
    );
    let final_result = encode_result_token(MERMAN_NATIVE_TOKEN_COUNTER_MAX);
    assert_eq!(
        final_result,
        last_counter_bits | MERMAN_NATIVE_RESULT_TOKEN_DOMAIN_TAG
    );
}

#[test]
fn engine_token_exhaustion_returns_state_for_lock_free_destruction() {
    let registry = Mutex::new(NativeEngineRegistry::new(0));
    let state = test_engine_state();
    let weak = Arc::downgrade(&state);

    let Err(rejected) = PendingNativeEngine::reserve(&registry, state) else {
        panic!("exhausted engine token space must reject publication");
    };
    assert!(registry.try_lock().is_ok());
    assert!(weak.upgrade().is_some());

    let mut token = 0;
    let mut result = native_result();
    let failure = unsafe {
        publish_native_engine_result_with_writer_in_registry(
            &registry,
            rejected.state,
            &mut token,
            &mut result,
            |_| panic!("exhaustion must precede writing success outputs"),
        )
    }
    .expect_err("exhaustion returns an engine constructor failure");
    assert_eq!(failure.status, MERMAN_NATIVE_STATUS_INTERNAL_ERROR);
    assert_eq!(
        failure.message.as_ref(),
        "native engine token space is exhausted"
    );
    assert_eq!(token, 0);
    assert_eq!(result.allocation_token, 0);
    assert!(!registry.lock().unwrap().was_issued(1));
    assert!(
        weak.upgrade().is_none(),
        "the caller can destroy the complete engine graph after releasing the registry lock"
    );
}

#[test]
fn reserved_engine_is_not_acquirable_until_success_outputs_are_committed() {
    let registry = Mutex::new(NativeEngineRegistry::new(1));
    let counter = 1;
    let predicted_token =
        (counter << MERMAN_NATIVE_TOKEN_COUNTER_SHIFT) | MERMAN_NATIVE_ENGINE_TOKEN_DOMAIN_TAG;
    let mut token = 0;
    let mut result = native_result();

    let status = unsafe {
        publish_native_engine_result_with_writer_in_registry(
            &registry,
            test_engine_state(),
            &mut token,
            &mut result,
            |_| {
                let registry = registry
                    .lock()
                    .unwrap_or_else(|poisoned| poisoned.into_inner());
                let entry = registry.get(counter).expect("engine is reserved");
                assert_eq!(entry.publication, NativeEnginePublication::Reserved);
                assert!(entry.published_state().is_none());
                MERMAN_NATIVE_STATUS_OK
            },
        )
    }
    .expect("engine reservation");

    assert_eq!(status, MERMAN_NATIVE_STATUS_OK);
    assert_eq!(token, predicted_token);
    let published = registry
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
        .get(counter)
        .and_then(NativeEngineEntry::published_state)
        .cloned()
        .expect("successful commit publishes the engine");
    let _ = published.admission.try_close();
    drop(published);
    let retired = registry
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
        .remove(counter);
    drop(retired);
}

#[test]
fn engine_publication_rolls_back_result_failure_and_panic_before_exposure() {
    let registry = Mutex::new(NativeEngineRegistry::new(2));
    let state = test_engine_state();
    let weak = Arc::downgrade(&state);
    let mut token = 0;
    let mut result = native_result();
    let status = unsafe {
        publish_native_engine_result_with_writer_in_registry(
            &registry,
            state,
            &mut token,
            &mut result,
            |_| {
                let registry = registry
                    .lock()
                    .unwrap_or_else(|poisoned| poisoned.into_inner());
                let entry = registry.get(1).expect("first engine is reserved");
                assert!(entry.published_state().is_none());
                MERMAN_NATIVE_STATUS_INTERNAL_ERROR
            },
        )
    }
    .expect("a result-writer status is not a constructor failure");
    assert_eq!(status, MERMAN_NATIVE_STATUS_INTERNAL_ERROR);
    assert_eq!(token, 0);
    assert_eq!(result.allocation_token, 0);
    assert!(registry.lock().unwrap().get(1).is_none());
    assert!(weak.upgrade().is_none());

    let state = test_engine_state();
    let weak = Arc::downgrade(&state);
    let unwind = catch_unwind(AssertUnwindSafe(|| unsafe {
        let _ = publish_native_engine_result_with_writer_in_registry(
            &registry,
            state,
            &mut token,
            &mut result,
            |_| {
                let registry = registry
                    .lock()
                    .unwrap_or_else(|poisoned| poisoned.into_inner());
                let entry = registry.get(2).expect("the first counter is never reused");
                assert!(entry.published_state().is_none());
                panic!("synthetic result writer panic")
            },
        );
    }))
    .expect_err("the synthetic result writer must unwind");
    assert_eq!(
        unwind.downcast_ref::<&str>().copied(),
        Some("synthetic result writer panic")
    );
    assert_eq!(token, 0);
    assert_eq!(result.allocation_token, 0);
    assert!(
        registry
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .get(2)
            .is_none()
    );
    assert!(weak.upgrade().is_none());
}

#[cfg(panic = "unwind")]
#[test]
fn result_status_boundary_recovers_when_payload_drop_panics() {
    use std::sync::atomic::{AtomicBool, Ordering};

    struct PanickingPayload(Arc<AtomicBool>);

    impl Drop for PanickingPayload {
        fn drop(&mut self) {
            self.0.store(true, Ordering::Relaxed);
            panic!("synthetic panic in payload destructor");
        }
    }

    let dropped = Arc::new(AtomicBool::new(false));
    let mut result = native_result();
    let status = unsafe {
        result_status_boundary(&mut result, MERMAN_NATIVE_OPERATION_NONE, || {
            std::panic::panic_any(PanickingPayload(Arc::clone(&dropped)))
        })
    };

    assert!(dropped.load(Ordering::Relaxed));
    assert_eq!(status, MERMAN_NATIVE_STATUS_PANIC);
    let error = result_json(&result);
    unsafe { native_result_free(&mut result) };
    assert_eq!(error["status"], MERMAN_NATIVE_STATUS_PANIC);
    assert_eq!(
        error["message"],
        "a Rust panic was caught at the native ABI boundary"
    );
}

#[test]
fn passing_a_token_to_the_wrong_api_never_crosses_registry_domains() {
    let api = api_table();
    let mut catalog = native_result();
    assert_eq!(
        unsafe { api.runtime_catalog.unwrap()(&mut catalog) },
        MERMAN_NATIVE_STATUS_OK
    );
    let result_token = catalog.allocation_token;
    let mut wrong_domain = native_result();
    wrong_domain.allocation_token =
        encode_engine_token(result_token >> MERMAN_NATIVE_TOKEN_COUNTER_SHIFT);
    unsafe { api.result_free.unwrap()(&mut wrong_domain) };
    assert_eq!(wrong_domain.allocation_token, 0);
    assert!(result_allocation_is_live(result_token));
    assert_eq!(
        unsafe { api.engine_try_close.unwrap()(result_token) },
        MERMAN_NATIVE_STATUS_INVALID_ENGINE
    );

    let request = native_request(
        MERMAN_NATIVE_OPERATION_SEMANTIC_JSON,
        b"flowchart TD\nA --> B",
    );
    let mut failure = native_result();
    assert_eq!(
        unsafe { api.execute_collect.unwrap()(result_token, &request, &mut failure) },
        MERMAN_NATIVE_STATUS_INVALID_ENGINE
    );
    unsafe { api.result_free.unwrap()(&mut failure) };
    unsafe { api.result_free.unwrap()(&mut catalog) };

    let mut engine_result = native_result();
    let mut engine = 0;
    assert_eq!(
        unsafe { api.engine_new.unwrap()(&native_config(), &mut engine, &mut engine_result) },
        MERMAN_NATIVE_STATUS_OK
    );
    unsafe { api.result_free.unwrap()(&mut engine_result) };
    let mut forged_result = native_result();
    forged_result.allocation_token = engine;
    unsafe { api.result_free.unwrap()(&mut forged_result) };
    assert_eq!(forged_result.allocation_token, 0);
    assert_eq!(
        unsafe { api.engine_try_close.unwrap()(engine) },
        MERMAN_NATIVE_STATUS_OK
    );
}

#[test]
fn result_token_exhaustion_does_not_retire_or_poison_an_engine() {
    let mut engines = Mutex::new(NativeEngineRegistry::new(1));
    let token = publish_test_engine(&engines);
    let counter = token >> MERMAN_NATIVE_TOKEN_COUNTER_SHIFT;
    let engines = engines.get_mut().unwrap();

    let results = Mutex::new(NativeResultRegistry::new(0));
    let mut result = native_result();
    assert_eq!(
        unsafe {
            write_native_result_with_registry(
                &results,
                &mut result,
                MERMAN_NATIVE_STATUS_OK,
                MERMAN_NATIVE_OPERATION_NONE,
                None,
                Vec::new(),
                Vec::new(),
            )
        },
        MERMAN_NATIVE_STATUS_INTERNAL_ERROR
    );
    assert!(!results.lock().unwrap().was_issued(1));
    assert_eq!(result.allocation_token, 0);

    let acquired = engines
        .get(counter)
        .and_then(NativeEngineEntry::published_state)
        .expect("engine remains published");
    let operation = acquired
        .admission
        .enter_operation()
        .expect("engine remains usable");
    drop(operation);
    assert!(engines.remove(counter).is_some());
}

#[test]
fn specialized_statuses_always_use_their_matching_error_kind() {
    assert_eq!(
        NativeFailure::new(MERMAN_NATIVE_STATUS_BUSY, "busy").kind,
        BindingErrorKind::Busy
    );
    assert_eq!(
        NativeFailure::new(MERMAN_NATIVE_STATUS_REENTRANT_CALL, "reentrant").kind,
        BindingErrorKind::ReentrantCall
    );
}

#[test]
fn semantic_operation_uses_tokenized_engine_and_owned_results() {
    let api = api_table();
    let mut config_result = native_result();
    let mut token = 0;
    let config = native_config();
    let status = unsafe { api.engine_new.unwrap()(&config, &mut token, &mut config_result) };
    assert_eq!(status, MERMAN_NATIVE_STATUS_OK);
    assert_ne!(token, 0);
    unsafe { api.result_free.unwrap()(&mut config_result) };

    let request = native_request(
        MERMAN_NATIVE_OPERATION_SEMANTIC_JSON,
        b"flowchart TD\nA --> B",
    );
    let mut result = native_result();
    let status = unsafe { api.execute_collect.unwrap()(token, &request, &mut result) };
    assert_eq!(status, MERMAN_NATIVE_STATUS_OK);
    assert_eq!(result.operation, MERMAN_NATIVE_OPERATION_SEMANTIC_JSON);
    assert!(!result.data.data.is_null());
    let bytes = unsafe { std::slice::from_raw_parts(result.data.data, result.data.len) };
    assert!(bytes.starts_with(b"{"));
    let metadata = unsafe {
        std::slice::from_raw_parts(
            result.metadata_or_error_json.data,
            result.metadata_or_error_json.len,
        )
    };
    let metadata: serde_json::Value = serde_json::from_slice(metadata).unwrap();
    assert_eq!(metadata["runtime_policy"], "deterministic");
    unsafe { api.result_free.unwrap()(&mut result) };
    assert!(result.data.data.is_null());

    assert_eq!(
        unsafe { api.engine_try_close.unwrap()(token) },
        MERMAN_NATIVE_STATUS_OK
    );
    assert_eq!(
        unsafe { api.engine_try_close.unwrap()(token) },
        MERMAN_NATIVE_STATUS_INVALID_ENGINE
    );
}

#[test]
fn engine_new_rejects_a_nonzero_out_engine_without_overwriting_it() {
    let api = api_table();
    let mut result = native_result();
    let mut token = 42;

    assert_eq!(
        unsafe { api.engine_new.unwrap()(&native_config(), &mut token, &mut result) },
        MERMAN_NATIVE_STATUS_INVALID_ARGUMENT
    );
    assert_eq!(token, 42);
    assert_eq!(result.status, MERMAN_NATIVE_STATUS_INVALID_ARGUMENT);
    assert_eq!(
        result_json(&result)["kind"],
        MERMAN_NATIVE_ERROR_KIND_GENERIC
    );
    unsafe { api.result_free.unwrap()(&mut result) };
}

#[test]
fn none_operation_is_invalid_while_unknown_codes_remain_unknown_operation() {
    let api = api_table();
    let mut config_result = native_result();
    let mut token = 0;
    assert_eq!(
        unsafe { api.engine_new.unwrap()(&native_config(), &mut token, &mut config_result) },
        MERMAN_NATIVE_STATUS_OK
    );
    unsafe { api.result_free.unwrap()(&mut config_result) };

    let none_request = native_request(MERMAN_NATIVE_OPERATION_NONE, b"flowchart TD\nA --> B");
    let mut none_result = native_result();
    assert_eq!(
        unsafe { api.execute_collect.unwrap()(token, &none_request, &mut none_result) },
        MERMAN_NATIVE_STATUS_INVALID_ARGUMENT
    );
    assert_eq!(none_result.status, MERMAN_NATIVE_STATUS_INVALID_ARGUMENT);
    assert_eq!(
        result_json(&none_result)["kind"],
        MERMAN_NATIVE_ERROR_KIND_GENERIC
    );
    unsafe { api.result_free.unwrap()(&mut none_result) };

    let unknown_request = native_request(i32::MAX, b"flowchart TD\nA --> B");
    let mut unknown_result = native_result();
    assert_eq!(
        unsafe { api.execute_collect.unwrap()(token, &unknown_request, &mut unknown_result) },
        MERMAN_NATIVE_STATUS_UNSUPPORTED_OPERATION
    );
    assert_eq!(
        result_json(&unknown_result)["kind"],
        MERMAN_NATIVE_ERROR_KIND_UNKNOWN_OPERATION
    );
    unsafe { api.result_free.unwrap()(&mut unknown_result) };
    assert_eq!(
        unsafe { api.engine_try_close.unwrap()(token) },
        MERMAN_NATIVE_STATUS_OK
    );

    let mut stale_result = native_result();
    assert_eq!(
        unsafe { api.execute_collect.unwrap()(token, &none_request, &mut stale_result) },
        MERMAN_NATIVE_STATUS_INVALID_ENGINE
    );
    assert_eq!(stale_result.status, MERMAN_NATIVE_STATUS_INVALID_ENGINE);
    unsafe { api.result_free.unwrap()(&mut stale_result) };
}

#[test]
fn request_options_cannot_override_the_engine_runtime_policy() {
    let api = api_table();
    let mut config_result = native_result();
    let mut token = 0;
    assert_eq!(
        unsafe { api.engine_new.unwrap()(&native_config(), &mut token, &mut config_result) },
        MERMAN_NATIVE_STATUS_OK
    );
    unsafe { api.result_free.unwrap()(&mut config_result) };

    let request = native_request_with_options(
        MERMAN_NATIVE_OPERATION_SEMANTIC_JSON,
        b"flowchart TD\nA --> B",
        br#"{"runtime_policy":"native"}"#,
    );
    let mut result = native_result();
    assert_eq!(
        unsafe { api.execute_collect.unwrap()(token, &request, &mut result) },
        MERMAN_NATIVE_STATUS_OPTIONS_JSON_ERROR
    );
    let error = result_json(&result);
    assert_eq!(error["status_name"], "options-json-error");
    assert!(
        error["message"]
            .as_str()
            .is_some_and(|message| message.contains("cannot set runtime_policy"))
    );
    unsafe { api.result_free.unwrap()(&mut result) };
    assert_eq!(
        unsafe { api.engine_try_close.unwrap()(token) },
        MERMAN_NATIVE_STATUS_OK
    );
}

#[test]
fn unknown_operation_code_has_a_distinct_machine_readable_error() {
    let api = api_table();
    let mut config_result = native_result();
    let mut token = 0;
    assert_eq!(
        unsafe { api.engine_new.unwrap()(&native_config(), &mut token, &mut config_result) },
        MERMAN_NATIVE_STATUS_OK
    );
    unsafe { api.result_free.unwrap()(&mut config_result) };

    for code in [15, 16, 17, i32::MAX] {
        let request = native_request(code, b"flowchart TD\nA --> B");
        let mut result = native_result();
        assert_eq!(
            unsafe { api.execute_collect.unwrap()(token, &request, &mut result) },
            MERMAN_NATIVE_STATUS_UNSUPPORTED_OPERATION
        );
        assert_eq!(result.operation, MERMAN_NATIVE_OPERATION_NONE);
        let error = result_json(&result);
        assert_eq!(error["version"], MERMAN_NATIVE_RESULT_SCHEMA_VERSION);
        assert_eq!(error["kind"], MERMAN_NATIVE_ERROR_KIND_UNKNOWN_OPERATION);
        assert!(error["capability_id"].is_null());
        unsafe { api.result_free.unwrap()(&mut result) };
    }
    assert_eq!(
        unsafe { api.engine_try_close.unwrap()(token) },
        MERMAN_NATIVE_STATUS_OK
    );
}

#[test]
fn svg_operation_follows_the_ffi_feature_surface_and_owns_its_result() {
    let api = api_table();
    let mut config_result = native_result();
    let mut token = 0;
    assert_eq!(
        unsafe { api.engine_new.unwrap()(&native_config(), &mut token, &mut config_result) },
        MERMAN_NATIVE_STATUS_OK
    );
    unsafe { api.result_free.unwrap()(&mut config_result) };

    let request = native_request(MERMAN_NATIVE_OPERATION_SVG, b"flowchart TD\nA --> B");
    let mut result = native_result();
    let status = unsafe { api.execute_collect.unwrap()(token, &request, &mut result) };
    assert_ne!(result.allocation_token, 0);
    if native_artifact_contract()
        .runtime_capabilities()
        .has_operation("svg")
    {
        assert_eq!(status, MERMAN_NATIVE_STATUS_OK);
        let data = unsafe { std::slice::from_raw_parts(result.data.data, result.data.len) };
        assert!(data.starts_with(b"<svg"));
    } else {
        assert_eq!(status, MERMAN_NATIVE_STATUS_UNSUPPORTED_OPERATION);
        let error = result_json(&result);
        assert_eq!(error["kind"], MERMAN_NATIVE_ERROR_KIND_MISSING_CAPABILITY);
        assert_eq!(error["capability_id"], "svg");
    }
    unsafe { api.result_free.unwrap()(&mut result) };
    assert_eq!(result.allocation_token, 0);
    assert_eq!(
        unsafe { api.engine_try_close.unwrap()(token) },
        MERMAN_NATIVE_STATUS_OK
    );
}

#[cfg(feature = "svg")]
#[test]
fn native_svg_operation_rejects_the_geometry_only_resource_override() {
    let api = api_table();
    let mut configured = native_result();
    let mut token = 0;
    assert_eq!(
        unsafe { api.engine_new.unwrap()(&native_config(), &mut token, &mut configured) },
        MERMAN_NATIVE_STATUS_OK
    );
    unsafe { api.result_free.unwrap()(&mut configured) };
    let request = native_request_with_options(
        MERMAN_NATIVE_OPERATION_SVG,
        b"flowchart LR\nA --> B",
        br#"{"resources":{"limits":{"max_geometry_json_bytes":1024}}}"#,
    );
    let mut result = native_result();
    assert_eq!(
        unsafe { api.execute_collect.unwrap()(token, &request, &mut result) },
        MERMAN_NATIVE_STATUS_INVALID_ARGUMENT
    );
    let error = result_json(&result);
    assert_eq!(error["status_name"], "invalid-argument");
    assert!(
        error["message"]
            .as_str()
            .unwrap()
            .contains("max_geometry_json_bytes")
    );
    unsafe { api.result_free.unwrap()(&mut result) };
    assert_eq!(
        unsafe { api.engine_try_close.unwrap()(token) },
        MERMAN_NATIVE_STATUS_OK
    );
}

#[cfg(all(
    feature = "svg",
    not(feature = "layout-cytoscape"),
    not(feature = "layout-dagre"),
    not(feature = "layout-elk"),
    not(feature = "math")
))]
#[test]
fn ambient_render_dependencies_do_not_widen_the_native_c_owner_contract() {
    let api = api_table();

    let mut catalog_result = native_result();
    assert_eq!(
        unsafe { api.runtime_catalog.unwrap()(&mut catalog_result) },
        MERMAN_NATIVE_STATUS_OK
    );
    let catalog = result_json(&catalog_result);
    let capability_ids = catalog["capabilities"]["capability_ids"]
        .as_array()
        .expect("capability IDs");
    assert!(capability_ids.iter().any(|id| id == "svg"));
    unsafe { api.result_free.unwrap()(&mut catalog_result) };

    let mut config_result = native_result();
    let mut token = 0;
    assert_eq!(
        unsafe { api.engine_new.unwrap()(&native_config(), &mut token, &mut config_result) },
        MERMAN_NATIVE_STATUS_OK
    );
    unsafe { api.result_free.unwrap()(&mut config_result) };

    let default_layout =
        if !merman::svg::layout_elk_available() && merman::svg::layout_dagre_available() {
            "layout-dagre"
        } else {
            "layout-elk"
        };
    for (required_capabilities, source, rejected_capability) in [
        (
            &["layout-cytoscape"][..],
            b"architecture-beta\n  service api(server)[API]".as_slice(),
            "layout-cytoscape",
        ),
        (
            &["layout-dagre"][..],
            b"---\nconfig:\n  layout: dagre\n---\nflowchart TD\nA --> B".as_slice(),
            "layout-dagre",
        ),
        (
            &[default_layout][..],
            b"---\nconfig:\n  layout: elk\n---\nflowchart TD\nA --> B".as_slice(),
            default_layout,
        ),
        (
            &[default_layout, "math"][..],
            b"flowchart TD\nA[\"$$x^2$$\"] --> B".as_slice(),
            default_layout,
        ),
    ] {
        for capability_id in required_capabilities {
            assert!(!capability_ids.iter().any(|id| id == capability_id));
        }

        let plan_request = native_request(MERMAN_NATIVE_OPERATION_SVG_PLAN_JSON, source);
        let mut plan_result = native_result();
        assert_eq!(
            unsafe { api.execute_collect.unwrap()(token, &plan_request, &mut plan_result) },
            MERMAN_NATIVE_STATUS_OK
        );
        let plan_bytes =
            unsafe { std::slice::from_raw_parts(plan_result.data.data, plan_result.data.len) };
        let plan: serde_json::Value = serde_json::from_slice(plan_bytes).unwrap();
        assert_eq!(
            plan["required_capability_ids"],
            serde_json::json!(required_capabilities)
        );
        assert_eq!(
            plan["missing_capability_ids"],
            serde_json::json!(required_capabilities)
        );
        assert_eq!(plan["ready"], false);
        unsafe { api.result_free.unwrap()(&mut plan_result) };

        let render_request = native_request(MERMAN_NATIVE_OPERATION_SVG, source);
        let mut render_result = native_result();
        assert_eq!(
            unsafe { api.execute_collect.unwrap()(token, &render_request, &mut render_result) },
            MERMAN_NATIVE_STATUS_UNSUPPORTED_OPERATION
        );
        let error = result_json(&render_result);
        assert_eq!(error["kind"], MERMAN_NATIVE_ERROR_KIND_MISSING_CAPABILITY);
        assert_eq!(error["capability_id"], rejected_capability);
        unsafe { api.result_free.unwrap()(&mut render_result) };
    }

    let ratex_request = native_request_with_options(
        MERMAN_NATIVE_OPERATION_SVG,
        b"---\nconfig:\n  layout: dagre\n---\nflowchart TD\nA --> B",
        br#"{"environment":{"math_renderer":"ratex"}}"#,
    );
    let mut ratex_result = native_result();
    assert_eq!(
        unsafe { api.execute_collect.unwrap()(token, &ratex_request, &mut ratex_result) },
        MERMAN_NATIVE_STATUS_UNSUPPORTED_OPERATION
    );
    let error = result_json(&ratex_result);
    assert_eq!(error["kind"], MERMAN_NATIVE_ERROR_KIND_MISSING_CAPABILITY);
    assert_eq!(error["capability_id"], "math");
    unsafe { api.result_free.unwrap()(&mut ratex_result) };

    assert_eq!(
        unsafe { api.engine_try_close.unwrap()(token) },
        MERMAN_NATIVE_STATUS_OK
    );
}

#[test]
fn acquired_engine_reference_cannot_enter_after_successful_close() {
    let api = api_table();
    let mut config_result = native_result();
    let mut token = 0;
    assert_eq!(
        unsafe { api.engine_new.unwrap()(&native_config(), &mut token, &mut config_result) },
        MERMAN_NATIVE_STATUS_OK
    );
    unsafe { api.result_free.unwrap()(&mut config_result) };

    let acquired = acquire_engine(token).expect("live token must be acquirable");
    assert_eq!(
        unsafe { api.engine_try_close.unwrap()(token) },
        MERMAN_NATIVE_STATUS_OK
    );
    let failure = match acquire_engine(token) {
        Ok(_) => panic!("retired token must reject new calls"),
        Err(failure) => failure,
    };
    assert_eq!(failure.status, MERMAN_NATIVE_STATUS_INVALID_ENGINE);

    assert!(matches!(
        acquired.admission.enter_operation(),
        Err(BindingEngineAdmissionError::Closed)
    ));
    assert_eq!(
        unsafe { api.engine_try_close.unwrap()(token) },
        MERMAN_NATIVE_STATUS_INVALID_ENGINE
    );
}

#[test]
fn busy_close_does_not_lose_the_engine() {
    let api = api_table();
    let mut config_result = native_result();
    let mut token = 0;
    assert_eq!(
        unsafe { api.engine_new.unwrap()(&native_config(), &mut token, &mut config_result) },
        MERMAN_NATIVE_STATUS_OK
    );
    unsafe { api.result_free.unwrap()(&mut config_result) };

    let acquired = acquire_engine(token).expect("live token");
    let operation = acquired
        .admission
        .enter_operation()
        .expect("active operation");
    assert_eq!(
        unsafe { api.engine_try_close.unwrap()(token) },
        MERMAN_NATIVE_STATUS_BUSY
    );
    assert!(
        acquire_engine(token).is_ok(),
        "busy close must retain the token"
    );

    drop(operation);
    assert_eq!(
        unsafe { api.engine_try_close.unwrap()(token) },
        MERMAN_NATIVE_STATUS_OK
    );
    assert!(acquire_engine(token).is_err());
}

#[test]
fn native_policy_engine_creation_matches_the_owner_adapter_probe() {
    let api = api_table();
    let mut config = native_config();
    config.options_json = borrowed_slice(br#"{"runtime_policy":"native"}"#);
    let mut result = native_result();
    let mut token = 0;
    let status = unsafe { api.engine_new.unwrap()(&config, &mut token, &mut result) };

    let artifact_contract = native_artifact_contract();
    assert_eq!(
        artifact_contract.runtime_policy_exposure(),
        merman_bindings_core::RuntimePolicyExposure::BindingOptions
    );
    let capabilities = artifact_contract.runtime_capabilities();
    let missing_adapter = ["system-clock", "system-timezone", "system-random"]
        .into_iter()
        .find(|adapter| {
            !capabilities
                .system_adapter_ids
                .iter()
                .any(|id| id == adapter)
        });

    if let Some(missing_adapter) = missing_adapter {
        assert!(capabilities.system_adapter_ids.is_empty());
        for adapter_id in ["system-clock", "system-random", "system-timezone"] {
            assert!(
                !capabilities
                    .capability_ids
                    .iter()
                    .any(|id| id == &adapter_id)
            );
        }
        assert_eq!(status, MERMAN_NATIVE_STATUS_UNSUPPORTED_OPERATION);
        assert_eq!(token, 0);
        let error = result_json(&result);
        assert_eq!(error["kind"], MERMAN_NATIVE_ERROR_KIND_MISSING_CAPABILITY);
        assert_eq!(error["capability_id"], missing_adapter);
    } else {
        assert_eq!(status, MERMAN_NATIVE_STATUS_OK);
        assert_ne!(token, 0);
        assert_eq!(
            capabilities.system_adapter_ids,
            ["system-clock", "system-random", "system-timezone"]
        );
        assert_eq!(
            unsafe { api.engine_try_close.unwrap()(token) },
            MERMAN_NATIVE_STATUS_OK
        );
    }
    unsafe { api.result_free.unwrap()(&mut result) };
}

#[cfg(feature = "svg")]
#[test]
fn text_measurement_callback_cannot_reenter_the_same_engine() {
    let api = api_table();
    let mut other_config_result = native_result();
    let mut other_token = 0;
    assert_eq!(
        unsafe {
            api.engine_new.unwrap()(&native_config(), &mut other_token, &mut other_config_result)
        },
        MERMAN_NATIVE_STATUS_OK
    );
    unsafe { api.result_free.unwrap()(&mut other_config_result) };

    let mut context = Box::new(ReentrantTextMeasureContext {
        token: 0,
        other_token,
        nested_status: MERMAN_NATIVE_STATUS_OK,
        nested_result_status: MERMAN_NATIVE_STATUS_OK,
        free_status: MERMAN_NATIVE_STATUS_OK,
        other_engine_status: MERMAN_NATIVE_STATUS_INVALID_ENGINE,
        nested_error_was_typed: false,
    });
    let mut config = native_config();
    config.text_measure = Some(reentrant_text_measure_callback);
    config.text_measure_user_data = (&mut *context as *mut ReentrantTextMeasureContext).cast();
    let mut config_result = native_result();
    let mut token = 0;
    assert_eq!(
        unsafe { api.engine_new.unwrap()(&config, &mut token, &mut config_result) },
        MERMAN_NATIVE_STATUS_OK
    );
    unsafe { api.result_free.unwrap()(&mut config_result) };
    context.token = token;

    let request = native_request_with_options(
        MERMAN_NATIVE_OPERATION_SVG,
        b"flowchart TD\nA[Measured] --> B[Fallback]",
        br#"{"svg":{"diagram_id":"callback-request"}}"#,
    );
    let mut result = native_result();
    assert_eq!(
        unsafe { api.execute_collect.unwrap()(token, &request, &mut result) },
        MERMAN_NATIVE_STATUS_OK
    );
    assert_eq!(context.nested_status, MERMAN_NATIVE_STATUS_REENTRANT_CALL);
    assert_eq!(
        context.nested_result_status,
        MERMAN_NATIVE_STATUS_REENTRANT_CALL
    );
    assert_eq!(context.free_status, MERMAN_NATIVE_STATUS_REENTRANT_CALL);
    assert_eq!(context.other_engine_status, MERMAN_NATIVE_STATUS_OK);
    assert!(context.nested_error_was_typed);
    let svg = unsafe { std::slice::from_raw_parts(result.data.data, result.data.len) };
    assert!(
        svg.windows(b"id=\"callback-request\"".len())
            .any(|window| window == b"id=\"callback-request\"")
    );

    unsafe { api.result_free.unwrap()(&mut result) };
    assert_eq!(
        unsafe { api.engine_try_close.unwrap()(token) },
        MERMAN_NATIVE_STATUS_OK
    );
    assert_eq!(
        unsafe { api.engine_try_close.unwrap()(other_token) },
        MERMAN_NATIVE_STATUS_OK
    );
}

#[cfg(feature = "svg")]
#[test]
fn operation_code_errors_precede_busy_and_reentrant_admission() {
    let api = api_table();
    let context = Box::new(ConcurrentFreeTextMeasureContext {
        blocked_once: std::sync::atomic::AtomicBool::new(true),
        entered: std::sync::Barrier::new(1),
        proceed: std::sync::Barrier::new(1),
    });
    let mut config = native_config();
    config.text_measure = Some(concurrent_free_text_measure_callback);
    config.text_measure_user_data = (&*context as *const ConcurrentFreeTextMeasureContext)
        .cast_mut()
        .cast();
    let mut config_result = native_result();
    let mut token = 0;
    assert_eq!(
        unsafe { api.engine_new.unwrap()(&config, &mut token, &mut config_result) },
        MERMAN_NATIVE_STATUS_OK
    );
    unsafe { api.result_free.unwrap()(&mut config_result) };

    let acquired = acquire_engine(token).expect("live callback engine");
    let operation = acquired
        .admission
        .enter_operation()
        .expect("first operation");
    for (operation_code, status, kind) in [
        (
            MERMAN_NATIVE_OPERATION_NONE,
            MERMAN_NATIVE_STATUS_INVALID_ARGUMENT,
            MERMAN_NATIVE_ERROR_KIND_GENERIC,
        ),
        (
            i32::MAX,
            MERMAN_NATIVE_STATUS_UNSUPPORTED_OPERATION,
            MERMAN_NATIVE_ERROR_KIND_UNKNOWN_OPERATION,
        ),
        (
            MERMAN_NATIVE_OPERATION_SEMANTIC_JSON,
            MERMAN_NATIVE_STATUS_BUSY,
            MERMAN_NATIVE_ERROR_KIND_BUSY,
        ),
    ] {
        let request = native_request(operation_code, b"flowchart TD\nA --> B");
        let mut result = native_result();
        assert_eq!(
            unsafe { api.execute_collect.unwrap()(token, &request, &mut result) },
            status
        );
        assert_eq!(result.status, status);
        assert_eq!(result_json(&result)["kind"], kind);
        unsafe { api.result_free.unwrap()(&mut result) };
    }

    let callback = acquired
        .admission
        .enter_callback()
        .expect("synthetic active callback");
    for (operation_code, status, kind) in [
        (
            MERMAN_NATIVE_OPERATION_NONE,
            MERMAN_NATIVE_STATUS_INVALID_ARGUMENT,
            MERMAN_NATIVE_ERROR_KIND_GENERIC,
        ),
        (
            i32::MAX,
            MERMAN_NATIVE_STATUS_UNSUPPORTED_OPERATION,
            MERMAN_NATIVE_ERROR_KIND_UNKNOWN_OPERATION,
        ),
        (
            MERMAN_NATIVE_OPERATION_SEMANTIC_JSON,
            MERMAN_NATIVE_STATUS_REENTRANT_CALL,
            MERMAN_NATIVE_ERROR_KIND_REENTRANT_CALL,
        ),
    ] {
        let request = native_request(operation_code, b"flowchart TD\nA --> B");
        let mut result = native_result();
        assert_eq!(
            unsafe { api.execute_collect.unwrap()(token, &request, &mut result) },
            status
        );
        assert_eq!(result.status, status);
        assert_eq!(result_json(&result)["kind"], kind);
        unsafe { api.result_free.unwrap()(&mut result) };
    }

    drop(callback);
    drop(operation);
    assert_eq!(
        unsafe { api.engine_try_close.unwrap()(token) },
        MERMAN_NATIVE_STATUS_OK
    );
}

#[cfg(feature = "svg")]
#[test]
fn text_measurement_callback_cannot_reenter_the_same_engine_from_another_thread() {
    let api = api_table();
    let mut context = Box::new(CrossThreadReentrantTextMeasureContext {
        token: 0,
        nested_status: MERMAN_NATIVE_STATUS_OK,
    });
    let mut config = native_config();
    config.text_measure = Some(cross_thread_reentrant_text_measure_callback);
    config.text_measure_user_data =
        (&mut *context as *mut CrossThreadReentrantTextMeasureContext).cast();
    let mut config_result = native_result();
    let mut token = 0;
    assert_eq!(
        unsafe { api.engine_new.unwrap()(&config, &mut token, &mut config_result) },
        MERMAN_NATIVE_STATUS_OK
    );
    unsafe { api.result_free.unwrap()(&mut config_result) };
    context.token = token;

    let request = native_request(
        MERMAN_NATIVE_OPERATION_SVG,
        b"flowchart TD\nA[Measured] --> B[Fallback]",
    );
    let mut result = native_result();
    assert_eq!(
        unsafe { api.execute_collect.unwrap()(token, &request, &mut result) },
        MERMAN_NATIVE_STATUS_OK
    );
    assert_eq!(context.nested_status, MERMAN_NATIVE_STATUS_REENTRANT_CALL);

    unsafe { api.result_free.unwrap()(&mut result) };
    assert_eq!(
        unsafe { api.engine_try_close.unwrap()(token) },
        MERMAN_NATIVE_STATUS_OK
    );
}

#[cfg(feature = "svg")]
#[test]
fn try_close_is_reentrant_while_any_thread_is_in_a_host_callback() {
    let api = api_table();
    let context = Box::new(ConcurrentFreeTextMeasureContext {
        blocked_once: std::sync::atomic::AtomicBool::new(false),
        entered: std::sync::Barrier::new(2),
        proceed: std::sync::Barrier::new(2),
    });
    let mut config = native_config();
    config.text_measure = Some(concurrent_free_text_measure_callback);
    config.text_measure_user_data = (&*context as *const ConcurrentFreeTextMeasureContext)
        .cast_mut()
        .cast();
    let mut config_result = native_result();
    let mut token = 0;
    assert_eq!(
        unsafe { api.engine_new.unwrap()(&config, &mut token, &mut config_result) },
        MERMAN_NATIVE_STATUS_OK
    );
    unsafe { api.result_free.unwrap()(&mut config_result) };

    let execution = std::thread::spawn(move || {
        let request = native_request(
            MERMAN_NATIVE_OPERATION_SVG,
            b"flowchart TD\nA[Measured] --> B[Still alive]",
        );
        let mut result = native_result();
        let status = unsafe { native_execute_collect(token, &request, &mut result) };
        let rendered_svg = if result.data.data.is_null() {
            Vec::new()
        } else {
            unsafe { std::slice::from_raw_parts(result.data.data, result.data.len).to_vec() }
        };
        unsafe { native_result_free(&mut result) };
        (status, rendered_svg)
    });

    context.entered.wait();
    assert_eq!(
        unsafe { api.engine_try_close.unwrap()(token) },
        MERMAN_NATIVE_STATUS_REENTRANT_CALL
    );
    context.proceed.wait();
    let (status, rendered_svg) = execution.join().expect("native execution thread");
    assert_eq!(status, MERMAN_NATIVE_STATUS_OK);
    assert!(rendered_svg.starts_with(b"<svg"));

    assert_eq!(
        unsafe { api.engine_try_close.unwrap()(token) },
        MERMAN_NATIVE_STATUS_OK
    );
}

#[cfg(all(
    feature = "png",
    feature = "jpeg",
    feature = "pdf",
    feature = "native-fonts"
))]
#[test]
fn binary_outputs_use_the_same_generic_operation_path() {
    let api = api_table();
    let mut config_result = native_result();
    let mut token = 0;
    let font = include_bytes!("../../merman-export/assets/fonts/Cousine-Regular.ttf");
    let fonts = [borrowed_slice(font)];
    let config = MermanNativeFontEngineConfig {
        struct_size: native_struct_size::<MermanNativeFontEngineConfig>(),
        options_json: borrowed_slice(b""),
        fonts: fonts.as_ptr(),
        font_count: fonts.len(),
        use_bundled_fonts: 0,
    };
    assert_eq!(
        unsafe { api.engine_new_with_fonts.unwrap()(&config, &mut token, &mut config_result) },
        MERMAN_NATIVE_STATUS_OK
    );
    unsafe { api.result_free.unwrap()(&mut config_result) };

    for (operation, signature) in [
        (MERMAN_NATIVE_OPERATION_PNG, b"\x89PNG\r\n\x1a\n".as_slice()),
        (MERMAN_NATIVE_OPERATION_JPEG, b"\xff\xd8\xff".as_slice()),
        (MERMAN_NATIVE_OPERATION_PDF, b"%PDF-".as_slice()),
    ] {
        let request = native_request(operation, b"flowchart TD\nA[Hello] --> B[World]");
        let mut result = native_result();
        assert_eq!(
            unsafe { api.execute_collect.unwrap()(token, &request, &mut result) },
            MERMAN_NATIVE_STATUS_OK,
            "operation {operation} must be callable through execute_collect"
        );
        assert_eq!(result.operation, operation);
        let data = unsafe { std::slice::from_raw_parts(result.data.data, result.data.len) };
        assert!(
            data.starts_with(signature),
            "operation {operation} did not return its declared binary format"
        );
        let metadata = unsafe {
            std::slice::from_raw_parts(
                result.metadata_or_error_json.data,
                result.metadata_or_error_json.len,
            )
        };
        let metadata: serde_json::Value = serde_json::from_slice(metadata).unwrap();
        assert!(
            metadata
                .get("output_plan")
                .is_some_and(serde_json::Value::is_object),
            "operation {operation} did not preserve effective output-plan metadata"
        );
        unsafe { api.result_free.unwrap()(&mut result) };
    }

    assert_eq!(
        unsafe { api.engine_try_close.unwrap()(token) },
        MERMAN_NATIVE_STATUS_OK
    );
}
