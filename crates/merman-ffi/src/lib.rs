#![deny(unsafe_op_in_unsafe_fn)]

//! Native ABI 4 exports for embedding Merman in C-compatible hosts.
//!
//! The only exported C symbol is [`merman_get_native_api`]. Hosts discover a size-tagged
//! function table and execute every operation through the shared binding operation path. No raw Rust
//! allocation or Rust object pointer crosses this boundary.

use merman::OperationControl;
#[cfg(feature = "native-fonts")]
use merman::svg::export::{NativeFontError, NativeFontLimits};
use merman_bindings_core::transport_boundary::catch_transport_unwind;
use merman_bindings_core::transport_handles::{HandleRegistry, OperationControlRegistry};
use merman_bindings_core::{
    BindingEngine, BindingEngineAdmission, BindingEngineAdmissionError, BindingEngineAdmissionMode,
    BindingEngineServices, BindingError, BindingErrorKind, BindingOperationRequest,
    BindingResourceErrorDetails, BindingStatus, OperationKey, ValidatedArtifactContract,
};
#[cfg(feature = "svg")]
use merman_bindings_core::{
    HostMeasurementResult, IconPack, IconRegistryResourceLimitId, build_icon_registry,
};
use sha2::{Digest, Sha256};
use std::mem::{align_of, size_of};
use std::ptr;
use std::sync::{Arc, Mutex, MutexGuard, OnceLock};
use std::time::Duration;

use self::result::{
    free_native_result, native_success_json, normalized_operation, result_is_writable,
    result_status_boundary, write_native_failure, write_native_result,
};

include!("generated/text_measurement_abi.rs");
include!("generated/native_abi.rs");

mod result;

const PACKAGE_VERSION: &[u8] = env!("CARGO_PKG_VERSION").as_bytes();
#[cfg(feature = "svg")]
const NATIVE_ICON_PACK_RECORD_LIMIT: usize = 16;

#[cfg(feature = "svg")]
const _: () = assert!(
    NATIVE_ICON_PACK_RECORD_LIMIT == IconRegistryResourceLimitId::MaxPacks.fixed_value() as usize
);

#[derive(Debug)]
struct NativeFailure {
    status: MermanNativeStatus,
    kind: BindingErrorKind,
    capability_id: Option<&'static str>,
    details: Option<Box<serde_json::Value>>,
    message: Box<str>,
}

impl NativeFailure {
    fn new(status: MermanNativeStatus, message: impl Into<String>) -> Self {
        Self::classified(status, BindingErrorKind::Generic, None, None, message)
    }

    fn reentrant_call(message: impl Into<String>) -> Self {
        Self::new(MERMAN_NATIVE_STATUS_REENTRANT_CALL, message)
    }

    fn unknown_operation(message: impl Into<String>) -> Self {
        Self::classified(
            MERMAN_NATIVE_STATUS_UNSUPPORTED_OPERATION,
            BindingErrorKind::UnknownOperation,
            None,
            None,
            message,
        )
    }

    fn classified(
        status: MermanNativeStatus,
        requested_kind: BindingErrorKind,
        capability_id: Option<&'static str>,
        resource: Option<BindingResourceErrorDetails>,
        message: impl Into<String>,
    ) -> Self {
        let kind = merman_native_normalize_error_kind(status, requested_kind);
        Self {
            status,
            kind,
            capability_id,
            details: resource.map(|resource| {
                Box::new(serde_json::json!({ "resource": resource.js_safe_json() }))
            }),
            message: message.into().into_boxed_str(),
        }
    }

    #[cfg(any(not(feature = "svg"), not(feature = "bundled-fonts")))]
    fn missing_capability(capability_id: &'static str, message: impl Into<String>) -> Self {
        Self::classified(
            MERMAN_NATIVE_STATUS_UNSUPPORTED_OPERATION,
            BindingErrorKind::MissingCapability,
            Some(capability_id),
            None,
            message,
        )
    }
}

#[derive(Debug)]
struct NativeExecution {
    operation: MermanNativeOperationCode,
    media_type: &'static str,
    data: Vec<u8>,
    metadata_json: Vec<u8>,
}

struct NativeEngineState {
    engine: BindingEngine,
    admission: Arc<BindingEngineAdmission>,
}

type NativeEngineRegistry = HandleRegistry<NativeEngineEntry>;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum NativeEnginePublication {
    Reserved,
    Published,
}

struct NativeEngineEntry {
    publication: NativeEnginePublication,
    state: Arc<NativeEngineState>,
}

impl NativeEngineEntry {
    fn published_state(&self) -> Option<&Arc<NativeEngineState>> {
        (self.publication == NativeEnginePublication::Published).then_some(&self.state)
    }
}

fn encode_engine_token(counter: u64) -> MermanNativeEngineToken {
    debug_assert!((1..=MERMAN_NATIVE_TOKEN_COUNTER_MAX).contains(&counter));
    (counter << MERMAN_NATIVE_TOKEN_COUNTER_SHIFT) | MERMAN_NATIVE_ENGINE_TOKEN_DOMAIN_TAG
}

fn token_has_domain(token: u64, domain_tag: u64) -> bool {
    token != 0 && token & MERMAN_NATIVE_TOKEN_DOMAIN_MASK == domain_tag
}

static ENGINE_REGISTRY: Mutex<NativeEngineRegistry> =
    Mutex::new(NativeEngineRegistry::new(MERMAN_NATIVE_TOKEN_COUNTER_MAX));
static OPERATION_CONTROL_REGISTRY: Mutex<OperationControlRegistry> = Mutex::new(
    OperationControlRegistry::new(MERMAN_NATIVE_TOKEN_COUNTER_MAX),
);
static RUNTIME_CATALOG: OnceLock<Box<[u8]>> = OnceLock::new();
static RUNTIME_CATALOG_DIGEST: OnceLock<Box<[u8]>> = OnceLock::new();
static ARTIFACT_CONTRACT: ValidatedArtifactContract =
    merman_bindings_core::native_sdk_artifact_contract!(NativeC);

fn reentrant_call_failure() -> NativeFailure {
    NativeFailure::reentrant_call("a host callback must not re-enter the same native engine")
}

fn native_failure_from_admission(error: BindingEngineAdmissionError) -> NativeFailure {
    match error {
        BindingEngineAdmissionError::Busy => NativeFailure::new(
            MERMAN_NATIVE_STATUS_BUSY,
            "the native engine has an active operation",
        ),
        BindingEngineAdmissionError::ReentrantCall => reentrant_call_failure(),
        BindingEngineAdmissionError::Closed => NativeFailure::new(
            MERMAN_NATIVE_STATUS_INVALID_ENGINE,
            "engine token is closed",
        ),
        BindingEngineAdmissionError::InvalidCallbackState => NativeFailure::new(
            MERMAN_NATIVE_STATUS_INTERNAL_ERROR,
            "host callback admission was requested outside its engine operation",
        ),
        BindingEngineAdmissionError::CounterExhausted => NativeFailure::new(
            MERMAN_NATIVE_STATUS_INTERNAL_ERROR,
            "native engine operation counter is exhausted",
        ),
    }
}

fn native_struct_size<T>() -> u32 {
    u32::try_from(size_of::<T>()).expect("native ABI record sizes fit in u32")
}

fn native_failure_from_binding(error: BindingError) -> NativeFailure {
    let status = match error.status() {
        BindingStatus::Ok => MERMAN_NATIVE_STATUS_INTERNAL_ERROR,
        BindingStatus::InvalidArgument => MERMAN_NATIVE_STATUS_INVALID_ARGUMENT,
        BindingStatus::Utf8Error => MERMAN_NATIVE_STATUS_UTF8_ERROR,
        BindingStatus::OptionsJsonError => MERMAN_NATIVE_STATUS_OPTIONS_JSON_ERROR,
        BindingStatus::NoDiagram => MERMAN_NATIVE_STATUS_NO_DIAGRAM,
        BindingStatus::ParseError => MERMAN_NATIVE_STATUS_PARSE_ERROR,
        BindingStatus::RenderError => MERMAN_NATIVE_STATUS_RENDER_ERROR,
        BindingStatus::UnsupportedOperation => MERMAN_NATIVE_STATUS_UNSUPPORTED_OPERATION,
        BindingStatus::Panic => MERMAN_NATIVE_STATUS_PANIC,
        BindingStatus::InternalError => MERMAN_NATIVE_STATUS_INTERNAL_ERROR,
        BindingStatus::ResourceLimitExceeded => MERMAN_NATIVE_STATUS_RESOURCE_LIMIT_EXCEEDED,
        BindingStatus::Busy => MERMAN_NATIVE_STATUS_BUSY,
        BindingStatus::Cancelled => MERMAN_NATIVE_STATUS_CANCELLED,
    };
    let details = match merman_bindings_core::binding_error_js_details_json(&error) {
        Ok(details) => details,
        Err(error) => {
            return NativeFailure::new(
                MERMAN_NATIVE_STATUS_INTERNAL_ERROR,
                format!("failed to encode binding error details: {error}"),
            );
        }
    };
    let mut failure = NativeFailure::classified(
        status,
        error.kind(),
        error.capability_id(),
        None,
        error.message(),
    );
    failure.details = details.map(Box::new);
    failure
}

