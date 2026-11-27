//! C result record publication and ownership of the backing allocations.

use merman_bindings_core::transport_boundary::catch_transport_unwind;
use merman_bindings_core::transport_handles::HandleRegistry;
use std::ptr;
use std::sync::Mutex;

use super::{
    MERMAN_NATIVE_OPERATION_NONE, MERMAN_NATIVE_RESULT_SCHEMA_VERSION,
    MERMAN_NATIVE_RESULT_TOKEN_DOMAIN_TAG, MERMAN_NATIVE_STATUS_INTERNAL_ERROR,
    MERMAN_NATIVE_STATUS_INVALID_ARGUMENT, MERMAN_NATIVE_STATUS_PANIC,
    MERMAN_NATIVE_TOKEN_COUNTER_MAX, MERMAN_NATIVE_TOKEN_COUNTER_SHIFT, MermanNativeBuffer,
    MermanNativeOperationCode, MermanNativeResult, MermanNativeSlice, MermanNativeStatus,
    NativeFailure, merman_native_error_kind_name, merman_native_operation_descriptor,
    merman_native_status_name, native_struct_size, read_record_struct_size, static_slice,
    token_has_domain, validate_pointer_alignment, validate_struct_size,
};

type NativeResultRegistry = HandleRegistry<NativeResultAllocation>;

struct NativeResultAllocation {
    _data: Vec<u8>,
    _metadata_or_error_json: Vec<u8>,
}

static ALLOCATION_REGISTRY: Mutex<NativeResultRegistry> =
    Mutex::new(NativeResultRegistry::new(MERMAN_NATIVE_TOKEN_COUNTER_MAX));

fn encode_result_token(counter: u64) -> u64 {
    debug_assert!((1..=MERMAN_NATIVE_TOKEN_COUNTER_MAX).contains(&counter));
    (counter << MERMAN_NATIVE_TOKEN_COUNTER_SHIFT) | MERMAN_NATIVE_RESULT_TOKEN_DOMAIN_TAG
}

pub(super) fn native_error_json(failure: &NativeFailure) -> Vec<u8> {
    let mut payload = serde_json::json!({
        "version": MERMAN_NATIVE_RESULT_SCHEMA_VERSION,
        "ok": false,
        "status": failure.status,
        "status_name": merman_native_status_name(failure.status),
        "kind": merman_native_error_kind_name(failure.kind),
        "capability_id": failure.capability_id,
        "message": failure.message.as_ref(),
    });
    if let Some(details) = failure.details.as_deref() {
        payload["details"] = details.clone();
    }
    serde_json::to_vec(&payload)
    .unwrap_or_else(|_| {
        format!(
            "{{\"version\":{},\"ok\":false,\"status\":9,\"status_name\":\"internal-error\",\"kind\":\"generic\",\"capability_id\":null,\"message\":\"native error serialization failed\"}}",
            MERMAN_NATIVE_RESULT_SCHEMA_VERSION
        )
        .into_bytes()
    })
}

pub(super) fn native_success_json(operation: &str) -> Vec<u8> {
    serde_json::to_vec(&serde_json::json!({
        "version": MERMAN_NATIVE_RESULT_SCHEMA_VERSION,
        "ok": true,
        "operation": operation,
    }))
    .unwrap_or_default()
}

fn empty_buffer() -> MermanNativeBuffer {
    MermanNativeBuffer {
        struct_size: native_struct_size::<MermanNativeBuffer>(),
        data: ptr::null_mut(),
        len: 0,
    }
}

fn owned_buffer_view(bytes: &mut Vec<u8>) -> MermanNativeBuffer {
    if bytes.is_empty() {
        return empty_buffer();
    }

    MermanNativeBuffer {
        struct_size: native_struct_size::<MermanNativeBuffer>(),
        data: bytes.as_mut_ptr(),
        len: bytes.len(),
    }
}

fn prepare_result_allocation(
    mut data: Vec<u8>,
    mut metadata_or_error_json: Vec<u8>,
) -> (
    NativeResultAllocation,
    MermanNativeBuffer,
    MermanNativeBuffer,
) {
    let data_view = owned_buffer_view(&mut data);
    let metadata_or_error_json_view = owned_buffer_view(&mut metadata_or_error_json);
    (
        NativeResultAllocation {
            _data: data,
            _metadata_or_error_json: metadata_or_error_json,
        },
        data_view,
        metadata_or_error_json_view,
    )
}

fn release_result_allocation(token: u64) {
    if !token_has_domain(token, MERMAN_NATIVE_RESULT_TOKEN_DOMAIN_TAG) {
        return;
    }
    let counter = token >> MERMAN_NATIVE_TOKEN_COUNTER_SHIFT;
    let retired = {
        let mut registry = ALLOCATION_REGISTRY
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        registry.remove(counter)
    };
    drop(retired);
}

fn operation_media_type(operation: MermanNativeOperationCode) -> Option<&'static str> {
    merman_native_operation_descriptor(operation).and_then(|descriptor| descriptor.media_type)
}

pub(super) fn normalized_operation(
    operation: MermanNativeOperationCode,
) -> MermanNativeOperationCode {
    if merman_native_operation_descriptor(operation).is_some() {
        operation
    } else {
        MERMAN_NATIVE_OPERATION_NONE
    }
}

