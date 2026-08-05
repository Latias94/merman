#![deny(unsafe_op_in_unsafe_fn)]

//! Native ABI 3 exports for embedding Merman in C-compatible hosts.
//!
//! The only exported C symbol is [`merman_get_native_api`]. Hosts discover a size-tagged
//! function table and execute every operation through the shared binding operation path. No raw Rust
//! allocation or Rust object pointer crosses this boundary.

#[cfg(feature = "svg")]
use merman_bindings_core::HostMeasurementResult;
use merman_bindings_core::{
    ArtifactCapabilitySurface, BindingEngine, BindingEngineAdmission, BindingEngineAdmissionError,
    BindingEngineAdmissionMode, BindingError, BindingErrorKind, BindingOperationRequest,
    BindingResourceErrorDetails, BindingStatus, TextMeasurementProviderProjection,
};
use sha2::{Digest, Sha256};
use std::collections::BTreeMap;
use std::mem::size_of;
use std::panic::{AssertUnwindSafe, catch_unwind};
use std::ptr;
use std::sync::{Arc, Mutex, OnceLock};

include!("generated/text_measurement_abi.rs");
include!("generated/abi3.rs");

const PACKAGE_VERSION: &[u8] = env!("CARGO_PKG_VERSION").as_bytes();

#[derive(Debug)]
struct NativeFailure {
    status: MermanNativeStatus,
    kind: BindingErrorKind,
    capability_id: Option<&'static str>,
    resource: Option<BindingResourceErrorDetails>,
    message: String,
}

impl NativeFailure {
    fn new(status: MermanNativeStatus, message: impl Into<String>) -> Self {
        Self {
            status,
            kind: BindingErrorKind::Generic,
            capability_id: None,
            resource: None,
            message: message.into(),
        }
    }

    fn reentrant_call(message: impl Into<String>) -> Self {
        Self {
            status: MERMAN_NATIVE_STATUS_REENTRANT_CALL,
            kind: BindingErrorKind::ReentrantCall,
            capability_id: None,
            resource: None,
            message: message.into(),
        }
    }

    fn with_kind(mut self, kind: BindingErrorKind) -> Self {
        self.kind = kind;
        self
    }