fn native_artifact_contract() -> &'static ValidatedArtifactContract {
    &ARTIFACT_CONTRACT
}

fn operation_control_registry() -> MutexGuard<'static, OperationControlRegistry> {
    OPERATION_CONTROL_REGISTRY
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
}

fn registered_operation_control(
    token: MermanNativeOperationControlToken,
) -> Option<OperationControl> {
    operation_control_registry()
        .get(token >> MERMAN_NATIVE_TOKEN_COUNTER_SHIFT)
        .cloned()
}

fn retire_operation_control(token: MermanNativeOperationControlToken) -> Option<OperationControl> {
    operation_control_registry().remove(token >> MERMAN_NATIVE_TOKEN_COUNTER_SHIFT)
}

fn runtime_catalog_bytes() -> Result<&'static [u8], NativeFailure> {
    if let Some(bytes) = RUNTIME_CATALOG.get() {
        return Ok(bytes);
    }

    let candidate = native_artifact_contract()
        .runtime_catalog_json(MERMAN_NATIVE_ABI_VERSION)
        .map_err(native_failure_from_binding)?
        .into_boxed_slice();
    let _ = RUNTIME_CATALOG.set(candidate);
    Ok(RUNTIME_CATALOG
        .get()
        .expect("runtime catalog must be initialized after set"))
}

fn runtime_catalog_digest_bytes() -> Result<&'static [u8], NativeFailure> {
    if let Some(bytes) = RUNTIME_CATALOG_DIGEST.get() {
        return Ok(bytes);
    }

    let catalog = runtime_catalog_bytes()?;
    let digest = Sha256::digest(catalog);
    let prefix = "sha256:";
    let mut encoded = String::with_capacity(prefix.len() + 2 * digest.len());
    encoded.push_str(prefix);
    merman::model::push_hex_lower(&mut encoded, &digest);
    let _ = RUNTIME_CATALOG_DIGEST.set(encoded.into_bytes().into_boxed_slice());
    Ok(RUNTIME_CATALOG_DIGEST
        .get()
        .expect("runtime catalog digest must be initialized after set"))
}

