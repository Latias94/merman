#![cfg(all(feature = "analysis", feature = "svg"))]

#[path = "../../../fixtures/bindings/semantic_operations.rs"]
mod semantic_operations;

use merman_ffi::*;
use semantic_operations::SemanticOperationCase;
use serde_json::Value;
use std::mem::size_of;
use std::ptr;

#[test]
fn c_abi_consumes_shared_semantic_operation_fixtures() {
    let api = api_table();
    let mut token = 0;
    let mut engine_result = native_result();
    let engine_config = MermanNativeEngineConfig {
        struct_size: size_of::<MermanNativeEngineConfig>() as u32,
        options_json: borrowed_slice(&[]),
        text_measure: None,
        text_measure_user_data: ptr::null_mut(),
    };
    assert_eq!(
        unsafe { api.engine_new.unwrap()(&engine_config, &mut token, &mut engine_result) },
        MERMAN_NATIVE_STATUS_OK
    );
    unsafe { api.result_free.unwrap()(&mut engine_result) };

    for (index, case) in semantic_operations::load().cases.iter().enumerate() {
        run_case(&api, token, index, case);
    }

    assert_eq!(
        unsafe { api.engine_try_close.unwrap()(token) },
        MERMAN_NATIVE_STATUS_OK
    );
}

#[test]
fn c_abi_constructors_share_the_authoritative_13_operation_matrix() {
    let api = api_table();
    let expectations = merman_bindings_core::binding_operation_expectations();
    assert_eq!(
        expectations.len(),
        13,
        "the native matrix must stay complete"
    );

    for use_services in [false, true] {
        let token = create_engine(&api, use_services);
        for expectation in expectations {
            let operation = native_operation(expectation.operation_id());
            let uri = expectation
                .requires_uri()
                .then_some(b"file:///ffi-operation-matrix.mmd".as_slice())
                .unwrap_or_default();
            let request = MermanNativeOperationRequest {
                struct_size: size_of::<MermanNativeOperationRequest>() as u32,
                operation,
                source: borrowed_slice(b"flowchart TD\nA --> B"),
                uri: borrowed_slice(uri),
                options_json: borrowed_slice(&[]),
            };
            let mut result = native_result();
            let status = unsafe { api.execute_collect.unwrap()(token, &request, &mut result) };
            let label = format!(
                "{} constructor operation `{}`",
                if use_services { "services" } else { "legacy" },
                expectation.operation_id()
            );

            if expectation.compiled() {
                assert_eq!(status, MERMAN_NATIVE_STATUS_OK, "{label}");
                assert_eq!(result.status, MERMAN_NATIVE_STATUS_OK, "{label}");
                assert_eq!(result.operation, operation, "{label}");
                assert_eq!(
                    native_slice(&result.media_type),
                    expectation.media_type().as_bytes(),
                    "{label}"
                );
                let metadata: Value =
                    serde_json::from_slice(native_buffer(&result.metadata_or_error_json))
                        .unwrap_or_else(|error| panic!("{label} metadata is not JSON: {error}"));
                assert_eq!(
                    metadata["operation_id"].as_str(),
                    Some(expectation.operation_id()),
                    "{label}"
                );
            } else {
                let unavailable = expectation
                    .unavailable()
                    .expect("every optional native operation has a capability failure");
                assert_eq!(
                    status, MERMAN_NATIVE_STATUS_UNSUPPORTED_OPERATION,
                    "{label}"
                );
                assert_eq!(result.status, status, "{label}");
                let error: Value =
                    serde_json::from_slice(native_buffer(&result.metadata_or_error_json))
                        .unwrap_or_else(|error| panic!("{label} error is not JSON: {error}"));
                assert_eq!(
                    error["kind"].as_str(),
                    Some(unavailable.error_kind()),
                    "{label}"
                );
                assert_eq!(
                    error["capability_id"].as_str(),
                    Some(unavailable.capability_id()),
                    "{label}"
                );
            }
            unsafe { api.result_free.unwrap()(&mut result) };
        }
        assert_eq!(
            unsafe { api.engine_try_close.unwrap()(token) },
            MERMAN_NATIVE_STATUS_OK
        );
    }
}