    #[cfg(not(feature = "svg"))]
    fn missing_capability(capability_id: &'static str, message: impl Into<String>) -> Self {
        Self {
            status: MERMAN_NATIVE_STATUS_UNSUPPORTED_OPERATION,
            kind: BindingErrorKind::MissingCapability,
            capability_id: Some(capability_id),
            resource: None,
            message: message.into(),
        }
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

#[derive(Default)]
struct NativeEngineRegistry {
    last_token: MermanNativeEngineToken,
    engines: BTreeMap<MermanNativeEngineToken, Arc<NativeEngineState>>,
}

impl NativeEngineRegistry {
    fn register(
        &mut self,
        engine: Arc<NativeEngineState>,
    ) -> Result<MermanNativeEngineToken, NativeFailure> {
        let token = self.last_token.checked_add(1).ok_or_else(|| {
            NativeFailure::new(
                MERMAN_NATIVE_STATUS_INTERNAL_ERROR,
                "native engine token space is exhausted",
            )
        })?;
        if token == 0 {
            return Err(NativeFailure::new(
                MERMAN_NATIVE_STATUS_INTERNAL_ERROR,
                "native engine token space is exhausted",
            ));
        }
        self.last_token = token;
        let previous = self.engines.insert(token, engine);
        debug_assert!(previous.is_none(), "native engine tokens are never reused");
        Ok(token)
    }

    fn acquire(&self, token: MermanNativeEngineToken) -> Option<Arc<NativeEngineState>> {
        self.engines.get(&token).map(Arc::clone)
    }

    fn retire(&mut self, token: MermanNativeEngineToken) -> Option<Arc<NativeEngineState>> {
        self.engines.remove(&token)
    }
}

#[derive(Default)]
struct NativeAllocationRegistry {
    last_token: u64,
    results: BTreeMap<u64, NativeResultAllocation>,
}

struct NativeResultAllocation {
    data: Vec<u8>,
    metadata_or_error_json: Vec<u8>,
}

impl NativeAllocationRegistry {
    fn register(
        &mut self,
        data: Vec<u8>,
        metadata_or_error_json: Vec<u8>,
    ) -> Result<u64, NativeFailure> {
        let token = self.last_token.checked_add(1).ok_or_else(|| {
            NativeFailure::new(
                MERMAN_NATIVE_STATUS_INTERNAL_ERROR,
                "native result allocation token space is exhausted",
            )
        })?;
        if token == 0 {
            return Err(NativeFailure::new(
                MERMAN_NATIVE_STATUS_INTERNAL_ERROR,
                "native result allocation token space is exhausted",
            ));
        }
        self.last_token = token;
        let previous = self.results.insert(
            token,
            NativeResultAllocation {
                data,
                metadata_or_error_json,
            },
        );
        debug_assert!(
            previous.is_none(),
            "native result allocation tokens are never reused"
        );
        Ok(token)
    }
}

static ENGINE_REGISTRY: OnceLock<Mutex<NativeEngineRegistry>> = OnceLock::new();
static ALLOCATION_REGISTRY: OnceLock<Mutex<NativeAllocationRegistry>> = OnceLock::new();
static RUNTIME_CATALOG: OnceLock<Box<[u8]>> = OnceLock::new();
static RUNTIME_CATALOG_DIGEST: OnceLock<Box<[u8]>> = OnceLock::new();

fn reentrant_call_failure() -> NativeFailure {
    NativeFailure::reentrant_call("a host callback must not re-enter the same native engine")
}

fn native_failure_from_admission(error: BindingEngineAdmissionError) -> NativeFailure {
    match error {
        BindingEngineAdmissionError::Busy => NativeFailure::new(
            MERMAN_NATIVE_STATUS_BUSY,
            "the native engine has an active operation",
        )
        .with_kind(BindingErrorKind::Busy),
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

fn native_status_name(status: MermanNativeStatus) -> &'static str {
    match status {
        MERMAN_NATIVE_STATUS_OK => "ok",
        MERMAN_NATIVE_STATUS_INVALID_ARGUMENT => "invalid-argument",
        MERMAN_NATIVE_STATUS_UTF8_ERROR => "utf8-error",
        MERMAN_NATIVE_STATUS_OPTIONS_JSON_ERROR => "options-json-error",
        MERMAN_NATIVE_STATUS_NO_DIAGRAM => "no-diagram",
        MERMAN_NATIVE_STATUS_PARSE_ERROR => "parse-error",
        MERMAN_NATIVE_STATUS_RENDER_ERROR => "render-error",
        MERMAN_NATIVE_STATUS_UNSUPPORTED_OPERATION => "unsupported-operation",
        MERMAN_NATIVE_STATUS_PANIC => "panic",
        MERMAN_NATIVE_STATUS_INTERNAL_ERROR => "internal-error",
        MERMAN_NATIVE_STATUS_RESOURCE_LIMIT_EXCEEDED => "resource-limit-exceeded",
        MERMAN_NATIVE_STATUS_ABI_MISMATCH => "abi-mismatch",
        MERMAN_NATIVE_STATUS_ABI_LAYOUT_MISMATCH => "abi-layout-mismatch",
        MERMAN_NATIVE_STATUS_CALLBACK_ERROR => "callback-error",
        MERMAN_NATIVE_STATUS_REENTRANT_CALL => "reentrant-call",
        MERMAN_NATIVE_STATUS_INVALID_ENGINE => "invalid-engine",
        MERMAN_NATIVE_STATUS_BUSY => "busy",
        _ => "unknown-status",
    }
}

fn native_error_kind_name(kind: BindingErrorKind) -> &'static str {
    match kind {
        BindingErrorKind::Generic => MERMAN_NATIVE_ERROR_KIND_GENERIC,
        BindingErrorKind::UnknownOperation => MERMAN_NATIVE_ERROR_KIND_UNKNOWN_OPERATION,
        BindingErrorKind::MissingCapability => MERMAN_NATIVE_ERROR_KIND_MISSING_CAPABILITY,
        BindingErrorKind::Busy => MERMAN_NATIVE_ERROR_KIND_BUSY,
        BindingErrorKind::ReentrantCall => MERMAN_NATIVE_ERROR_KIND_REENTRANT_CALL,
    }
}

fn native_error_json(failure: &NativeFailure) -> Vec<u8> {
    let mut payload = serde_json::json!({
        "version": MERMAN_NATIVE_RESULT_SCHEMA_VERSION,
        "ok": false,
        "status": failure.status,
        "status_name": native_status_name(failure.status),
        "kind": native_error_kind_name(failure.kind),
        "capability_id": failure.capability_id,
        "message": failure.message.as_str(),
    });
    if let Some(resource) = failure.resource {
        payload["details"] = serde_json::json!({ "resource": resource });
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

fn native_success_json(operation: &str) -> Vec<u8> {
    serde_json::to_vec(&serde_json::json!({
        "version": MERMAN_NATIVE_RESULT_SCHEMA_VERSION,
        "ok": true,
        "operation": operation,
    }))
    .unwrap_or_default()
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
    };
    NativeFailure {
        status,
        kind: error.kind(),
        capability_id: error.capability_id(),
        resource: error.resource_details(),
        message: error.message().to_string(),
    }
}

fn binding_engine_for_transport(options_json: &[u8]) -> Result<BindingEngine, BindingError> {
    BindingEngine::from_options(options_json)
}

fn native_transport_capability_surface() -> ArtifactCapabilitySurface {
    #[cfg(feature = "svg")]
    let text_measurement = TextMeasurementProviderProjection::PreserveCompiled;
    #[cfg(not(feature = "svg"))]
    let text_measurement = TextMeasurementProviderProjection::VendoredOnly;

    merman_bindings_core::binding_transport_capability_surface()
        .project_to_descriptor_target("native", text_measurement)
        .expect("the C transport exposes a valid native capability surface")
}

fn engine_registry() -> &'static Mutex<NativeEngineRegistry> {
    ENGINE_REGISTRY.get_or_init(|| Mutex::new(NativeEngineRegistry::default()))
}

fn allocation_registry() -> &'static Mutex<NativeAllocationRegistry> {
    ALLOCATION_REGISTRY.get_or_init(|| Mutex::new(NativeAllocationRegistry::default()))
}

fn runtime_catalog_bytes() -> Result<&'static [u8], NativeFailure> {
    if let Some(bytes) = RUNTIME_CATALOG.get() {
        return Ok(bytes);
    }

