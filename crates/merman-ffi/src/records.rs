//! Native C record sizes, pointer and range preflight, and borrowed request decoding.

use std::mem::{align_of, size_of};
use std::ptr;

use super::{
    MERMAN_NATIVE_STATUS_INVALID_ARGUMENT, MermanNativeEngineConfig,
    MermanNativeEngineServicesConfig, MermanNativeOperationCode, MermanNativeOperationRequest,
    MermanNativeSlice, NativeFailure,
};

pub(super) fn native_struct_size<T>() -> u32 {
    u32::try_from(size_of::<T>()).expect("native ABI record sizes fit in u32")
}

pub(super) fn static_slice(bytes: &'static [u8]) -> MermanNativeSlice {
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

#[cfg(any(feature = "svg", test))]
pub(super) fn borrowed_slice(bytes: &[u8]) -> MermanNativeSlice {
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

pub(super) fn validate_struct_size<T>(actual: u32, name: &str) -> Result<(), NativeFailure> {
    let required = native_struct_size::<T>();
    if actual != required {
        return Err(NativeFailure::new(
            MERMAN_NATIVE_STATUS_INVALID_ARGUMENT,
            format!("{name}.struct_size is {actual}; expected exactly {required}"),
        ));
    }
    Ok(())
}

pub(super) fn validate_disjoint_storage(
    left: *const u8,
    left_len: usize,
    left_name: &str,
    right: *const u8,
    right_len: usize,
    right_name: &str,
) -> Result<(), NativeFailure> {
    if left_len == 0 || right_len == 0 {
        return Ok(());
    }
    let left_start = left as usize;
    let right_start = right as usize;
    let Some(left_end) = left_start.checked_add(left_len) else {
        return Err(NativeFailure::new(
            MERMAN_NATIVE_STATUS_INVALID_ARGUMENT,
            format!("{left_name} address range overflows usize"),
        ));
    };
    let Some(right_end) = right_start.checked_add(right_len) else {
        return Err(NativeFailure::new(
            MERMAN_NATIVE_STATUS_INVALID_ARGUMENT,
            format!("{right_name} address range overflows usize"),
        ));
    };
    if left_start < right_end && right_start < left_end {
        return Err(NativeFailure::new(
            MERMAN_NATIVE_STATUS_INVALID_ARGUMENT,
            format!("{left_name} and {right_name} must not overlap"),
        ));
    }
    Ok(())
}

/// Reads the leading size of a native record whose prefix is readable under the C ABI.
pub(super) unsafe fn read_record_struct_size<T>(record: *const T) -> u32 {
    unsafe { ptr::read_unaligned(record.cast::<u32>()) }
}

pub(super) fn validate_pointer_alignment<T>(
    record: *const T,
    name: &str,
) -> Result<(), NativeFailure> {
    if !record.is_aligned() {
        return Err(NativeFailure::new(
            MERMAN_NATIVE_STATUS_INVALID_ARGUMENT,
            format!(
                "{name} must be aligned to {} bytes for its native record type",
                align_of::<T>()
            ),
        ));
    }
    Ok(())
}

/// Borrows the bytes after checking the record shape. The host must supply live, readable storage.
pub(super) unsafe fn native_slice_bytes<'a>(
    slice: MermanNativeSlice,
    name: &str,
) -> Result<&'a [u8], NativeFailure> {
    validate_native_slice_shape(slice, name)?;
    if slice.len == 0 {
        return Ok(&[]);
    }
    Ok(unsafe { std::slice::from_raw_parts(slice.data, slice.len) })
}

pub(super) fn validate_native_slice_shape(
    slice: MermanNativeSlice,
    name: &str,
) -> Result<(), NativeFailure> {
    validate_struct_size::<MermanNativeSlice>(slice.struct_size, name)?;
    if slice.len == 0 {
        return Ok(());
    }
    if slice.data.is_null() {
        return Err(NativeFailure::new(
            MERMAN_NATIVE_STATUS_INVALID_ARGUMENT,
            format!("{name}.data must not be null when len is non-zero"),
        ));
    }
    if slice.len > isize::MAX as usize {
        return Err(NativeFailure::new(
            MERMAN_NATIVE_STATUS_INVALID_ARGUMENT,
            format!("{name}.len must not exceed isize::MAX"),
        ));
    }
    if slice.data.addr().checked_add(slice.len).is_none() {
        return Err(NativeFailure::new(
            MERMAN_NATIVE_STATUS_INVALID_ARGUMENT,
            format!("{name} address range overflows usize"),
        ));
    }
    Ok(())
}

/// Reads the base config after checking the fixed-size record prefix.
pub(super) unsafe fn read_engine_config(
    config: *const MermanNativeEngineConfig,
) -> Result<MermanNativeEngineConfig, NativeFailure> {
    if config.is_null() {
        return Err(NativeFailure::new(
            MERMAN_NATIVE_STATUS_INVALID_ARGUMENT,
            "config must not be null",
        ));
    }
    validate_pointer_alignment(config, "config")?;
    validate_struct_size::<MermanNativeEngineConfig>(
        unsafe { read_record_struct_size(config) },
        "config",
    )?;
    Ok(unsafe { ptr::read(config) })
}