fn create_engine(api: &MermanNativeApi, use_services: bool) -> MermanNativeEngineToken {
    let engine_config = MermanNativeEngineConfig {
        struct_size: size_of::<MermanNativeEngineConfig>() as u32,
        options_json: borrowed_slice(&[]),
        text_measure: None,
        text_measure_user_data: ptr::null_mut(),
    };
    let mut token = 0;
    let mut result = native_result();
    let status = if use_services {
        let services = MermanNativeEngineServicesConfig {
            struct_size: size_of::<MermanNativeEngineServicesConfig>() as u32,
            engine_config,
            icon_packs: ptr::null(),
            icon_pack_count: 0,
        };
        unsafe { api.engine_new_with_services.unwrap()(&services, &mut token, &mut result) }
    } else {
        unsafe { api.engine_new.unwrap()(&engine_config, &mut token, &mut result) }
    };
    assert_eq!(status, MERMAN_NATIVE_STATUS_OK);
    assert_ne!(token, 0);
    unsafe { api.result_free.unwrap()(&mut result) };
    token
}

fn run_case(
    api: &MermanNativeApi,
    token: MermanNativeEngineToken,
    index: usize,
    case: &SemanticOperationCase,
) {
    let operation = native_operation(&case.operation_id);
    let options_json = case
        .options
        .as_ref()
        .map(serde_json::to_vec)
        .transpose()
        .expect("validated fixture options must serialize")
        .unwrap_or_default();
    let request = MermanNativeOperationRequest {
        struct_size: size_of::<MermanNativeOperationRequest>() as u32,
        operation,
        source: borrowed_slice(case.source.as_bytes()),
        uri: borrowed_slice(case.uri.as_deref().unwrap_or_default().as_bytes()),
        options_json: borrowed_slice(&options_json),
    };
    let mut result = native_result();
    let status = unsafe { api.execute_collect.unwrap()(token, &request, &mut result) };
    let label = format!("fixture {index} operation `{}`", case.operation_id);

    if let Some(expected_media_type) = &case.expected_media_type {
        assert_eq!(status, MERMAN_NATIVE_STATUS_OK, "{label}");
        assert_eq!(result.status, MERMAN_NATIVE_STATUS_OK, "{label}");
        assert_eq!(result.operation, operation, "{label}");
        assert_eq!(
            native_slice(&result.media_type),
            expected_media_type.as_bytes()
        );
        assert_success_invariants(
            case,
            native_buffer(&result.data),
            native_buffer(&result.metadata_or_error_json),
            &label,
        );
    } else {
        assert_ne!(status, MERMAN_NATIVE_STATUS_OK, "{label}");
        assert_eq!(result.status, status, "{label}");
        let error: Value = serde_json::from_slice(native_buffer(&result.metadata_or_error_json))
            .unwrap_or_else(|error| panic!("{label} returned invalid error JSON: {error}"));
        assert_eq!(
            error["kind"].as_str(),
            case.expected_error_kind.as_deref(),
            "{label}"
        );
        assert!(error["capability_id"].is_null(), "{label}");
        assert_error_invariants(case, &error, &label);
    }

    unsafe { api.result_free.unwrap()(&mut result) };
}

fn native_operation(operation_id: &str) -> MermanNativeOperationCode {
    MERMAN_NATIVE_OPERATION_DESCRIPTORS
        .iter()
        .find(|descriptor| descriptor.operation_id == Some(operation_id))
        .map(|descriptor| descriptor.code)
        .unwrap_or_else(|| panic!("generated native ABI has no operation `{operation_id}`"))
}