    let candidate = merman_bindings_core::runtime_catalog_json_for(
        MERMAN_NATIVE_ABI_VERSION,
        native_transport_capability_surface(),
    )
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
    let digest = format!("sha256:{:x}", Sha256::digest(catalog))
        .into_bytes()
        .into_boxed_slice();
    let _ = RUNTIME_CATALOG_DIGEST.set(digest);
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

fn register_result_allocation(
    registry: &mut NativeAllocationRegistry,
    data: Vec<u8>,
    metadata_or_error_json: Vec<u8>,
) -> Result<(u64, MermanNativeBuffer, MermanNativeBuffer), NativeFailure> {
    let token = registry.register(data, metadata_or_error_json)?;
    let allocation = registry
        .results
        .get_mut(&token)
        .expect("result allocation was inserted");
    let data = owned_buffer_view(&mut allocation.data);
    let metadata_or_error_json = owned_buffer_view(&mut allocation.metadata_or_error_json);
    Ok((token, data, metadata_or_error_json))
}

fn release_result_allocation(token: u64) {
    if token == 0 {
        return;
    }
    let mut registry = allocation_registry()
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    let _ = registry.results.remove(&token);
}

fn operation_media_type(operation: MermanNativeOperationCode) -> Option<&'static str> {
    merman_native_operation_descriptor(operation).and_then(|descriptor| descriptor.media_type)
}

fn normalized_operation(operation: MermanNativeOperationCode) -> MermanNativeOperationCode {
    if merman_native_operation_descriptor(operation).is_some() {
        operation
    } else {
        MERMAN_NATIVE_OPERATION_NONE
    }
}

fn initialized_native_result() -> MermanNativeResult {
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
    unsafe { ptr::read(record.cast::<u32>()) }
}