fn static_slice(bytes: &'static [u8]) -> MermanNativeSlice {
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

fn validate_struct_size<T>(actual: u32, name: &str) -> Result<(), NativeFailure> {
    let required = native_struct_size::<T>();
    if actual != required {
        return Err(NativeFailure::new(
            MERMAN_NATIVE_STATUS_INVALID_ARGUMENT,
            format!("{name}.struct_size is {actual}; expected exactly {required}"),
        ));
    }
    Ok(())
}

fn validate_disjoint_storage(
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

unsafe fn read_record_struct_size<T>(record: *const T) -> u32 {
    unsafe { ptr::read_unaligned(record.cast::<u32>()) }
}

fn validate_pointer_alignment<T>(record: *const T, name: &str) -> Result<(), NativeFailure> {
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

unsafe fn native_slice_bytes<'a>(
    slice: MermanNativeSlice,
    name: &str,
) -> Result<&'a [u8], NativeFailure> {
    validate_native_slice_shape(slice, name)?;
    if slice.len == 0 {
        return Ok(&[]);
    }
    Ok(unsafe { std::slice::from_raw_parts(slice.data, slice.len) })
}

fn validate_native_slice_shape(slice: MermanNativeSlice, name: &str) -> Result<(), NativeFailure> {
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

unsafe fn read_engine_config(
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
fn checked_record_array_byte_len<T>(count: usize, name: &str) -> Result<usize, NativeFailure> {
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
fn validate_record_array_range<T>(
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
fn validate_record_array_shape<T>(
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

fn validate_declared_record_array_disjoint<T>(
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

struct DeferredNativeFailure {
    failure: NativeFailure,
    #[cfg(feature = "svg")]
    result_write_is_safe: bool,
}

fn defer_first_failure(slot: &mut Option<DeferredNativeFailure>, failure: NativeFailure) {
    if slot.is_none() {
        *slot = Some(DeferredNativeFailure {
            failure,
            #[cfg(feature = "svg")]
            result_write_is_safe: true,
        });
    }
}

#[cfg(feature = "svg")]
fn defer_first_status_only_failure(
    slot: &mut Option<DeferredNativeFailure>,
    failure: NativeFailure,
) {
    if let Some(deferred) = slot {
        deferred.result_write_is_safe = false;
    } else {
        *slot = Some(DeferredNativeFailure {
            failure,
            result_write_is_safe: false,
        });
    }
}

unsafe fn read_engine_services_config(
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

struct NativeOperationRequest<'a> {
    operation: MermanNativeOperationCode,
    source: &'a [u8],
    uri: Option<&'a [u8]>,
    options_json: &'a [u8],
}

unsafe fn read_operation_request<'a>(
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

fn acquire_operation_control(
    token: MermanNativeOperationControlToken,
) -> Result<Option<OperationControl>, NativeFailure> {
    if token == 0 {
        return Ok(None);
    }
    if !token_has_domain(token, MERMAN_NATIVE_OPERATION_CONTROL_TOKEN_DOMAIN_TAG) {
        return Err(NativeFailure::new(
            MERMAN_NATIVE_STATUS_INVALID_ARGUMENT,
            "operation control token is zero or belongs to a different opaque token domain",
        ));
    }
    registered_operation_control(token)
        .map(Some)
        .ok_or_else(|| {
            NativeFailure::new(
                MERMAN_NATIVE_STATUS_INVALID_ARGUMENT,
                "operation control token is unknown or has already been released",
            )
        })
}

fn acquire_engine(token: MermanNativeEngineToken) -> Result<Arc<NativeEngineState>, NativeFailure> {
    if !token_has_domain(token, MERMAN_NATIVE_ENGINE_TOKEN_DOMAIN_TAG) {
        return Err(NativeFailure::new(
            MERMAN_NATIVE_STATUS_INVALID_ENGINE,
            "engine token is zero or belongs to a different opaque token domain",
        ));
    }
    let registry = ENGINE_REGISTRY
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    registry
        .get(token >> MERMAN_NATIVE_TOKEN_COUNTER_SHIFT)
        .and_then(NativeEngineEntry::published_state)
        .cloned()
        .ok_or_else(|| {
            NativeFailure::new(
                MERMAN_NATIVE_STATUS_INVALID_ENGINE,
                "engine token is unknown or has already been released",
            )
        })
}

fn resolve_executable_native_operation(
    operation: MermanNativeOperationCode,
) -> Result<OperationKey, NativeFailure> {
    let descriptor = merman_native_operation_descriptor(operation).ok_or_else(|| {
        NativeFailure::unknown_operation(format!("unknown native operation code `{operation}`"))
    })?;
    if !descriptor.executable {
        let failure = descriptor
            .non_executable_failure
            .expect("validated non-executable native operations define a failure");
        return Err(NativeFailure::classified(
            failure.status,
            merman_binding_error_kind_from_native_name(failure.error_kind)
                .expect("descriptor-generated operation failures use a known error kind"),
            None,
            None,
            "native operation NONE is a non-executable sentinel",
        ));
    }
    Ok(merman_native_operation_key(operation)
        .expect("validated executable native operations map to binding operation keys"))
}

fn execute_with_engine<T>(
    token: MermanNativeEngineToken,
    control_token: MermanNativeOperationControlToken,
    request: NativeOperationRequest<'_>,
    consume: impl FnOnce(NativeExecution) -> Result<T, NativeFailure>,
) -> Result<T, NativeFailure> {
    let state = acquire_engine(token)?;
    // Clone the operation control while the registry lock is held, then release every registry
    // lock before admission and synchronous execution. Releasing the token concurrently is safe:
    // the in-flight clone retains the control state until this operation returns.
    let operation_control = acquire_operation_control(control_token)?;
    // Freeze request classification before lifecycle admission. For a live token, NONE and
    // unknown numeric codes are caller errors regardless of transient busy/reentrant state.
    let operation = resolve_executable_native_operation(request.operation)?;
    let _operation = state
        .admission
        .enter_operation()
        .map_err(native_failure_from_admission)?;
    let result = state
        .engine
        .execute(
            BindingOperationRequest::new(operation.spec().id, request.source)
                .with_optional_uri(request.uri)
                .with_options_json(request.options_json)
                .with_control(operation_control.unwrap_or_default()),
        )
        .map_err(native_failure_from_binding)?;
    let (operation, media_type, data, metadata) = result.into_parts();

    consume(NativeExecution {
        operation: merman_native_operation_code(operation.key())
            .expect("every native ABI operation must retain its frozen numeric code"),
        media_type,
        data,
        metadata_json: metadata.into_json_bytes(),
    })
}

#[cfg(feature = "svg")]
#[derive(Clone)]
struct NativeHostTextMeasurer {
    callback: MermanNativeTextMeasureCallback,
    user_data: usize,
    admission: Arc<BindingEngineAdmission>,
}

#[cfg(feature = "svg")]
impl NativeHostTextMeasurer {
    const DEFAULT_FONT_STYLE: &'static [u8] = b"normal";
    const DEFAULT_FONT_WEIGHT: &'static [u8] = b"normal";

    fn new(
        callback: MermanNativeTextMeasureCallback,
        user_data: *mut std::ffi::c_void,
        admission: Arc<BindingEngineAdmission>,
    ) -> Self {
        Self {
            callback,
            user_data: user_data as usize,
            admission,
        }
    }

    fn measure_host(
        &self,
        request: merman_bindings_core::HostTextMeasurementRequest<'_>,
    ) -> HostMeasurementResult {
        let transport = merman_bindings_core::host_text_measurement_transport_fields(request);
        let style = request.style;
        let max_width = request.max_width;
        let font_family = style.font_family.as_deref().unwrap_or_default().as_bytes();
        let font_weight = style
            .font_weight
            .as_deref()
            .map(str::as_bytes)
            .unwrap_or(Self::DEFAULT_FONT_WEIGHT);
        let font_style = style
            .font_style
            .as_deref()
            .map(str::as_bytes)
            .unwrap_or(Self::DEFAULT_FONT_STYLE);
        let native_request = MermanNativeTextMeasureRequest {
            struct_size: native_struct_size::<MermanNativeTextMeasureRequest>(),
            text_measurement_protocol_version: MERMAN_TEXT_MEASUREMENT_PROTOCOL_VERSION,
            text: borrowed_slice(request.text.as_bytes()),
            font_family: borrowed_slice(font_family),
            font_size: style.font_size,
            font_weight: borrowed_slice(font_weight),
            font_style: borrowed_slice(font_style),
            max_width: max_width.unwrap_or(0.0),
            line_height: transport.line_height,
            letter_spacing: 0.0,
            word_spacing: 0.0,
            wrap_mode: transport.wrap_mode,
            direction: transport.direction,
            white_space: transport.white_space,
            has_max_width: u8::from(max_width.is_some()),
            phase: transport.phase,
            operation: transport.operation,
        };
        let mut native_result = MermanNativeTextMeasureResult {
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
        };
        let _callback = self
            .admission
            .enter_callback()
            .map_err(native_failure_from_admission)
            .map_err(|failure| {
                merman_bindings_core::HostTextMeasurementError::new(failure.message)
            })?;
        let status = unsafe {
            (self.callback)(
                &native_request,
                &mut native_result,
                self.user_data as *mut std::ffi::c_void,
            )
        };
        if !merman_native_status_is_known(status) {
            return Err(
                merman_bindings_core::HostTextMeasurementError::invalid_value(format!(
                    "host text-measure callback returned unknown status {status}"
                )),
            );
        }
        if status != MERMAN_NATIVE_STATUS_OK {
            return Err(merman_bindings_core::HostTextMeasurementError::new(
                "host text-measure callback returned an error status",
            ));
        }
        if validate_struct_size::<MermanNativeTextMeasureResult>(
            native_result.struct_size,
            "host text-measure result",
        )
        .is_err()
            || native_result.handled > 1
            || native_result.has_raw_width > 1
        {
            return Err(
                merman_bindings_core::HostTextMeasurementError::invalid_value(
                    "host text-measure callback returned an invalid result record",
                ),
            );
        }
        if native_result.handled == 0 {
            return Ok(None);
        }

        use merman_bindings_core::HostTextMeasurementResultKind as ResultKind;
        let kind = ResultKind::from_external_code(native_result.result_kind);
        let metrics = matches!(
            kind,
            Some(ResultKind::Metrics | ResultKind::WrappedWithRawWidth)
        );
        let length = matches!(kind, Some(ResultKind::Length));
        let extents = matches!(kind, Some(ResultKind::HorizontalExtents));
        let record = merman_bindings_core::HostTextMeasurementRecord {
            result_kind: kind,
            width: metrics.then_some(native_result.width),
            height: metrics.then_some(native_result.height),
            line_count: metrics.then_some(
                i128::try_from(native_result.line_count)
                    .expect("C size_t line counts fit in an i128"),
            ),
            length: length.then_some(native_result.length),
            bbox_left: extents.then_some(native_result.bbox_left),
            bbox_right: extents.then_some(native_result.bbox_right),
            raw_width: (native_result.has_raw_width != 0).then_some(native_result.raw_width),
        };
        merman_bindings_core::decode_host_text_measurement(request, record).map(Some)
    }
}

#[cfg(feature = "svg")]
impl merman_bindings_core::HostTextMeasurer for NativeHostTextMeasurer {
    fn measure(
        &self,
        request: merman_bindings_core::HostTextMeasurementRequest<'_>,
    ) -> HostMeasurementResult {
        self.measure_host(request)
    }
}

fn status_boundary(f: impl FnOnce() -> MermanNativeStatus) -> MermanNativeStatus {
    catch_transport_unwind(f).unwrap_or(MERMAN_NATIVE_STATUS_PANIC)
}

/// Discovers the generated native ABI 4 function table.
///
/// # Safety
///
/// `request` and `out_api` must point to writable/readable records with the generated ABI layouts.
/// Their `struct_size` fields must describe at least the record prefix consumed by this ABI, and all
/// slices referenced by `request` must remain valid for the duration of the call.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn merman_get_native_api(
    request: *const MermanNativeApiRequest,
    out_api: *mut MermanNativeApi,
) -> MermanNativeStatus {
    status_boundary(|| unsafe { get_native_api_impl(request, out_api) })
}

unsafe fn get_native_api_impl(
    request: *const MermanNativeApiRequest,
    out_api: *mut MermanNativeApi,
) -> MermanNativeStatus {
    let result = (|| {
        if request.is_null() {
            return Err(NativeFailure::new(
                MERMAN_NATIVE_STATUS_INVALID_ARGUMENT,
                "request must not be null",
            ));
        }
        if out_api.is_null() {
            return Err(NativeFailure::new(
                MERMAN_NATIVE_STATUS_INVALID_ARGUMENT,
                "out_api must not be null",
            ));
        }
        validate_pointer_alignment(request, "request")?;
        validate_pointer_alignment(out_api, "out_api")?;
        validate_struct_size::<MermanNativeApiRequest>(
            unsafe { read_record_struct_size(request) },
            "request",
        )?;
        let request = unsafe { ptr::read(request) };
        let output_capacity = unsafe { read_record_struct_size(out_api) };
        if output_capacity < MERMAN_NATIVE_API_MINIMUM_PREFIX_SIZE {
            return Err(NativeFailure::new(
                MERMAN_NATIVE_STATUS_INVALID_ARGUMENT,
                format!(
                    "out_api.struct_size capacity is {output_capacity}; expected at least the native ABI minimum prefix size {MERMAN_NATIVE_API_MINIMUM_PREFIX_SIZE}"
                ),
            ));
        }
        if request.expected_abi_version != MERMAN_NATIVE_ABI_VERSION {
            return Err(NativeFailure::new(
                MERMAN_NATIVE_STATUS_ABI_MISMATCH,
                format!(
                    "native ABI {} was requested; this library implements {}",
                    request.expected_abi_version, MERMAN_NATIVE_ABI_VERSION
                ),
            ));
        }
        let expected_prefix_digest = unsafe {
            native_slice_bytes(
                request.expected_minimum_prefix_layout_digest,
                "request.expected_minimum_prefix_layout_digest",
            )
        }?;
        if expected_prefix_digest != MERMAN_NATIVE_ABI_MINIMUM_PREFIX_LAYOUT_DIGEST.as_bytes() {
            return Err(NativeFailure::new(
                MERMAN_NATIVE_STATUS_ABI_LAYOUT_MISMATCH,
                "native ABI minimum-prefix layout digest does not match this library",
            ));
        }

        let capability_catalog_digest = runtime_catalog_digest_bytes()?;
        let api = MermanNativeApi {
            struct_size: native_struct_size::<MermanNativeApi>(),
            abi_version: MERMAN_NATIVE_ABI_VERSION,
            minimum_prefix_layout_digest: static_slice(
                MERMAN_NATIVE_ABI_MINIMUM_PREFIX_LAYOUT_DIGEST.as_bytes(),
            ),
            full_descriptor_digest: static_slice(
                MERMAN_NATIVE_ABI_FULL_DESCRIPTOR_DIGEST.as_bytes(),
            ),
            capability_catalog_digest: static_slice(capability_catalog_digest),
            package_version: static_slice(PACKAGE_VERSION),
            runtime_catalog: Some(native_runtime_catalog),
            engine_new: Some(native_engine_new),
            engine_try_close: Some(native_engine_try_close),
            execute_collect: Some(native_execute_collect),
            result_free: Some(native_result_free),
            metadata_collect: Some(native_metadata_collect),
            engine_new_with_services: Some(native_engine_new_with_services),
            operation_control_new: Some(native_operation_control_new),
            operation_control_cancel: Some(native_operation_control_cancel),
            operation_control_release: Some(native_operation_control_release),
            execute_collect_controlled: Some(native_execute_collect_controlled),
            engine_new_with_fonts: Some(native_engine_new_with_fonts),
        };
        let initialized_size = MERMAN_NATIVE_API_COMPLETE_PREFIX_SIZES
            .iter()
            .copied()
            .take_while(|size| *size <= output_capacity)
            .last()
            .expect("the validated ABI minimum prefix is always a complete table boundary");
        unsafe {
            ptr::copy_nonoverlapping(
                ptr::addr_of!(api).cast::<u8>(),
                out_api.cast::<u8>(),
                initialized_size as usize,
            );
            ptr::addr_of_mut!((*out_api).struct_size).write(initialized_size);
        }
        Ok(())
    })();

    match result {
        Ok(()) => MERMAN_NATIVE_STATUS_OK,
        Err(failure) => failure.status,
    }
}

unsafe extern "C" fn native_runtime_catalog(
    out_result: *mut MermanNativeResult,
) -> MermanNativeStatus {
    unsafe {
        result_status_boundary(out_result, MERMAN_NATIVE_OPERATION_NONE, || {
            runtime_catalog_impl(out_result)
        })
    }
}

fn runtime_catalog_impl(out_result: *mut MermanNativeResult) -> MermanNativeStatus {
    if let Err(failure) = unsafe { result_is_writable(out_result) } {
        return failure.status;
    }
    match runtime_catalog_bytes() {
        Ok(catalog) => unsafe {
            write_native_result(
                out_result,
                MERMAN_NATIVE_STATUS_OK,
                MERMAN_NATIVE_OPERATION_NONE,
                None,
                Vec::new(),
                catalog.to_vec(),
            )
        },
        Err(failure) => unsafe {
            write_native_failure(out_result, MERMAN_NATIVE_OPERATION_NONE, &failure)
        },
    }
}

unsafe extern "C" fn native_metadata_collect(
    metadata_id: MermanNativeSlice,
    out_result: *mut MermanNativeResult,
) -> MermanNativeStatus {
    unsafe {
        result_status_boundary(out_result, MERMAN_NATIVE_OPERATION_NONE, || {
            metadata_collect_impl(metadata_id, out_result)
        })
    }
}

fn metadata_collect_impl(
    metadata_id: MermanNativeSlice,
    out_result: *mut MermanNativeResult,
) -> MermanNativeStatus {
    if let Err(failure) = unsafe { result_is_writable(out_result) } {
        return failure.status;
    }
    if let Err(failure) = validate_disjoint_storage(
        metadata_id.data,
        metadata_id.len,
        "metadata_id",
        out_result.cast::<u8>(),
        size_of::<MermanNativeResult>(),
        "out_result",
    ) {
        return failure.status;
    }
    if let Err(failure) = validate_native_slice_shape(metadata_id, "metadata_id") {
        return unsafe { write_native_failure(out_result, MERMAN_NATIVE_OPERATION_NONE, &failure) };
    }

    let outcome = (|| {
        let metadata_id = unsafe { native_slice_bytes(metadata_id, "metadata_id") }?;
        let metadata_id = std::str::from_utf8(metadata_id).map_err(|error| {
            NativeFailure::new(
                MERMAN_NATIVE_STATUS_UTF8_ERROR,
                format!("metadata_id must be valid UTF-8: {error}"),
            )
        })?;
        native_artifact_contract()
            .metadata_json(metadata_id)
            .map_err(native_failure_from_binding)
    })();

    match outcome {
        Ok(metadata) => unsafe {
            write_native_result(
                out_result,
                MERMAN_NATIVE_STATUS_OK,
                MERMAN_NATIVE_OPERATION_NONE,
                None,
                Vec::new(),
                metadata,
            )
        },
        Err(failure) => unsafe {
            write_native_failure(out_result, MERMAN_NATIVE_OPERATION_NONE, &failure)
        },
    }
}

unsafe extern "C" fn native_engine_new(
    config: *const MermanNativeEngineConfig,
    out_engine: *mut MermanNativeEngineToken,
    out_result: *mut MermanNativeResult,
) -> MermanNativeStatus {
    unsafe {
        result_status_boundary(out_result, MERMAN_NATIVE_OPERATION_NONE, || {
            engine_new_impl(config, out_engine, out_result)
        })
    }
}

unsafe fn engine_new_impl(
    config: *const MermanNativeEngineConfig,
    out_engine: *mut MermanNativeEngineToken,
    out_result: *mut MermanNativeResult,
) -> MermanNativeStatus {
    if let Err(failure) = unsafe { result_is_writable(out_result) } {
        return failure.status;
    }
    let config_ptr = config;
    let out_engine_storage_len = if out_engine.is_null() {
        0
    } else {
        size_of::<MermanNativeEngineToken>()
    };
    let mut early_output_failure = out_engine.is_null().then(|| {
        NativeFailure::new(
            MERMAN_NATIVE_STATUS_INVALID_ARGUMENT,
            "out_engine must not be null",
        )
    });
    let fixed_storage_validation = (|| {
        validate_disjoint_storage(
            out_engine.cast::<u8>(),
            out_engine_storage_len,
            "out_engine",
            out_result.cast::<u8>(),
            size_of::<MermanNativeResult>(),
            "out_result",
        )?;
        validate_disjoint_storage(
            config_ptr.cast::<u8>(),
            size_of::<MermanNativeEngineConfig>(),
            "config",
            out_engine.cast::<u8>(),
            out_engine_storage_len,
            "out_engine",
        )?;
        validate_disjoint_storage(
            config_ptr.cast::<u8>(),
            size_of::<MermanNativeEngineConfig>(),
            "config",
            out_result.cast::<u8>(),
            size_of::<MermanNativeResult>(),
            "out_result",
        )
    })();
    if let Err(failure) = fixed_storage_validation {
        return failure.status;
    }
    if early_output_failure.is_none()
        && let Err(failure) = validate_pointer_alignment(out_engine, "out_engine")
    {
        early_output_failure = Some(failure);
    }
    let config = match unsafe { read_engine_config(config_ptr) } {
        Ok(config) => config,
        Err(failure) => {
            if early_output_failure.is_some() {
                return failure.status;
            }
            return unsafe {
                write_native_failure(out_result, MERMAN_NATIVE_OPERATION_NONE, &failure)
            };
        }
    };
    let storage_validation = (|| {
        validate_disjoint_storage(
            config_ptr.cast::<u8>(),
            size_of::<MermanNativeEngineConfig>(),
            "config",
            config.options_json.data,
            config.options_json.len,
            "config.options_json",
        )?;
        validate_disjoint_storage(
            config.options_json.data,
            config.options_json.len,
            "config.options_json",
            out_engine.cast::<u8>(),
            out_engine_storage_len,
            "out_engine",
        )?;
        validate_disjoint_storage(
            config.options_json.data,
            config.options_json.len,
            "config.options_json",
            out_result.cast::<u8>(),
            size_of::<MermanNativeResult>(),
            "out_result",
        )
    })();
    if let Err(failure) = storage_validation {
        return failure.status;
    }
    if let Some(failure) = early_output_failure.take() {
        return unsafe { write_native_failure(out_result, MERMAN_NATIVE_OPERATION_NONE, &failure) };
    }
    if let Err(failure) = validate_native_slice_shape(config.options_json, "config.options_json") {
        return unsafe { write_native_failure(out_result, MERMAN_NATIVE_OPERATION_NONE, &failure) };
    }
    if unsafe { ptr::read(out_engine) } != 0 {
        let failure = NativeFailure::new(
            MERMAN_NATIVE_STATUS_INVALID_ARGUMENT,
            "out_engine must be initialized to zero",
        );
        return unsafe { write_native_failure(out_result, MERMAN_NATIVE_OPERATION_NONE, &failure) };
    }
    let outcome = (|| {
        let options_json =
            unsafe { native_slice_bytes(config.options_json, "config.options_json") }?;
        let state = create_native_engine_state(config, options_json, BindingEngineServices::new())?;
        unsafe { publish_native_engine_result(state, out_engine, out_result, "engine-new") }
    })();

    match outcome {
        Ok(status) => status,
        Err(failure) => unsafe {
            write_native_failure(out_result, MERMAN_NATIVE_OPERATION_NONE, &failure)
        },
    }
}

unsafe extern "C" fn native_engine_new_with_services(
    config: *const MermanNativeEngineServicesConfig,
    out_engine: *mut MermanNativeEngineToken,
    out_result: *mut MermanNativeResult,
) -> MermanNativeStatus {
    unsafe {
        result_status_boundary(out_result, MERMAN_NATIVE_OPERATION_NONE, || {
            engine_new_with_services_impl(config, out_engine, out_result)
        })
    }
}

unsafe extern "C" fn native_engine_new_with_fonts(
    config: *const MermanNativeFontEngineConfig,
    out_engine: *mut MermanNativeEngineToken,
    out_result: *mut MermanNativeResult,
) -> MermanNativeStatus {
    unsafe {
        result_status_boundary(out_result, MERMAN_NATIVE_OPERATION_NONE, || {
            engine_new_with_fonts_impl(config, out_engine, out_result)
        })
    }
}

unsafe fn engine_new_with_fonts_impl(
    config_ptr: *const MermanNativeFontEngineConfig,
    out_engine: *mut MermanNativeEngineToken,
    out_result: *mut MermanNativeResult,
) -> MermanNativeStatus {
    if let Err(failure) = unsafe { result_is_writable(out_result) } {
        return failure.status;
    }
    let out_engine_len = if out_engine.is_null() {
        0
    } else {
        size_of::<MermanNativeEngineToken>()
    };
    let outputs = [
        (
            out_engine.cast::<u8>().cast_const(),
            out_engine_len,
            "out_engine",
        ),
        (
            out_result.cast::<u8>().cast_const(),
            size_of::<MermanNativeResult>(),
            "out_result",
        ),
    ];
    let fixed_storage_validation = (|| {
        validate_disjoint_storage(
            outputs[0].0,
            outputs[0].1,
            outputs[0].2,
            outputs[1].0,
            outputs[1].1,
            outputs[1].2,
        )?;
        for (output, len, name) in outputs {
            validate_disjoint_storage(
                config_ptr.cast(),
                size_of::<MermanNativeFontEngineConfig>(),
                "config",
                output,
                len,
                name,
            )?;
        }
        if config_ptr.is_null() {
            return Err(NativeFailure::new(
                MERMAN_NATIVE_STATUS_INVALID_ARGUMENT,
                "config must not be null",
            ));
        }
        validate_pointer_alignment(config_ptr, "config")?;
        validate_struct_size::<MermanNativeFontEngineConfig>(
            unsafe { read_record_struct_size(config_ptr) },
            "config",
        )
    })();
    if let Err(failure) = fixed_storage_validation {
        return failure.status;
    }
    let config = unsafe { ptr::read(config_ptr) };
    let input_storage_validation = (|| {
        validate_disjoint_storage(
            config_ptr.cast(),
            size_of::<MermanNativeFontEngineConfig>(),
            "config",
            config.options_json.data,
            config.options_json.len,
            "config.options_json",
        )?;
        for (output, len, name) in outputs {
            validate_disjoint_storage(
                config.options_json.data,
                config.options_json.len,
                "config.options_json",
                output,
                len,
                name,
            )?;
            validate_declared_record_array_disjoint(
                config.fonts,
                config.font_count,
                "config.fonts",
                output,
                len,
                name,
            )?;
        }
        validate_declared_record_array_disjoint(
            config.fonts,
            config.font_count,
            "config.fonts",
            config_ptr.cast(),
            size_of::<MermanNativeFontEngineConfig>(),
            "config",
        )?;
        validate_declared_record_array_disjoint(
            config.fonts,
            config.font_count,
            "config.fonts",
            config.options_json.data,
            config.options_json.len,
            "config.options_json",
        )
    })();
    if let Err(failure) = input_storage_validation {
        return failure.status;
    }

    let mut deferred_failure = None;
    if let Err(failure) = validate_native_slice_shape(config.options_json, "config.options_json") {
        defer_first_failure(&mut deferred_failure, failure);
    }
    if config.use_bundled_fonts > 1 {
        defer_first_failure(
            &mut deferred_failure,
            NativeFailure::new(
                MERMAN_NATIVE_STATUS_INVALID_ARGUMENT,
                "config.use_bundled_fonts must be zero or one",
            ),
        );
    }

    #[cfg(feature = "native-fonts")]
    let fonts = {
        let limits = NativeFontLimits::default();
        let mut fonts = Vec::new();
        if config.font_count > limits.max_files {
            defer_first_failure(
                &mut deferred_failure,
                native_failure_from_font(NativeFontError::Limit {
                    resource: "files",
                    actual: config.font_count,
                    limit: limits.max_files,
                }),
            );
        } else if config.font_count != 0 && config.fonts.is_null() {
            defer_first_failure(
                &mut deferred_failure,
                NativeFailure::new(
                    MERMAN_NATIVE_STATUS_INVALID_ARGUMENT,
                    "config.fonts must not be null when font_count is non-zero",
                ),
            );
        } else {
            let array_len = match validate_record_array_range(
                config.fonts,
                config.font_count,
                "config.fonts",
            ) {
                Ok(len) => len,
                Err(failure) => return failure.status,
            };
            if config.font_count != 0
                && let Err(failure) = validate_pointer_alignment(config.fonts, "config.fonts")
            {
                return failure.status;
            }
            for index in 0..config.font_count {
                let font_ptr = unsafe { config.fonts.add(index) };
                if let Err(failure) = validate_struct_size::<MermanNativeSlice>(
                    unsafe { read_record_struct_size(font_ptr) },
                    "config.fonts[]",
                ) {
                    defer_first_status_only_failure(&mut deferred_failure, failure);
                    continue;
                }
                let font = unsafe { ptr::read(font_ptr) };
                let storage_validation: Result<(), NativeFailure> = (|| {
                    validate_disjoint_storage(
                        config_ptr.cast(),
                        size_of::<MermanNativeFontEngineConfig>(),
                        "config",
                        font.data,
                        font.len,
                        "config.fonts[].data",
                    )?;
                    validate_disjoint_storage(
                        config.fonts.cast(),
                        array_len,
                        "config.fonts",
                        font.data,
                        font.len,
                        "config.fonts[].data",
                    )?;
                    for (output, len, name) in outputs {
                        validate_disjoint_storage(
                            font.data,
                            font.len,
                            "config.fonts[].data",
                            output,
                            len,
                            name,
                        )?;
                    }
                    Ok(())
                })();
                if let Err(failure) = storage_validation {
                    return failure.status;
                }
                if let Err(failure) = validate_native_slice_shape(font, "config.fonts[]") {
                    defer_first_failure(&mut deferred_failure, failure);
                }
                fonts.push(font);
            }
        }
        if let Some(failure) = deferred_failure.as_ref()
            && !failure.result_write_is_safe
        {
            return failure.failure.status;
        }
        fonts
    };

    let output_validation = if out_engine.is_null() {
        Err(NativeFailure::new(
            MERMAN_NATIVE_STATUS_INVALID_ARGUMENT,
            "out_engine must not be null",
        ))
    } else {
        validate_pointer_alignment(out_engine, "out_engine").and_then(|()| {
            if unsafe { ptr::read(out_engine) } == 0 {
                Ok(())
            } else {
                Err(NativeFailure::new(
                    MERMAN_NATIVE_STATUS_INVALID_ARGUMENT,
                    "out_engine must be initialized to zero",
                ))
            }
        })
    };
    if let Err(failure) = output_validation {
        return unsafe { write_native_failure(out_result, MERMAN_NATIVE_OPERATION_NONE, &failure) };
    }
    if let Some(failure) = deferred_failure {
        return unsafe {
            write_native_failure(out_result, MERMAN_NATIVE_OPERATION_NONE, &failure.failure)
        };
    }

    #[cfg(not(feature = "native-fonts"))]
    {
        let failure = NativeFailure::missing_capability(
            "native-fonts",
            "memory-font construction requires an artifact with the native-fonts capability",
        );
        unsafe { write_native_failure(out_result, MERMAN_NATIVE_OPERATION_NONE, &failure) }
    }
    #[cfg(feature = "native-fonts")]
    {
        let outcome = (|| {
            let mut bundled = Vec::new();
            if config.use_bundled_fonts != 0 {
                #[cfg(feature = "bundled-fonts")]
                bundled.extend(merman::svg::export::bundled_font_data());
                #[cfg(not(feature = "bundled-fonts"))]
                return Err(NativeFailure::missing_capability(
                    "native-fonts",
                    "bundled fallback requires an artifact built with bundled-fonts",
                ));
            }
            let limits = NativeFontLimits::default();
            let count = fonts.len() + bundled.len();
            if count > limits.max_files {
                return Err(native_failure_from_font(NativeFontError::Limit {
                    resource: "files",
                    actual: count,
                    limit: limits.max_files,
                }));
            }
            let mut bytes = 0usize;
            for len in fonts
                .iter()
                .map(|font| font.len)
                .chain(bundled.iter().map(Vec::len))
            {
                bytes = bytes.saturating_add(len);
                if bytes > limits.max_bytes {
                    return Err(native_failure_from_font(NativeFontError::Limit {
                        resource: "font-bytes",
                        actual: bytes,
                        limit: limits.max_bytes,
                    }));
                }
            }
            let mut owned_fonts = Vec::with_capacity(count);
            for font in fonts {
                owned_fonts.push(unsafe { native_slice_bytes(font, "config.fonts[]") }?.to_vec());
            }
            owned_fonts.append(&mut bundled);
            let control = OperationControl::new();
            let services =
                BindingEngineServices::from_strict_native_font_bytes(owned_fonts, limits, &control)
                    .map_err(native_failure_from_font)?;
            let options_json =
                unsafe { native_slice_bytes(config.options_json, "config.options_json") }?;
            let engine_config = MermanNativeEngineConfig {
                struct_size: native_struct_size::<MermanNativeEngineConfig>(),
                options_json: config.options_json,
                text_measure: None,
                text_measure_user_data: ptr::null_mut(),
            };
            let state = create_native_engine_state(engine_config, options_json, services)?;
            unsafe {
                publish_native_engine_result(state, out_engine, out_result, "engine-new-with-fonts")
            }
        })();
        match outcome {
            Ok(status) => status,
            Err(failure) => unsafe {
                write_native_failure(out_result, MERMAN_NATIVE_OPERATION_NONE, &failure)
            },
        }
    }
}

#[cfg(feature = "native-fonts")]
fn native_failure_from_font(error: NativeFontError) -> NativeFailure {
    let status = if matches!(error, NativeFontError::Limit { .. }) {
        MERMAN_NATIVE_STATUS_RESOURCE_LIMIT_EXCEEDED
    } else {
        MERMAN_NATIVE_STATUS_INVALID_ARGUMENT
    };
    NativeFailure::new(status, error.to_string())
}

unsafe fn engine_new_with_services_impl(
    config: *const MermanNativeEngineServicesConfig,
    out_engine: *mut MermanNativeEngineToken,
    out_result: *mut MermanNativeResult,
) -> MermanNativeStatus {
    let config_ptr = config;
    if let Err(failure) = unsafe { result_is_writable(out_result) } {
        return failure.status;
    }
    let out_engine_storage_len = if out_engine.is_null() {
        0
    } else {
        size_of::<MermanNativeEngineToken>()
    };
    let mut early_output_failure = out_engine.is_null().then(|| {
        NativeFailure::new(
            MERMAN_NATIVE_STATUS_INVALID_ARGUMENT,
            "out_engine must not be null",
        )
    });

    let fixed_storage_validation = (|| {
        validate_disjoint_storage(
            out_engine.cast::<u8>(),
            out_engine_storage_len,
            "out_engine",
            out_result.cast::<u8>(),
            size_of::<MermanNativeResult>(),
            "out_result",
        )?;
        validate_disjoint_storage(
            config.cast::<u8>(),
            size_of::<MermanNativeEngineServicesConfig>(),
            "config",
            out_engine.cast::<u8>(),
            out_engine_storage_len,
            "out_engine",
        )?;
        validate_disjoint_storage(
            config.cast::<u8>(),
            size_of::<MermanNativeEngineServicesConfig>(),
            "config",
            out_result.cast::<u8>(),
            size_of::<MermanNativeResult>(),
            "out_result",
        )
    })();
    if let Err(failure) = fixed_storage_validation {
        return failure.status;
    }
    if early_output_failure.is_none()
        && let Err(failure) = validate_pointer_alignment(out_engine, "out_engine")
    {
        early_output_failure = Some(failure);
    }

    let config = match unsafe { read_engine_services_config(config_ptr) } {
        Ok(config) => config,
        Err(failure) => return failure.status,
    };
    let engine_config = config.engine_config;

    let options_storage_validation = (|| {
        validate_disjoint_storage(
            config_ptr.cast::<u8>(),
            size_of::<MermanNativeEngineServicesConfig>(),
            "config",
            engine_config.options_json.data,
            engine_config.options_json.len,
            "config.engine_config.options_json",
        )?;
        validate_disjoint_storage(
            engine_config.options_json.data,
            engine_config.options_json.len,
            "config.engine_config.options_json",
            out_engine.cast::<u8>(),
            out_engine_storage_len,
            "out_engine",
        )?;
        validate_disjoint_storage(
            engine_config.options_json.data,
            engine_config.options_json.len,
            "config.engine_config.options_json",
            out_result.cast::<u8>(),
            size_of::<MermanNativeResult>(),
            "out_result",
        )
    })();
    if let Err(failure) = options_storage_validation {
        return failure.status;
    }
    let mut deferred_failure = None;
    if let Err(failure) = validate_struct_size::<MermanNativeEngineConfig>(
        engine_config.struct_size,
        "config.engine_config",
    ) {
        defer_first_failure(&mut deferred_failure, failure);
    }
    if let Err(failure) = validate_native_slice_shape(
        engine_config.options_json,
        "config.engine_config.options_json",
    ) {
        defer_first_failure(&mut deferred_failure, failure);
    }

    let declared_array_storage_validation = (|| {
        validate_declared_record_array_disjoint(
            config.icon_packs,
            config.icon_pack_count,
            "config.icon_packs",
            config_ptr.cast::<u8>(),
            size_of::<MermanNativeEngineServicesConfig>(),
            "config",
        )?;
        validate_declared_record_array_disjoint(
            config.icon_packs,
            config.icon_pack_count,
            "config.icon_packs",
            engine_config.options_json.data,
            engine_config.options_json.len,
            "config.engine_config.options_json",
        )?;
        validate_declared_record_array_disjoint(
            config.icon_packs,
            config.icon_pack_count,
            "config.icon_packs",
            out_engine.cast::<u8>(),
            out_engine_storage_len,
            "out_engine",
        )?;
        validate_declared_record_array_disjoint(
            config.icon_packs,
            config.icon_pack_count,
            "config.icon_packs",
            out_result.cast::<u8>(),
            size_of::<MermanNativeResult>(),
            "out_result",
        )
    })();
    if let Err(failure) = declared_array_storage_validation {
        return failure.status;
    }

    #[cfg(not(feature = "svg"))]
    {
        if let Some(failure) = early_output_failure.take() {
            return unsafe {
                write_native_failure(out_result, MERMAN_NATIVE_OPERATION_NONE, &failure)
            };
        }
        if let Some(failure) = deferred_failure.take() {
            return unsafe {
                write_native_failure(out_result, MERMAN_NATIVE_OPERATION_NONE, &failure.failure)
            };
        }
        if config.icon_pack_count != 0 {
            let failure = NativeFailure::missing_capability(
                "svg",
                "icon registry construction requires an artifact with the svg capability",
            );
            return unsafe {
                write_native_failure(out_result, MERMAN_NATIVE_OPERATION_NONE, &failure)
            };
        }
    }

    #[cfg(feature = "svg")]
    let icon_packs = {
        if config.icon_pack_count > NATIVE_ICON_PACK_RECORD_LIMIT {
            defer_first_failure(
                &mut deferred_failure,
                native_icon_pack_count_limit_failure(config.icon_pack_count),
            );
        }

        let mut native_packs = Vec::new();
        if config.icon_pack_count <= NATIVE_ICON_PACK_RECORD_LIMIT {
            let icon_pack_byte_len = match validate_record_array_range(
                config.icon_packs,
                config.icon_pack_count,
                "config.icon_packs",
            ) {
                Ok(byte_len) => byte_len,
                Err(failure) => return failure.status,
            };
            let icon_pack_array = config.icon_packs.cast::<u8>();
            if config.icon_pack_count != 0
                && let Err(failure) =
                    validate_pointer_alignment(config.icon_packs, "config.icon_packs")
            {
                return failure.status;
            }

            native_packs.reserve(config.icon_pack_count);
            for index in 0..config.icon_pack_count {
                let pack_ptr = unsafe { config.icon_packs.add(index) };
                match validate_struct_size::<MermanNativeIconPack>(
                    unsafe { read_record_struct_size(pack_ptr) },
                    &format!("config.icon_packs[{index}]"),
                ) {
                    Ok(()) => native_packs.push(Some(unsafe { ptr::read(pack_ptr) })),
                    Err(failure) => {
                        defer_first_status_only_failure(&mut deferred_failure, failure);
                        native_packs.push(None);
                    }
                }
            }

            for (index, pack) in native_packs.iter().enumerate() {
                let Some(pack) = pack else {
                    continue;
                };
                for (slice, name) in [
                    (pack.json, format!("config.icon_packs[{index}].json")),
                    (
                        pack.registration_name,
                        format!("config.icon_packs[{index}].registration_name"),
                    ),
                ] {
                    let slice_storage_validation = (|| {
                        validate_disjoint_storage(
                            config_ptr.cast::<u8>(),
                            size_of::<MermanNativeEngineServicesConfig>(),
                            "config",
                            slice.data,
                            slice.len,
                            &name,
                        )?;
                        validate_disjoint_storage(
                            icon_pack_array,
                            icon_pack_byte_len,
                            "config.icon_packs",
                            slice.data,
                            slice.len,
                            &name,
                        )?;
                        validate_disjoint_storage(
                            slice.data,
                            slice.len,
                            &name,
                            out_engine.cast::<u8>(),
                            out_engine_storage_len,
                            "out_engine",
                        )?;
                        validate_disjoint_storage(
                            slice.data,
                            slice.len,
                            &name,
                            out_result.cast::<u8>(),
                            size_of::<MermanNativeResult>(),
                            "out_result",
                        )
                    })();
                    if let Err(failure) = slice_storage_validation {
                        return failure.status;
                    }
                    if let Err(failure) = validate_native_slice_shape(slice, &name) {
                        defer_first_failure(&mut deferred_failure, failure);
                    }
                }
            }
        }

        if let Some(failure) = deferred_failure.as_ref()
            && !failure.result_write_is_safe
        {
            return failure.failure.status;
        }
        if let Some(failure) = early_output_failure.take() {
            return unsafe {
                write_native_failure(out_result, MERMAN_NATIVE_OPERATION_NONE, &failure)
            };
        }
        if let Some(failure) = deferred_failure.take() {
            return unsafe {
                write_native_failure(out_result, MERMAN_NATIVE_OPERATION_NONE, &failure.failure)
            };
        }

        // Keep the safety checks above authoritative, then complete the whole length-only resource
        // preflight before constructing borrowed byte slices or decoding registration-name UTF-8.
        if let Err(failure) = preflight_native_icon_pack_resource_limits(&native_packs) {
            return unsafe {
                write_native_failure(out_result, MERMAN_NATIVE_OPERATION_NONE, &failure)
            };
        }

        let mut packs = Vec::with_capacity(native_packs.len());
        for (index, pack) in native_packs.into_iter().enumerate() {
            let pack = pack.expect("validated native icon-pack records are complete");
            let json = match unsafe {
                native_slice_bytes(pack.json, &format!("config.icon_packs[{index}].json"))
            } {
                Ok(json) => json,
                Err(failure) => {
                    return unsafe {
                        write_native_failure(out_result, MERMAN_NATIVE_OPERATION_NONE, &failure)
                    };
                }
            };
            let registration_name_bytes = match unsafe {
                native_slice_bytes(
                    pack.registration_name,
                    &format!("config.icon_packs[{index}].registration_name"),
                )
            } {
                Ok(bytes) => bytes,
                Err(failure) => {
                    return unsafe {
                        write_native_failure(out_result, MERMAN_NATIVE_OPERATION_NONE, &failure)
                    };
                }
            };
            let registration_name = if registration_name_bytes.is_empty() {
                None
            } else {
                match std::str::from_utf8(registration_name_bytes) {
                    Ok(name) => Some(name),
                    Err(error) => {
                        let failure = native_failure_from_binding(
                            BindingError::icon_registry_invalid_utf8(
                                index,
                                format!(
                                    "config.icon_packs[{index}].registration_name must be valid UTF-8: {error}"
                                ),
                            ),
                        );
                        return unsafe {
                            write_native_failure(out_result, MERMAN_NATIVE_OPERATION_NONE, &failure)
                        };
                    }
                }
            };
            let pack = match registration_name {
                Some(name) => IconPack::new(json).with_registration_name(name),
                None => IconPack::new(json),
            };
            packs.push(pack);
        }
        packs
    };

    if unsafe { ptr::read(out_engine) } != 0 {
        let failure = NativeFailure::new(
            MERMAN_NATIVE_STATUS_INVALID_ARGUMENT,
            "out_engine must be initialized to zero",
        );
        return unsafe { write_native_failure(out_result, MERMAN_NATIVE_OPERATION_NONE, &failure) };
    }
    if engine_config.text_measure.is_none() && !engine_config.text_measure_user_data.is_null() {
        let failure = NativeFailure::new(
            MERMAN_NATIVE_STATUS_INVALID_ARGUMENT,
            "config.engine_config.text_measure_user_data must be null when text_measure is null",
        );
        return unsafe { write_native_failure(out_result, MERMAN_NATIVE_OPERATION_NONE, &failure) };
    }

    let outcome = (|| {
        let options_json = unsafe {
            native_slice_bytes(
                engine_config.options_json,
                "config.engine_config.options_json",
            )
        }?;
        let services = BindingEngineServices::new();
        #[cfg(feature = "svg")]
        let services = if icon_packs.is_empty() {
            services
        } else {
            let icon_registry =
                build_icon_registry(icon_packs).map_err(native_failure_from_binding)?;
            services.with_icon_registry(icon_registry)
        };
        let state = create_native_engine_state(engine_config, options_json, services)?;
        unsafe {
            publish_native_engine_result(state, out_engine, out_result, "engine-new-with-services")
        }
    })();

    match outcome {
        Ok(status) => status,
        Err(failure) => unsafe {
            write_native_failure(out_result, MERMAN_NATIVE_OPERATION_NONE, &failure)
        },
    }
}

#[cfg(feature = "svg")]
fn preflight_native_icon_pack_resource_limits(
    native_packs: &[Option<MermanNativeIconPack>],
) -> Result<(), NativeFailure> {
    use IconRegistryResourceLimitId::{MaxInputBytes, MaxPackBytes, MaxPrefixBytes};

    let mut input_bytes = 0usize;
    for (index, pack) in native_packs.iter().enumerate() {
        let pack = pack
            .as_ref()
            .expect("validated native icon-pack records are complete");

        let pack_bytes = u64::try_from(pack.json.len).unwrap_or(u64::MAX);
        if pack_bytes > MaxPackBytes.fixed_value() {
            return Err(native_failure_from_binding(
                BindingError::icon_registry_resource_limit(
                    MaxPackBytes,
                    pack_bytes,
                    Some(index),
                    "icon pack bytes exceed the fixed per-pack ceiling",
                ),
            ));
        }

        let next_input_bytes = input_bytes.checked_add(pack.json.len).ok_or_else(|| {
            native_failure_from_binding(BindingError::icon_registry_resource_limit(
                MaxInputBytes,
                u64::MAX,
                Some(index),
                "aggregate icon pack byte accounting overflowed",
            ))
        })?;
        let next_input_bytes_u64 = u64::try_from(next_input_bytes).unwrap_or(u64::MAX);
        if next_input_bytes_u64 > MaxInputBytes.fixed_value() {
            return Err(native_failure_from_binding(
                BindingError::icon_registry_resource_limit(
                    MaxInputBytes,
                    next_input_bytes_u64,
                    Some(index),
                    "aggregate icon pack bytes exceed the fixed registry ceiling",
                ),
            ));
        }

        let prefix_bytes = u64::try_from(pack.registration_name.len).unwrap_or(u64::MAX);
        if prefix_bytes > MaxPrefixBytes.fixed_value() {
            return Err(native_failure_from_binding(
                BindingError::icon_registry_resource_limit(
                    MaxPrefixBytes,
                    prefix_bytes,
                    Some(index),
                    "icon registry registration name exceeds the fixed byte ceiling",
                ),
            ));
        }

        input_bytes = next_input_bytes;
    }

    Ok(())
}

#[cfg(feature = "svg")]
fn native_icon_pack_count_limit_failure(actual: usize) -> NativeFailure {
    let limit = IconRegistryResourceLimitId::MaxPacks;
    native_failure_from_binding(BindingError::icon_registry_resource_limit(
        limit,
        u64::try_from(actual).unwrap_or(u64::MAX),
        None,
        format!(
            "icon registry pack count {actual} exceeds the fixed limit {}",
            limit.fixed_value()
        ),
    ))
}

fn create_native_engine_state(
    config: MermanNativeEngineConfig,
    options_json: &[u8],
    mut services: BindingEngineServices,
) -> Result<Arc<NativeEngineState>, NativeFailure> {
    let admission = BindingEngineAdmission::new(if config.text_measure.is_some() {
        BindingEngineAdmissionMode::HostCallback
    } else {
        BindingEngineAdmissionMode::Concurrent
    });

    #[cfg(feature = "svg")]
    if let Some(callback) = config.text_measure {
        let measurer = NativeHostTextMeasurer::new(
            callback,
            config.text_measure_user_data,
            Arc::clone(&admission),
        );
        services = services.with_host_text_measurer(Arc::new(measurer));
    }

    #[cfg(not(feature = "svg"))]
    {
        let _ = &mut services;
        if config.text_measure.is_some() {
            return Err(NativeFailure::missing_capability(
                "svg",
                "host text measurement requires an artifact with the svg capability",
            ));
        }
    }

    let engine = native_artifact_contract()
        .create_engine_with_services(options_json, services)
        .map_err(native_failure_from_binding)?;
    Ok(Arc::new(NativeEngineState { engine, admission }))
}

struct PendingNativeEngine<'registry> {
    registry: &'registry Mutex<NativeEngineRegistry>,
    counter: Option<u64>,
}

impl<'registry> PendingNativeEngine<'registry> {
    fn reserve(
        registry: &'registry Mutex<NativeEngineRegistry>,
        state: Arc<NativeEngineState>,
    ) -> Result<Self, NativeEngineEntry> {
        let counter = {
            let mut registry = registry
                .lock()
                .unwrap_or_else(|poisoned| poisoned.into_inner());
            registry.try_insert(NativeEngineEntry {
                publication: NativeEnginePublication::Reserved,
                state,
            })
        }?;
        Ok(Self {
            registry,
            counter: Some(counter),
        })
    }

    unsafe fn commit(mut self, out_engine: *mut MermanNativeEngineToken) {
        let counter = self
            .counter
            .expect("a pending native engine owns one reserved token");
        let token = encode_engine_token(counter);
        let mut registry = self
            .registry
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        let entry = registry
            .get_mut(counter)
            .expect("a pending native engine owns one reserved token");
        assert_eq!(
            entry.publication,
            NativeEnginePublication::Reserved,
            "a native engine reservation may be committed exactly once"
        );
        // Everything that can fail is complete before this point. The caller-visible token and
        // registry visibility are committed in the same critical section, and publication is the
        // final infallible state transition.
        unsafe { ptr::write(out_engine, token) };
        entry.publication = NativeEnginePublication::Published;
        self.counter = None;
    }
}

impl Drop for PendingNativeEngine<'_> {
    fn drop(&mut self) {
        let Some(counter) = self.counter.take() else {
            return;
        };
        let cancelled = {
            let mut registry = self
                .registry
                .lock()
                .unwrap_or_else(|poisoned| poisoned.into_inner());
            if registry
                .get(counter)
                .is_some_and(|entry| entry.publication == NativeEnginePublication::Reserved)
            {
                registry.remove(counter)
            } else {
                None
            }
        };
        if let Some(entry) = cancelled {
            let _ = entry.state.admission.try_close();
            drop(entry);
        }
    }
}

unsafe fn publish_native_engine_result(
    state: Arc<NativeEngineState>,
    out_engine: *mut MermanNativeEngineToken,
    out_result: *mut MermanNativeResult,
    success_operation: &str,
) -> Result<MermanNativeStatus, NativeFailure> {
    unsafe {
        publish_native_engine_result_with_writer(state, out_engine, out_result, |out_result| {
            write_native_result(
                out_result,
                MERMAN_NATIVE_STATUS_OK,
                MERMAN_NATIVE_OPERATION_NONE,
                None,
                Vec::new(),
                native_success_json(success_operation),
            )
        })
    }
}

unsafe fn publish_native_engine_result_with_writer(
    state: Arc<NativeEngineState>,
    out_engine: *mut MermanNativeEngineToken,
    out_result: *mut MermanNativeResult,
    write_success: impl FnOnce(*mut MermanNativeResult) -> MermanNativeStatus,
) -> Result<MermanNativeStatus, NativeFailure> {
    unsafe {
        publish_native_engine_result_with_writer_in_registry(
            &ENGINE_REGISTRY,
            state,
            out_engine,
            out_result,
            write_success,
        )
    }
}

unsafe fn publish_native_engine_result_with_writer_in_registry(
    registry: &Mutex<NativeEngineRegistry>,
    state: Arc<NativeEngineState>,
    out_engine: *mut MermanNativeEngineToken,
    out_result: *mut MermanNativeResult,
    write_success: impl FnOnce(*mut MermanNativeResult) -> MermanNativeStatus,
) -> Result<MermanNativeStatus, NativeFailure> {
    let pending = PendingNativeEngine::reserve(registry, state).map_err(|rejected| {
        drop(rejected);
        NativeFailure::new(
            MERMAN_NATIVE_STATUS_INTERNAL_ERROR,
            "native engine token space is exhausted",
        )
    })?;
    let status = write_success(out_result);
    if status != MERMAN_NATIVE_STATUS_OK {
        return Ok(status);
    }
    unsafe { pending.commit(out_engine) };
    Ok(status)
}

unsafe extern "C" fn native_engine_try_close(
    engine: MermanNativeEngineToken,
) -> MermanNativeStatus {
    status_boundary(|| {
        if !token_has_domain(engine, MERMAN_NATIVE_ENGINE_TOKEN_DOMAIN_TAG) {
            return MERMAN_NATIVE_STATUS_INVALID_ENGINE;
        }
        let counter = engine >> MERMAN_NATIVE_TOKEN_COUNTER_SHIFT;
        let retired = {
            let mut registry = ENGINE_REGISTRY
                .lock()
                .unwrap_or_else(|poisoned| poisoned.into_inner());
            let Some(state) = registry
                .get(counter)
                .and_then(NativeEngineEntry::published_state)
            else {
                return MERMAN_NATIVE_STATUS_INVALID_ENGINE;
            };
            match state.admission.try_close() {
                Ok(()) => {
                    let retired = registry.remove(counter);
                    debug_assert!(retired.is_some(), "close retires the acquired engine token");
                    retired
                }
                Err(error) => return native_failure_from_admission(error).status,
            }
        };
        drop(retired);
        MERMAN_NATIVE_STATUS_OK
    })
}

unsafe extern "C" fn native_operation_control_new(
    timeout_ms: u64,
    has_timeout_ms: u8,
    out_control: *mut MermanNativeOperationControlToken,
    out_result: *mut MermanNativeResult,
) -> MermanNativeStatus {
    unsafe {
        result_status_boundary(out_result, MERMAN_NATIVE_OPERATION_NONE, || {
            operation_control_new_impl(timeout_ms, has_timeout_ms, out_control, out_result)
        })
    }
}

unsafe fn operation_control_new_impl(
    timeout_ms: u64,
    has_timeout_ms: u8,
    out_control: *mut MermanNativeOperationControlToken,
    out_result: *mut MermanNativeResult,
) -> MermanNativeStatus {
    if let Err(failure) = unsafe { result_is_writable(out_result) } {
        return failure.status;
    }
    if out_control.is_null() {
        let failure = NativeFailure::new(
            MERMAN_NATIVE_STATUS_INVALID_ARGUMENT,
            "out_control must not be null",
        );
        return unsafe { write_native_failure(out_result, MERMAN_NATIVE_OPERATION_NONE, &failure) };
    }
    if let Err(failure) = validate_pointer_alignment(out_control, "out_control") {
        return failure.status;
    }
    if let Err(failure) = validate_disjoint_storage(
        out_control.cast::<u8>(),
        size_of::<MermanNativeOperationControlToken>(),
        "out_control",
        out_result.cast::<u8>(),
        size_of::<MermanNativeResult>(),
        "out_result",
    ) {
        return failure.status;
    }
    if has_timeout_ms > 1 {
        let failure = NativeFailure::new(
            MERMAN_NATIVE_STATUS_INVALID_ARGUMENT,
            "has_timeout_ms must be 0 or 1",
        );
        return unsafe { write_native_failure(out_result, MERMAN_NATIVE_OPERATION_NONE, &failure) };
    }
    if unsafe { ptr::read(out_control) } != 0 {
        let failure = NativeFailure::new(
            MERMAN_NATIVE_STATUS_INVALID_ARGUMENT,
            "out_control must be initialized to zero",
        );
        return unsafe { write_native_failure(out_result, MERMAN_NATIVE_OPERATION_NONE, &failure) };
    }
    let issued = {
        let mut registry = operation_control_registry();
        let control = OperationControl::new();
        let control = if has_timeout_ms == 1 {
            control.with_deadline(Duration::from_millis(timeout_ms))
        } else {
            control
        };
        registry.try_insert(control)
    };
    let Ok(counter) = issued else {
        let failure = NativeFailure::new(
            MERMAN_NATIVE_STATUS_INTERNAL_ERROR,
            "native operation-control token space is exhausted",
        );
        return unsafe { write_native_failure(out_result, MERMAN_NATIVE_OPERATION_NONE, &failure) };
    };
    let token = (counter << MERMAN_NATIVE_TOKEN_COUNTER_SHIFT)
        | MERMAN_NATIVE_OPERATION_CONTROL_TOKEN_DOMAIN_TAG;
    let status = unsafe {
        write_native_result(
            out_result,
            MERMAN_NATIVE_STATUS_OK,
            MERMAN_NATIVE_OPERATION_NONE,
            None,
            Vec::new(),
            native_success_json("operation-control-new"),
        )
    };
    if status != MERMAN_NATIVE_STATUS_OK {
        drop(retire_operation_control(token));
        return status;
    }
    unsafe { ptr::write(out_control, token) };
    status
}

unsafe extern "C" fn native_operation_control_cancel(
    control: MermanNativeOperationControlToken,
) -> MermanNativeStatus {
    status_boundary(|| {
        if !token_has_domain(control, MERMAN_NATIVE_OPERATION_CONTROL_TOKEN_DOMAIN_TAG) {
            return MERMAN_NATIVE_STATUS_INVALID_ARGUMENT;
        }
        let Some(control) = registered_operation_control(control) else {
            return MERMAN_NATIVE_STATUS_INVALID_ARGUMENT;
        };
        control.cancel();
        MERMAN_NATIVE_STATUS_OK
    })
}

unsafe extern "C" fn native_operation_control_release(
    control: MermanNativeOperationControlToken,
) -> MermanNativeStatus {
    status_boundary(|| {
        if !token_has_domain(control, MERMAN_NATIVE_OPERATION_CONTROL_TOKEN_DOMAIN_TAG) {
            return MERMAN_NATIVE_STATUS_INVALID_ARGUMENT;
        }
        if retire_operation_control(control).is_some() {
            MERMAN_NATIVE_STATUS_OK
        } else {
            MERMAN_NATIVE_STATUS_INVALID_ARGUMENT
        }
    })
}

unsafe extern "C" fn native_execute_collect(
    engine: MermanNativeEngineToken,
    request: *const MermanNativeOperationRequest,
    out_result: *mut MermanNativeResult,
) -> MermanNativeStatus {
    unsafe {
        result_status_boundary(out_result, MERMAN_NATIVE_OPERATION_NONE, || {
            execute_collect_impl(engine, 0, request, out_result)
        })
    }
}

unsafe extern "C" fn native_execute_collect_controlled(
    engine: MermanNativeEngineToken,
    control: MermanNativeOperationControlToken,
    request: *const MermanNativeOperationRequest,
    out_result: *mut MermanNativeResult,
) -> MermanNativeStatus {
    unsafe {
        result_status_boundary(out_result, MERMAN_NATIVE_OPERATION_NONE, || {
            execute_collect_impl(engine, control, request, out_result)
        })
    }
}

unsafe fn execute_collect_impl(
    engine: MermanNativeEngineToken,
    control: MermanNativeOperationControlToken,
    request: *const MermanNativeOperationRequest,
    out_result: *mut MermanNativeResult,
) -> MermanNativeStatus {
    if let Err(failure) = unsafe { result_is_writable(out_result) } {
        return failure.status;
    }
    let request = match unsafe { read_operation_request(request) } {
        Ok(request) => request,
        Err(failure) => {
            return unsafe {
                write_native_failure(out_result, MERMAN_NATIVE_OPERATION_NONE, &failure)
            };
        }
    };
    let operation = normalized_operation(request.operation);
    match execute_with_engine(engine, control, request, |execution| {
        let status = unsafe {
            write_native_result(
                out_result,
                MERMAN_NATIVE_STATUS_OK,
                execution.operation,
                Some(execution.media_type),
                execution.data,
                execution.metadata_json,
            )
        };
        Ok(status)
    }) {
        Ok(status) => status,
        Err(failure) => unsafe { write_native_failure(out_result, operation, &failure) },
    }
}

unsafe extern "C" fn native_result_free(result: *mut MermanNativeResult) {
    unsafe { free_native_result(result) }
}

#[cfg(test)]
mod tests;