pub(super) fn initialized_native_result() -> MermanNativeResult {
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

pub(super) unsafe fn result_is_writable(
    out_result: *mut MermanNativeResult,
) -> Result<(), NativeFailure> {
    if out_result.is_null() {
        return Err(NativeFailure::new(
            MERMAN_NATIVE_STATUS_INVALID_ARGUMENT,
            "out_result must not be null",
        ));
    }
    validate_pointer_alignment(out_result, "out_result")?;
    let struct_size = unsafe { read_record_struct_size(out_result) };
    validate_struct_size::<MermanNativeResult>(struct_size, "out_result")?;
    let result = unsafe { ptr::read(out_result) };
    let is_zero_initialized = result.allocation_token == 0
        && result.status == 0
        && result.operation == 0
        && result.media_type.struct_size == 0
        && result.media_type.data.is_null()
        && result.media_type.len == 0
        && result.data.struct_size == 0
        && result.data.data.is_null()
        && result.data.len == 0
        && result.metadata_or_error_json.struct_size == 0
        && result.metadata_or_error_json.data.is_null()
        && result.metadata_or_error_json.len == 0;
    if !is_zero_initialized {
        return Err(NativeFailure::new(
            MERMAN_NATIVE_STATUS_INVALID_ARGUMENT,
            "out_result must be fully zero-initialized with only struct_size set",
        ));
    }
    Ok(())
}

pub(super) unsafe fn write_native_result(
    out_result: *mut MermanNativeResult,
    status: MermanNativeStatus,
    operation: MermanNativeOperationCode,
    media_type: Option<&'static str>,
    data: Vec<u8>,
    metadata_or_error_json: Vec<u8>,
) -> MermanNativeStatus {
    unsafe {
        write_native_result_with_registry(
            &ALLOCATION_REGISTRY,
            out_result,
            status,
            operation,
            media_type,
            data,
            metadata_or_error_json,
        )
    }
}

unsafe fn write_native_result_with_registry(
    registry: &Mutex<NativeResultRegistry>,
    out_result: *mut MermanNativeResult,
    status: MermanNativeStatus,
    operation: MermanNativeOperationCode,
    media_type: Option<&'static str>,
    data: Vec<u8>,
    metadata_or_error_json: Vec<u8>,
) -> MermanNativeStatus {
    let (allocation, data, metadata_or_error_json) =
        prepare_result_allocation(data, metadata_or_error_json);
    let rejected = {
        let mut registry = registry
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        match registry.try_insert(allocation) {
            Ok(counter) => {
                unsafe {
                    ptr::write(
                        out_result,
                        MermanNativeResult {
                            struct_size: native_struct_size::<MermanNativeResult>(),
                            allocation_token: encode_result_token(counter),
                            status,
                            operation: normalized_operation(operation),
                            media_type: static_slice(media_type.unwrap_or_default().as_bytes()),
                            data,
                            metadata_or_error_json,
                        },
                    );
                }
                return status;
            }
            Err(rejected) => rejected,
        }
    };
    drop(rejected);
    MERMAN_NATIVE_STATUS_INTERNAL_ERROR
}

pub(super) unsafe fn write_native_failure(
    out_result: *mut MermanNativeResult,
    operation: MermanNativeOperationCode,
    failure: &NativeFailure,
) -> MermanNativeStatus {
    unsafe {
        write_native_result(
            out_result,
            failure.status,
            operation,
            operation_media_type(operation),
            Vec::new(),
            native_error_json(failure),
        )
    }
}

unsafe fn write_failure_if_possible(
    out_result: *mut MermanNativeResult,
    operation: MermanNativeOperationCode,
    failure: &NativeFailure,
) -> Option<MermanNativeStatus> {
    if unsafe { result_is_writable(out_result) }.is_ok() {
        return Some(unsafe { write_native_failure(out_result, operation, failure) });
    }
    None
}

pub(super) unsafe fn result_status_boundary(
    out_result: *mut MermanNativeResult,
    operation: MermanNativeOperationCode,
    f: impl FnOnce() -> MermanNativeStatus,
) -> MermanNativeStatus {
    match catch_transport_unwind(f) {
        Some(status) => status,
        None => {
            let failure = NativeFailure::new(
                MERMAN_NATIVE_STATUS_PANIC,
                "a Rust panic was caught at the native ABI boundary",
            );
            unsafe { write_failure_if_possible(out_result, operation, &failure) }
                .unwrap_or(failure.status)
        }
    }
}

pub(super) unsafe fn free_native_result(result: *mut MermanNativeResult) {
    let _ = catch_transport_unwind(|| unsafe {
        if result.is_null() {
            return;
        }
        if validate_pointer_alignment(result, "result").is_err() {
            return;
        }
        if validate_struct_size::<MermanNativeResult>(read_record_struct_size(result), "result")
            .is_err()
        {
            return;
        }
        let allocation_token = ptr::addr_of!((*result).allocation_token).read();
        ptr::write(result, initialized_native_result());
        release_result_allocation(allocation_token);
    });
}

#[cfg(test)]
pub(super) fn result_allocation_is_live(token: u64) -> bool {
    ALLOCATION_REGISTRY
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
        .get(token >> MERMAN_NATIVE_TOKEN_COUNTER_SHIFT)
        .is_some()
}

#[cfg(test)]
mod tests;
