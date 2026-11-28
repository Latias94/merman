#![deny(unsafe_op_in_unsafe_fn)]

//! Native ABI 4 exports for embedding Merman in C-compatible hosts.
//!
//! The only exported C symbol is [`merman_get_native_api`]. Hosts discover a size-tagged
//! function table and execute every operation through the shared binding operation path. No raw Rust
//! allocation or Rust object pointer crosses this boundary.

use merman::OperationControl;
#[cfg(feature = "svg")]
use merman_bindings_core::HostMeasurementResult;
use merman_bindings_core::transport_boundary::catch_transport_unwind;
use merman_bindings_core::transport_handles::{HandleRegistry, OperationControlRegistry};
use merman_bindings_core::{
    BindingEngine, BindingEngineAdmission, BindingEngineAdmissionError, BindingEngineAdmissionMode,
    BindingEngineServices, BindingError, BindingErrorKind, BindingOperationRequest,
    BindingResourceErrorDetails, BindingStatus, OperationKey, ValidatedArtifactContract,
};
use sha2::{Digest, Sha256};
use std::mem::size_of;
use std::ptr;
use std::sync::{Arc, Mutex, MutexGuard, OnceLock};
use std::time::Duration;

use self::engine_config::{
    native_engine_new, native_engine_new_with_fonts, native_engine_new_with_services,
};
#[cfg(any(feature = "svg", test))]
use self::records::borrowed_slice;
use self::records::{
    NativeOperationRequest, native_slice_bytes, native_struct_size, read_operation_request,
    read_record_struct_size, static_slice, validate_disjoint_storage, validate_native_slice_shape,
    validate_pointer_alignment, validate_struct_size,
};
use self::result::{
    free_native_result, native_success_json, normalized_operation, result_is_writable,
    result_status_boundary, write_native_failure, write_native_result,
};

include!("generated/text_measurement_abi.rs");
include!("generated/native_abi.rs");

mod engine_config;
mod records;
mod result;

const PACKAGE_VERSION: &[u8] = env!("CARGO_PKG_VERSION").as_bytes();

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
        .acquire(token >> MERMAN_NATIVE_TOKEN_COUNTER_SHIFT)
        .ok()
}

fn retire_operation_control(token: MermanNativeOperationControlToken) -> Option<OperationControl> {
    operation_control_registry()
        .retire(token >> MERMAN_NATIVE_TOKEN_COUNTER_SHIFT)
        .ok()
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