fn assert_success_invariants(
    case: &SemanticOperationCase,
    data: &[u8],
    metadata: &[u8],
    label: &str,
) {
    for invariant in &case.payload_invariants {
        match invariant.as_str() {
            "nonempty" => assert!(!data.is_empty(), "{label}"),
            "utf8" => {
                std::str::from_utf8(data)
                    .unwrap_or_else(|error| panic!("{label} payload is not UTF-8: {error}"));
            }
            "json-object" => {
                let payload: Value = serde_json::from_slice(data)
                    .unwrap_or_else(|error| panic!("{label} payload is not JSON: {error}"));
                assert!(payload.is_object(), "{label} payload must be a JSON object");
            }
            "svg-root" => {
                let payload = std::str::from_utf8(data)
                    .unwrap_or_else(|error| panic!("{label} payload is not UTF-8: {error}"));
                assert!(payload.trim_start().starts_with("<svg"), "{label}");
            }
            "metadata-operation-id" => {
                let metadata: Value = serde_json::from_slice(metadata)
                    .unwrap_or_else(|error| panic!("{label} metadata is not JSON: {error}"));
                assert_eq!(
                    metadata["operation_id"].as_str(),
                    Some(case.operation_id.as_str()),
                    "{label}"
                );
            }
            other => panic!("{label} has unsupported success invariant `{other}`"),
        }
    }
}

fn assert_error_invariants(case: &SemanticOperationCase, error: &Value, label: &str) {
    for invariant in &case.payload_invariants {
        match invariant.as_str() {
            "error-message-nonempty" => assert!(
                error["message"]
                    .as_str()
                    .is_some_and(|message| !message.is_empty()),
                "{label}"
            ),
            other => panic!("{label} has unsupported error invariant `{other}`"),
        }
    }
}

fn api_table() -> MermanNativeApi {
    let mut api = MermanNativeApi {
        struct_size: size_of::<MermanNativeApi>() as u32,
        abi_version: 0,
        minimum_prefix_layout_digest: borrowed_slice(&[]),
        full_descriptor_digest: borrowed_slice(&[]),
        capability_catalog_digest: borrowed_slice(&[]),
        package_version: borrowed_slice(&[]),
        runtime_catalog: None,
        engine_new: None,
        engine_try_close: None,
        execute_collect: None,
        result_free: None,
        metadata_collect: None,
        engine_new_with_services: None,
    };
    let request = MermanNativeApiRequest {
        struct_size: size_of::<MermanNativeApiRequest>() as u32,
        expected_abi_version: MERMAN_NATIVE_ABI_VERSION,
        expected_minimum_prefix_layout_digest: borrowed_slice(
            MERMAN_NATIVE_ABI_MINIMUM_PREFIX_LAYOUT_DIGEST.as_bytes(),
        ),
    };
    assert_eq!(
        unsafe { merman_get_native_api(&request, &mut api) },
        MERMAN_NATIVE_STATUS_OK
    );
    api
}

fn native_result() -> MermanNativeResult {
    MermanNativeResult {
        struct_size: size_of::<MermanNativeResult>() as u32,
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

fn borrowed_slice(bytes: &[u8]) -> MermanNativeSlice {
    MermanNativeSlice {
        struct_size: size_of::<MermanNativeSlice>() as u32,
        data: bytes.as_ptr(),
        len: bytes.len(),
    }
}

fn native_slice(slice: &MermanNativeSlice) -> &[u8] {
    if slice.len == 0 {
        return &[];
    }
    assert!(!slice.data.is_null());
    unsafe { std::slice::from_raw_parts(slice.data, slice.len) }
}

fn native_buffer(buffer: &MermanNativeBuffer) -> &[u8] {
    if buffer.len == 0 {
        return &[];
    }
    assert!(!buffer.data.is_null());
    unsafe { std::slice::from_raw_parts(buffer.data, buffer.len) }
}