#[cfg(any(feature = "svg", test))]
pub(super) fn checked_record_array_byte_len<T>(
    count: usize,
    name: &str,
) -> Result<usize, NativeFailure> {
    let byte_len = count.checked_mul(size_of::<T>()).ok_or_else(|| {
        NativeFailure::new(
            MERMAN_NATIVE_STATUS_INVALID_ARGUMENT,
            format!("{name} byte length overflows usize"),
        )
    })?;
    if byte_len > isize::MAX as usize {
        return Err(NativeFailure::new(
            MERMAN_NATIVE_STATUS_INVALID_ARGUMENT,
            format!("{name} byte length must not exceed isize::MAX"),
        ));
    }
    Ok(byte_len)
}

#[cfg(any(feature = "svg", test))]
pub(super) fn validate_record_array_range<T>(
    records: *const T,
    count: usize,
    name: &str,
) -> Result<usize, NativeFailure> {
    if count == 0 {
        return Ok(0);
    }
    if records.is_null() {
        return Err(NativeFailure::new(
            MERMAN_NATIVE_STATUS_INVALID_ARGUMENT,
            format!("{name} must not be null when its count is non-zero"),
        ));
    }
    let byte_len = checked_record_array_byte_len::<T>(count, name)?;
    if records.addr().checked_add(byte_len).is_none() {
        return Err(NativeFailure::new(
            MERMAN_NATIVE_STATUS_INVALID_ARGUMENT,
            format!("{name} address range overflows usize"),
        ));
    }
    Ok(byte_len)
}

#[cfg(test)]
pub(super) fn validate_record_array_shape<T>(
    records: *const T,
    count: usize,
    name: &str,
) -> Result<usize, NativeFailure> {
    let byte_len = validate_record_array_range(records, count, name)?;
    if count != 0 {
        validate_pointer_alignment(records, name)?;
    }
    Ok(byte_len)
}

pub(super) fn validate_declared_record_array_disjoint<T>(
    records: *const T,
    count: usize,
    records_name: &str,
    other: *const u8,
    other_len: usize,
    other_name: &str,
) -> Result<(), NativeFailure> {
    if records.is_null() || count == 0 || other_len == 0 {
        return Ok(());
    }
    let element_size = size_of::<T>();
    debug_assert_ne!(element_size, 0, "native record arrays never contain ZSTs");
    let records_start = records.addr();
    let other_start = other.addr();
    let other_end = other_start.checked_add(other_len).ok_or_else(|| {
        NativeFailure::new(
            MERMAN_NATIVE_STATUS_INVALID_ARGUMENT,
            format!("{other_name} address range overflows usize"),
        )
    })?;
    let overlaps = if records_start >= other_end {
        false
    } else if records_start >= other_start {
        true
    } else {
        let distance = other_start - records_start;
        count > distance / element_size
    };
    if overlaps {
        return Err(NativeFailure::new(
            MERMAN_NATIVE_STATUS_INVALID_ARGUMENT,
            format!("{records_name} and {other_name} must not overlap"),
        ));
    }
    Ok(())
}

/// Reads the services config after checking the fixed-size record prefix.
pub(super) unsafe fn read_engine_services_config(
    config: *const MermanNativeEngineServicesConfig,
) -> Result<MermanNativeEngineServicesConfig, NativeFailure> {
    if config.is_null() {
        return Err(NativeFailure::new(
            MERMAN_NATIVE_STATUS_INVALID_ARGUMENT,
            "config must not be null",
        ));
    }
    validate_pointer_alignment(config, "config")?;
    validate_struct_size::<MermanNativeEngineServicesConfig>(
        unsafe { read_record_struct_size(config) },
        "config",
    )?;
    Ok(unsafe { ptr::read(config) })
}

pub(super) struct NativeOperationRequest<'a> {
    pub(super) operation: MermanNativeOperationCode,
    pub(super) source: &'a [u8],
    pub(super) uri: Option<&'a [u8]>,
    pub(super) options_json: &'a [u8],
}

/// Reads the request and borrows all three byte ranges for the duration of the operation.
pub(super) unsafe fn read_operation_request<'a>(
    request: *const MermanNativeOperationRequest,
) -> Result<NativeOperationRequest<'a>, NativeFailure> {
    if request.is_null() {
        return Err(NativeFailure::new(
            MERMAN_NATIVE_STATUS_INVALID_ARGUMENT,
            "request must not be null",
        ));
    }
    validate_pointer_alignment(request, "request")?;
    validate_struct_size::<MermanNativeOperationRequest>(
        unsafe { read_record_struct_size(request) },
        "request",
    )?;
    let request = unsafe { ptr::read(request) };
    let source = unsafe { native_slice_bytes(request.source, "request.source") }?;
    let uri = unsafe { native_slice_bytes(request.uri, "request.uri") }?;
    let options_json = unsafe { native_slice_bytes(request.options_json, "request.options_json") }?;
    Ok(NativeOperationRequest {
        operation: request.operation,
        source,
        uri: (!uri.is_empty()).then_some(uri),
        options_json,
    })
}