unsafe fn result_is_writable(out_result: *mut MermanNativeResult) -> Result<(), NativeFailure> {
    if out_result.is_null() {
        return Err(NativeFailure::new(
            MERMAN_NATIVE_STATUS_INVALID_ARGUMENT,
            "out_result must not be null",
        ));
    }
    let struct_size = unsafe { read_record_struct_size(out_result) };
    validate_struct_size::<MermanNativeResult>(struct_size, "out_result")?;
    let result = unsafe { &*out_result };
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

unsafe fn write_native_result(
    out_result: *mut MermanNativeResult,
    status: MermanNativeStatus,
    operation: MermanNativeOperationCode,
    media_type: Option<&'static str>,
    data: Vec<u8>,
    metadata_or_error_json: Vec<u8>,
) -> MermanNativeStatus {
    let mut registry = allocation_registry()
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    unsafe {
        write_native_result_with_registry(
            &mut registry,
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
    registry: &mut NativeAllocationRegistry,
    out_result: *mut MermanNativeResult,
    status: MermanNativeStatus,
    operation: MermanNativeOperationCode,
    media_type: Option<&'static str>,
    data: Vec<u8>,
    metadata_or_error_json: Vec<u8>,
) -> MermanNativeStatus {
    let (allocation_token, data, metadata_or_error_json) =
        match register_result_allocation(registry, data, metadata_or_error_json) {
            Ok(allocation) => allocation,
            Err(failure) => return failure.status,
        };
    unsafe {
        ptr::write(
            out_result,
            MermanNativeResult {
                struct_size: native_struct_size::<MermanNativeResult>(),
                allocation_token,
                status,
                operation: normalized_operation(operation),
                media_type: static_slice(media_type.unwrap_or_default().as_bytes()),
                data,
                metadata_or_error_json,
            },
        );
    }
    status
}

unsafe fn write_native_failure(
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
    validate_struct_size::<MermanNativeEngineConfig>(
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

fn acquire_engine(token: MermanNativeEngineToken) -> Result<Arc<NativeEngineState>, NativeFailure> {
    if token == 0 {
        return Err(NativeFailure::new(
            MERMAN_NATIVE_STATUS_INVALID_ENGINE,
            "engine token 0 is not valid",
        ));
    }
    let registry = engine_registry()
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    registry.acquire(token).ok_or_else(|| {
        NativeFailure::new(
            MERMAN_NATIVE_STATUS_INVALID_ENGINE,
            "engine token is unknown or has already been released",
        )
    })
}

fn execute_with_engine<T>(
    token: MermanNativeEngineToken,
    request: NativeOperationRequest<'_>,
    consume: impl FnOnce(NativeExecution) -> Result<T, NativeFailure>,
) -> Result<T, NativeFailure> {
    let state = acquire_engine(token)?;
    let _operation = state
        .admission
        .enter_operation()
        .map_err(native_failure_from_admission)?;

    let operation =
        merman_native_operation_key(request.operation).ok_or_else(|| NativeFailure {
            status: MERMAN_NATIVE_STATUS_UNSUPPORTED_OPERATION,
            kind: BindingErrorKind::UnknownOperation,
            capability_id: None,
            resource: None,
            message: format!("unknown native operation code `{}`", request.operation),
        })?;
    let result = state
        .engine
        .execute(BindingOperationRequest {
            operation_id: operation.spec().id,
            source: request.source,
            uri: request.uri,
            options_json: request.options_json,
        })
        .map_err(native_failure_from_binding)?;

    consume(NativeExecution {
        operation: merman_native_operation_code(result.operation.key()),
        media_type: result.media_type,
        data: result.data,
        metadata_json: result.metadata_json,
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
    match catch_unwind(AssertUnwindSafe(f)) {
        Ok(status) => status,
        Err(_) => MERMAN_NATIVE_STATUS_PANIC,
    }
}

unsafe fn result_status_boundary(
    out_result: *mut MermanNativeResult,
    operation: MermanNativeOperationCode,
    f: impl FnOnce() -> MermanNativeStatus,
) -> MermanNativeStatus {
    match catch_unwind(AssertUnwindSafe(f)) {
        Ok(status) => status,
        Err(_) => {
            let failure = NativeFailure::new(
                MERMAN_NATIVE_STATUS_PANIC,
                "a Rust panic was caught at the native ABI boundary",
            );
            unsafe { write_failure_if_possible(out_result, operation, &failure) }
                .unwrap_or(failure.status)
        }
    }
}

/// Discovers the generated native ABI 3 function table.
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
                    "out_api.struct_size capacity is {output_capacity}; expected at least the ABI 3 minimum prefix size {MERMAN_NATIVE_API_MINIMUM_PREFIX_SIZE}"
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
        let capability_surface = native_transport_capability_surface();
        merman_bindings_core::binding_metadata_json_for(metadata_id, &capability_surface)
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
    if out_engine.is_null() {
        let failure = NativeFailure::new(
            MERMAN_NATIVE_STATUS_INVALID_ARGUMENT,
            "out_engine must not be null",
        );
        return unsafe { write_native_failure(out_result, MERMAN_NATIVE_OPERATION_NONE, &failure) };
    }
    let config_ptr = config;
    let fixed_storage_validation = (|| {
        validate_disjoint_storage(
            out_engine.cast::<u8>(),
            size_of::<MermanNativeEngineToken>(),
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
            size_of::<MermanNativeEngineToken>(),
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
    let config = match unsafe { read_engine_config(config_ptr) } {
        Ok(config) => config,
        Err(failure) => {
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
            size_of::<MermanNativeEngineToken>(),
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
    if let Err(failure) = validate_native_slice_shape(config.options_json, "config.options_json") {
        return unsafe { write_native_failure(out_result, MERMAN_NATIVE_OPERATION_NONE, &failure) };
    }
    let outcome = (|| {
        let options_json =
            unsafe { native_slice_bytes(config.options_json, "config.options_json") }?;
        let admission = BindingEngineAdmission::new(if config.text_measure.is_some() {
            BindingEngineAdmissionMode::HostCallback
        } else {
            BindingEngineAdmissionMode::Concurrent
        });
        let engine =
            binding_engine_for_transport(options_json).map_err(native_failure_from_binding)?;

        #[cfg(feature = "svg")]
        let engine = if let Some(callback) = config.text_measure {
            let measurer = NativeHostTextMeasurer::new(
                callback,
                config.text_measure_user_data,
                Arc::clone(&admission),
            );
            engine.with_host_text_measurer(Arc::new(measurer))
        } else {
            engine
        };

        #[cfg(not(feature = "svg"))]
        let engine = {
            if config.text_measure.is_some() {
                return Err(NativeFailure::missing_capability(
                    "svg",
                    "host text measurement requires an artifact with the svg capability",
                ));
            }
            engine
        };

        let state = Arc::new(NativeEngineState { engine, admission });
        let token = engine_registry()
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .register(state)?;
        Ok(token)
    })();

    match outcome {
        Ok(token) => {
            let result_status = unsafe {
                write_native_result(
                    out_result,
                    MERMAN_NATIVE_STATUS_OK,
                    MERMAN_NATIVE_OPERATION_NONE,
                    None,
                    Vec::new(),
                    native_success_json("engine-new"),
                )
            };
            if result_status != MERMAN_NATIVE_STATUS_OK {
                let mut registry = engine_registry()
                    .lock()
                    .unwrap_or_else(|poisoned| poisoned.into_inner());
                if let Some(state) = registry.acquire(token) {
                    let _ = state.admission.try_close();
                }
                let _ = registry.retire(token);
                return result_status;
            }
            unsafe { ptr::write(out_engine, token) };
            result_status
        }
        Err(failure) => unsafe {
            write_native_failure(out_result, MERMAN_NATIVE_OPERATION_NONE, &failure)
        },
    }
}

unsafe extern "C" fn native_engine_try_close(
    engine: MermanNativeEngineToken,
) -> MermanNativeStatus {
    status_boundary(|| {
        if engine == 0 {
            return MERMAN_NATIVE_STATUS_INVALID_ENGINE;
        }
        let mut registry = engine_registry()
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        let Some(state) = registry.acquire(engine) else {
            return MERMAN_NATIVE_STATUS_INVALID_ENGINE;
        };
        match state.admission.try_close() {
            Ok(()) => {
                let retired = registry.retire(engine);
                debug_assert!(retired.is_some(), "close retires the acquired engine token");
                MERMAN_NATIVE_STATUS_OK
            }
            Err(error) => native_failure_from_admission(error).status,
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
            execute_collect_impl(engine, request, out_result)
        })
    }
}

unsafe fn execute_collect_impl(
    engine: MermanNativeEngineToken,
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
    match execute_with_engine(engine, request, |execution| {
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
    let _ = catch_unwind(AssertUnwindSafe(|| unsafe {
        if result.is_null() {
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
    }));
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::BTreeSet;

    #[repr(C)]
    struct StructSizeOnly {
        struct_size: u32,
    }

    /// The ABI 3 table shape before the append-only `metadata_collect` slot.
    ///
    /// This deliberately does not use `MermanNativeApi`: a real older consumer owns only this
    /// prefix and may call discovery more than once with the returned `struct_size`.
    #[repr(C)]
    struct Abi3MinimumApi {
        struct_size: u32,
        abi_version: u32,
        minimum_prefix_layout_digest: MermanNativeSlice,
        full_descriptor_digest: MermanNativeSlice,
        capability_catalog_digest: MermanNativeSlice,
        package_version: MermanNativeSlice,
        runtime_catalog: Option<MermanNativeRuntimeCatalogFn>,
        engine_new: Option<MermanNativeEngineNewFn>,
        engine_try_close: Option<MermanNativeEngineTryCloseFn>,
        execute_collect: Option<MermanNativeExecuteCollectFn>,
        result_free: Option<MermanNativeResultFreeFn>,
    }

    #[repr(C)]
    struct Abi3MinimumApiBuffer {
        api: Abi3MinimumApi,
        trailing_guard: [u8; 16],
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
        }
    }

    fn empty_minimum_api() -> Abi3MinimumApi {
        Abi3MinimumApi {
            struct_size: MERMAN_NATIVE_API_MINIMUM_PREFIX_SIZE,
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
    }

    fn native_config() -> MermanNativeEngineConfig {
        MermanNativeEngineConfig {
            struct_size: native_struct_size::<MermanNativeEngineConfig>(),
            options_json: borrowed_slice(&[]),
            text_measure: None,
            text_measure_user_data: ptr::null_mut(),
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
        context.other_engine_status = unsafe {
            native_execute_collect(context.other_token, &nested_request, &mut other_result)
        };
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
        let wrong_version = MermanNativeApiRequest {
            struct_size: native_struct_size::<MermanNativeApiRequest>(),
            expected_abi_version: 2,
            expected_minimum_prefix_layout_digest: borrowed_slice(
                MERMAN_NATIVE_ABI_MINIMUM_PREFIX_LAYOUT_DIGEST.as_bytes(),
            ),
        };
        assert_eq!(
            unsafe { merman_get_native_api(&wrong_version, &mut api) },
            MERMAN_NATIVE_STATUS_ABI_MISMATCH
        );

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
    }

    #[test]
    fn discovery_reports_only_complete_prefixes_and_preserves_tail_storage() {
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
            MERMAN_NATIVE_API_METADATA_COLLECT_PREFIX_SIZE,
            native_struct_size::<MermanNativeApi>()
        );
        assert!(buffer.api.metadata_collect.is_some());
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
                MERMAN_NATIVE_API_METADATA_COLLECT_PREFIX_SIZE,
            ]
        );

        assert_eq!(
            size_of::<Abi3MinimumApi>() as u32,
            MERMAN_NATIVE_API_MINIMUM_PREFIX_SIZE
        );
        let mut minimum = Abi3MinimumApiBuffer {
            api: empty_minimum_api(),
            trailing_guard: [0xa5; 16],
        };
        let minimum_api = ptr::addr_of_mut!(minimum.api).cast::<MermanNativeApi>();
        assert_eq!(
            unsafe { merman_get_native_api(&request, minimum_api) },
            MERMAN_NATIVE_STATUS_OK
        );
        assert_eq!(
            minimum.api.struct_size,
            MERMAN_NATIVE_API_MINIMUM_PREFIX_SIZE
        );
        assert!(minimum.api.runtime_catalog.is_some());
        assert!(minimum.api.result_free.is_some());
        assert_eq!(minimum.trailing_guard, [0xa5; 16]);

        // The returned prefix size is itself safe input capacity. Older consumers do not need to
        // retain a second hidden copy of their allocation size before rediscovery.
        assert_eq!(
            unsafe { merman_get_native_api(&request, minimum_api) },
            MERMAN_NATIVE_STATUS_OK
        );
        assert_eq!(
            minimum.api.struct_size,
            MERMAN_NATIVE_API_MINIMUM_PREFIX_SIZE
        );
        assert_eq!(minimum.trailing_guard, [0xa5; 16]);

        // A capacity that ends inside the appended function pointer must not receive a partial
        // pointer, nor be reported as if that complete slot were available.
        minimum.api = empty_minimum_api();
        minimum.api.struct_size = MERMAN_NATIVE_API_MINIMUM_PREFIX_SIZE + 1;
        assert_eq!(
            unsafe { merman_get_native_api(&request, minimum_api) },
            MERMAN_NATIVE_STATUS_OK
        );
        assert_eq!(
            minimum.api.struct_size,
            MERMAN_NATIVE_API_MINIMUM_PREFIX_SIZE
        );
        assert_eq!(minimum.trailing_guard, [0xa5; 16]);
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
            api.result_free.unwrap()(
                (&mut prefix as *mut StructSizeOnly).cast::<MermanNativeResult>(),
            )
        };
        assert_eq!(
            unsafe { api.engine_try_close.unwrap()(token) },
            MERMAN_NATIVE_STATUS_OK
        );
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
            failure.message,
            "request.source.len must not exceed isize::MAX"
        );

        slice.struct_size = slice.struct_size.saturating_add(1);
        let failure = unsafe { native_slice_bytes(slice, "request.source") }
            .expect_err("oversized records must not silently negotiate a prefix");
        assert_eq!(failure.status, MERMAN_NATIVE_STATUS_INVALID_ARGUMENT);
        assert!(failure.message.contains("expected exactly"));
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
            unsafe {
                api.engine_new.unwrap()(ptr::addr_of!(config), overlapping_engine, &mut result)
            },
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
    fn runtime_catalog_is_the_flat_artifact_owned_contract() {
        let api = api_table();
        let mut result = native_result();
        assert_eq!(
            unsafe { api.runtime_catalog.unwrap()(&mut result) },
            MERMAN_NATIVE_STATUS_OK
        );
        assert_eq!(result.status, MERMAN_NATIVE_STATUS_OK);
        assert_eq!(result.operation, MERMAN_NATIVE_OPERATION_NONE);

        let catalog = unsafe {
            std::slice::from_raw_parts(
                result.metadata_or_error_json.data,
                result.metadata_or_error_json.len,
            )
        };
        let catalog: serde_json::Value = serde_json::from_slice(catalog).unwrap();
        let root = catalog.as_object().unwrap();
        assert_eq!(
            root.keys().map(String::as_str).collect::<BTreeSet<_>>(),
            [
                "capabilities",
                "metadata_ids",
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
        assert!(catalog.get("runtime_contract").is_none());
        assert!(catalog.get("capability_vocabulary").is_none());
        assert_eq!(
            catalog["capabilities"],
            serde_json::to_value(native_transport_capability_surface().runtime_capabilities())
                .unwrap()
        );
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
        assert_eq!(
            source_limit["operation_ids"],
            catalog["capabilities"]["operation_ids"]
        );

        let expected_digest = format!(
            "sha256:{:x}",
            Sha256::digest(serde_json::to_vec(&catalog).unwrap())
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

        for metadata_id in merman_bindings_core::BINDING_METADATA_IDS {
            let mut result = native_result();
            let status = unsafe { collect(borrowed_slice(metadata_id.as_bytes()), &mut result) };
            let capability_surface = native_transport_capability_surface();
            match merman_bindings_core::binding_metadata_json_for(metadata_id, &capability_surface)
            {
                Ok(expected) => {
                    assert_eq!(status, MERMAN_NATIVE_STATUS_OK, "{metadata_id}");
                    let actual = unsafe {
                        std::slice::from_raw_parts(
                            result.metadata_or_error_json.data,
                            result.metadata_or_error_json.len,
                        )
                    };
                    assert_eq!(actual, expected, "{metadata_id}");
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
        assert!(
            allocation_registry()
                .lock()
                .unwrap_or_else(|poisoned| poisoned.into_inner())
                .results
                .contains_key(&allocation_token)
        );

        let mut moved = unsafe { ptr::read(&original) };
        original = native_result();
        let moved_data = unsafe { std::slice::from_raw_parts(moved.data.data, moved.data.len) };
        assert_eq!(moved_data, b"owned by Merman");
        unsafe { native_result_free(&mut moved) };
        assert!(
            !allocation_registry()
                .lock()
                .unwrap_or_else(|poisoned| poisoned.into_inner())
                .results
                .contains_key(&allocation_token)
        );
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
        assert!(
            !allocation_registry()
                .lock()
                .unwrap_or_else(|poisoned| poisoned.into_inner())
                .results
                .contains_key(&allocation_token)
        );
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
        assert!(
            !allocation_registry()
                .lock()
                .unwrap_or_else(|poisoned| poisoned.into_inner())
                .results
                .contains_key(&token)
        );
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
        assert!(
            allocation_registry()
                .lock()
                .unwrap_or_else(|poisoned| poisoned.into_inner())
                .results
                .contains_key(&second.allocation_token)
        );

        let mut random = native_result();
        random.allocation_token = u64::MAX;
        unsafe { native_result_free(&mut random) };
        assert_eq!(random.allocation_token, 0);
        unsafe { native_result_free(&mut second) };
    }

    #[test]
    fn allocation_token_exhaustion_is_checked_without_reuse() {
        let mut registry = NativeAllocationRegistry {
            last_token: u64::MAX,
            results: BTreeMap::new(),
        };
        let mut result = native_result();
        let status = unsafe {
            write_native_result_with_registry(
                &mut registry,
                &mut result,
                MERMAN_NATIVE_STATUS_OK,
                MERMAN_NATIVE_OPERATION_NONE,
                None,
                Vec::new(),
                Vec::new(),
            )
        };

        assert_eq!(status, MERMAN_NATIVE_STATUS_INTERNAL_ERROR);
        assert!(registry.results.is_empty());
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

        let request = native_request(i32::MAX, b"flowchart TD\nA --> B");
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
        assert_eq!(
            unsafe { api.engine_try_close.unwrap()(token) },
            MERMAN_NATIVE_STATUS_OK
        );
    }

    #[test]
    fn svg_operation_follows_the_resolved_dependency_surface_and_owns_its_result() {
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
        if native_transport_capability_surface()
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
    fn try_close_returns_busy_without_retiring_an_active_engine() {
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
        let compiled = merman_bindings_core::compiled_runtime_capabilities().system_adapter_ids;
        let missing = ["system-clock", "system-timezone", "system-random"]
            .into_iter()
            .find(|capability| !compiled.contains(capability));
        let status = unsafe { api.engine_new.unwrap()(&config, &mut token, &mut result) };

        match missing {
            Some(expected_capability) => {
                assert_eq!(status, MERMAN_NATIVE_STATUS_UNSUPPORTED_OPERATION);
                assert_eq!(token, 0);
                let error = result_json(&result);
                assert_eq!(error["kind"], MERMAN_NATIVE_ERROR_KIND_MISSING_CAPABILITY);
                assert_eq!(error["capability_id"], expected_capability);
            }
            None => {
                assert_eq!(status, MERMAN_NATIVE_STATUS_OK);
                assert_ne!(token, 0);
                assert_eq!(
                    unsafe { api.engine_try_close.unwrap()(token) },
                    MERMAN_NATIVE_STATUS_OK
                );
            }
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
                api.engine_new.unwrap()(
                    &native_config(),
                    &mut other_token,
                    &mut other_config_result,
                )
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
    fn callback_enabled_engine_rejects_a_competing_operation_as_busy() {
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
        let request = native_request(
            MERMAN_NATIVE_OPERATION_SEMANTIC_JSON,
            b"flowchart TD\nA --> B",
        );
        let mut result = native_result();
        assert_eq!(
            unsafe { api.execute_collect.unwrap()(token, &request, &mut result) },
            MERMAN_NATIVE_STATUS_BUSY
        );
        assert_eq!(result.status, MERMAN_NATIVE_STATUS_BUSY);
        assert_eq!(result_json(&result)["kind"], MERMAN_NATIVE_ERROR_KIND_BUSY);
        unsafe { api.result_free.unwrap()(&mut result) };

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

    #[cfg(all(feature = "png", feature = "jpeg", feature = "pdf"))]
    #[test]
    fn binary_outputs_use_the_same_generic_operation_path() {
        let api = api_table();
        let mut config_result = native_result();
        let mut token = 0;
        let config = native_config();
        assert_eq!(
            unsafe { api.engine_new.unwrap()(&config, &mut token, &mut config_result) },
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
}
