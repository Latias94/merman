#![deny(unsafe_op_in_unsafe_fn)]

//! Native ABI 3 exports for embedding Merman in C-compatible hosts.
//!
//! The only exported C symbol is [`merman_get_native_api`]. Hosts discover a size-tagged
//! function table and execute every operation through the shared binding operation path. No raw Rust
//! allocation or Rust object pointer crosses this boundary.

#[cfg(feature = "svg")]
use merman_bindings_core::HostMeasurementResult;
use merman_bindings_core::{
    BindingEngine, BindingError, BindingErrorKind, BindingOperationRequest, BindingStatus,
};
use sha2::{Digest, Sha256};
use std::collections::{BTreeMap, BTreeSet};
use std::mem::size_of;
use std::panic::{AssertUnwindSafe, catch_unwind};
use std::ptr;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Condvar, Mutex, OnceLock};

include!("generated/text_measurement_abi.rs");
include!("generated/abi3.rs");

const PACKAGE_VERSION: &[u8] = env!("CARGO_PKG_VERSION").as_bytes();

#[derive(Debug)]
struct NativeFailure {
    status: MermanNativeStatus,
    kind: BindingErrorKind,
    capability_id: Option<&'static str>,
    message: String,
}

impl NativeFailure {
    fn new(status: MermanNativeStatus, message: impl Into<String>) -> Self {
        Self {
            status,
            kind: BindingErrorKind::Generic,
            capability_id: None,
            message: message.into(),
        }
    }

    fn reentrant_call(message: impl Into<String>) -> Self {
        Self {
            status: MERMAN_NATIVE_STATUS_REENTRANT_CALL,
            kind: BindingErrorKind::ReentrantCall,
            capability_id: None,
            message: message.into(),
        }
    }

    #[cfg(not(feature = "svg"))]
    fn missing_capability(capability_id: &'static str, message: impl Into<String>) -> Self {
        Self {
            status: MERMAN_NATIVE_STATUS_UNSUPPORTED_OPERATION,
            kind: BindingErrorKind::MissingCapability,
            capability_id: Some(capability_id),
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
    coordinator: Arc<NativeExecutionCoordinator>,
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
        engine.coordinator.bind_token(token);
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
    results: BTreeMap<usize, NativeResultAllocation>,
}

struct NativeResultAllocation {
    data: Vec<u8>,
    metadata_or_error_json: Vec<u8>,
}

static ENGINE_REGISTRY: OnceLock<Mutex<NativeEngineRegistry>> = OnceLock::new();
static ALLOCATION_REGISTRY: OnceLock<Mutex<NativeAllocationRegistry>> = OnceLock::new();
static RUNTIME_CATALOG: OnceLock<Box<[u8]>> = OnceLock::new();
static RUNTIME_CATALOG_DIGEST: OnceLock<Box<[u8]>> = OnceLock::new();
static ACTIVE_CALLBACK_TOKENS: OnceLock<Mutex<BTreeSet<MermanNativeEngineToken>>> = OnceLock::new();
// Serializes the callback-active transition with engine retirement. Without this
// lifecycle lock, `engine_free` can observe the callback token before the
// callback entry has published it (or vice versa) and retire an engine during
// the small publication window.
static ENGINE_LIFECYCLE_LOCK: OnceLock<Mutex<()>> = OnceLock::new();

#[derive(Default)]
struct NativeExecutionState {
    operation_active: bool,
    callback_active: bool,
}

struct NativeExecutionCoordinator {
    token: AtomicU64,
    state: Mutex<NativeExecutionState>,
    ready: Condvar,
}

impl NativeExecutionCoordinator {
    fn new() -> Self {
        Self {
            token: AtomicU64::new(0),
            state: Mutex::new(NativeExecutionState::default()),
            ready: Condvar::new(),
        }
    }

    fn bind_token(&self, token: MermanNativeEngineToken) {
        assert_ne!(token, 0);
        assert_eq!(
            self.token
                .compare_exchange(0, token, Ordering::AcqRel, Ordering::Acquire),
            Ok(0),
            "a native execution coordinator is bound exactly once",
        );
    }

    fn enter_operation(&self) -> Result<NativeOperationGuard<'_>, NativeFailure> {
        let mut state = self
            .state
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        loop {
            if state.callback_active {
                return Err(reentrant_call_failure());
            }
            if !state.operation_active {
                state.operation_active = true;
                return Ok(NativeOperationGuard { coordinator: self });
            }
            state = self
                .ready
                .wait(state)
                .unwrap_or_else(|poisoned| poisoned.into_inner());
        }
    }

    #[cfg(feature = "svg")]
    fn invoke_host_callback<T>(&self, callback: impl FnOnce() -> T) -> T {
        let token = self.token.load(Ordering::Acquire);
        debug_assert_ne!(token, 0, "host callbacks require a bound engine token");

        {
            let _lifecycle_guard = engine_lifecycle_lock()
                .lock()
                .unwrap_or_else(|poisoned| poisoned.into_inner());
            let mut state = self
                .state
                .lock()
                .unwrap_or_else(|poisoned| poisoned.into_inner());
            debug_assert!(
                state.operation_active,
                "host callbacks run inside an operation"
            );
            debug_assert!(
                !state.callback_active,
                "host callbacks are not recursively nested"
            );
            state.callback_active = true;
            active_callback_tokens()
                .lock()
                .unwrap_or_else(|poisoned| poisoned.into_inner())
                .insert(token);
            self.ready.notify_all();
        }

        struct CallbackGuard<'a> {
            coordinator: &'a NativeExecutionCoordinator,
            token: MermanNativeEngineToken,
        }

        impl Drop for CallbackGuard<'_> {
            fn drop(&mut self) {
                let _lifecycle_guard = engine_lifecycle_lock()
                    .lock()
                    .unwrap_or_else(|poisoned| poisoned.into_inner());
                {
                    let mut state = self
                        .coordinator
                        .state
                        .lock()
                        .unwrap_or_else(|poisoned| poisoned.into_inner());
                    state.callback_active = false;
                    self.coordinator.ready.notify_all();
                }
                active_callback_tokens()
                    .lock()
                    .unwrap_or_else(|poisoned| poisoned.into_inner())
                    .remove(&self.token);
            }
        }

        let _guard = CallbackGuard {
            coordinator: self,
            token,
        };
        callback()
    }
}

struct NativeOperationGuard<'a> {
    coordinator: &'a NativeExecutionCoordinator,
}

impl Drop for NativeOperationGuard<'_> {
    fn drop(&mut self) {
        let mut state = self
            .coordinator
            .state
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        debug_assert!(
            !state.callback_active,
            "operation cannot end during a host callback"
        );
        state.operation_active = false;
        self.coordinator.ready.notify_all();
    }
}

fn active_callback_tokens() -> &'static Mutex<BTreeSet<MermanNativeEngineToken>> {
    ACTIVE_CALLBACK_TOKENS.get_or_init(|| Mutex::new(BTreeSet::new()))
}

fn engine_lifecycle_lock() -> &'static Mutex<()> {
    ENGINE_LIFECYCLE_LOCK.get_or_init(|| Mutex::new(()))
}

fn callback_is_active(token: MermanNativeEngineToken) -> bool {
    active_callback_tokens()
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
        .contains(&token)
}

fn reentrant_call_failure() -> NativeFailure {
    NativeFailure::reentrant_call("a host callback must not re-enter the same native engine")
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
        _ => "unknown-status",
    }
}

fn native_error_kind_name(kind: BindingErrorKind) -> &'static str {
    match kind {
        BindingErrorKind::Generic => MERMAN_NATIVE_ERROR_KIND_GENERIC,
        BindingErrorKind::UnknownOperation => MERMAN_NATIVE_ERROR_KIND_UNKNOWN_OPERATION,
        BindingErrorKind::MissingCapability => MERMAN_NATIVE_ERROR_KIND_MISSING_CAPABILITY,
        BindingErrorKind::ReentrantCall => MERMAN_NATIVE_ERROR_KIND_REENTRANT_CALL,
    }
}

fn native_error_json(failure: &NativeFailure) -> Vec<u8> {
    serde_json::to_vec(&serde_json::json!({
        "version": MERMAN_NATIVE_RESULT_SCHEMA_VERSION,
        "ok": false,
        "status": failure.status,
        "status_name": native_status_name(failure.status),
        "kind": native_error_kind_name(failure.kind),
        "capability_id": failure.capability_id,
        "message": failure.message.as_str(),
    }))
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
    };
    NativeFailure {
        status,
        kind: error.kind(),
        capability_id: error.capability_id(),
        message: error.message().to_string(),
    }
}

fn binding_engine_for_transport(options_json: &[u8]) -> Result<BindingEngine, BindingError> {
    BindingEngine::from_options(options_json)
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

    let candidate = merman_bindings_core::runtime_catalog_json(MERMAN_NATIVE_ABI_VERSION)
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
    result: *mut MermanNativeResult,
    data: Vec<u8>,
    metadata_or_error_json: Vec<u8>,
) -> (MermanNativeBuffer, MermanNativeBuffer) {
    let mut registry = allocation_registry()
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    registry.results.insert(
        result as usize,
        NativeResultAllocation {
            data,
            metadata_or_error_json,
        },
    );
    let allocation = registry
        .results
        .get_mut(&(result as usize))
        .expect("result allocation was inserted");
    let data = owned_buffer_view(&mut allocation.data);
    let metadata_or_error_json = owned_buffer_view(&mut allocation.metadata_or_error_json);
    (data, metadata_or_error_json)
}

fn release_result_allocation(result: *mut MermanNativeResult) {
    let mut registry = allocation_registry()
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    let _ = registry.results.remove(&(result as usize));
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

fn empty_native_result(
    status: MermanNativeStatus,
    operation: MermanNativeOperationCode,
    media_type: Option<&'static str>,
) -> MermanNativeResult {
    MermanNativeResult {
        struct_size: native_struct_size::<MermanNativeResult>(),
        status,
        operation: normalized_operation(operation),
        media_type: static_slice(media_type.unwrap_or_default().as_bytes()),
        data: empty_buffer(),
        metadata_or_error_json: empty_buffer(),
    }
}

fn validate_struct_size<T>(actual: u32, name: &str) -> Result<(), NativeFailure> {
    let required = native_struct_size::<T>();
    if actual < required {
        return Err(NativeFailure::new(
            MERMAN_NATIVE_STATUS_INVALID_ARGUMENT,
            format!("{name}.struct_size is {actual}; expected at least {required}"),
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
    validate_struct_size::<MermanNativeResult>(struct_size, "out_result")
}

unsafe fn write_native_result(
    out_result: *mut MermanNativeResult,
    status: MermanNativeStatus,
    operation: MermanNativeOperationCode,
    media_type: Option<&'static str>,
    data: Vec<u8>,
    metadata_or_error_json: Vec<u8>,
) {
    let (data, metadata_or_error_json) =
        register_result_allocation(out_result, data, metadata_or_error_json);
    unsafe {
        ptr::write(
            out_result,
            MermanNativeResult {
                struct_size: native_struct_size::<MermanNativeResult>(),
                status,
                operation: normalized_operation(operation),
                media_type: static_slice(media_type.unwrap_or_default().as_bytes()),
                data,
                metadata_or_error_json,
            },
        );
    }
}

unsafe fn write_native_failure(
    out_result: *mut MermanNativeResult,
    operation: MermanNativeOperationCode,
    failure: &NativeFailure,
) {
    unsafe {
        write_native_result(
            out_result,
            failure.status,
            operation,
            operation_media_type(operation),
            Vec::new(),
            native_error_json(failure),
        );
    }
}

unsafe fn write_failure_if_possible(
    out_result: *mut MermanNativeResult,
    operation: MermanNativeOperationCode,
    failure: &NativeFailure,
) {
    if unsafe { result_is_writable(out_result) }.is_ok() {
        unsafe { write_native_failure(out_result, operation, failure) };
    }
}

unsafe fn native_slice_bytes<'a>(
    slice: MermanNativeSlice,
    name: &str,
) -> Result<&'a [u8], NativeFailure> {
    validate_struct_size::<MermanNativeSlice>(slice.struct_size, name)?;
    if slice.len == 0 {
        return Ok(&[]);
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
    Ok(unsafe { std::slice::from_raw_parts(slice.data, slice.len) })
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
    if callback_is_active(token) {
        return Err(reentrant_call_failure());
    }
    let state = acquire_engine(token)?;
    let _operation = state.coordinator.enter_operation()?;

    let operation = merman_bindings_core::BindingOperationKind::from_native_code(request.operation)
        .map_err(native_failure_from_binding)?;
    let result = state
        .engine
        .execute(BindingOperationRequest {
            operation_id: operation.operation_id(),
            source: request.source,
            uri: request.uri,
            options_json: request.options_json,
        })
        .map_err(native_failure_from_binding)?;

    consume(NativeExecution {
        operation: result.operation.native_code(),
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
    coordinator: Arc<NativeExecutionCoordinator>,
}

#[cfg(feature = "svg")]
impl NativeHostTextMeasurer {
    const DEFAULT_FONT_STYLE: &'static [u8] = b"normal";
    const DEFAULT_FONT_WEIGHT: &'static [u8] = b"normal";

    fn new(
        callback: MermanNativeTextMeasureCallback,
        user_data: *mut std::ffi::c_void,
        coordinator: Arc<NativeExecutionCoordinator>,
    ) -> Self {
        Self {
            callback,
            user_data: user_data as usize,
            coordinator,
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
        let status = self.coordinator.invoke_host_callback(|| unsafe {
            (self.callback)(
                &native_request,
                &mut native_result,
                self.user_data as *mut std::ffi::c_void,
            )
        });
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
            return Err(merman_bindings_core::HostTextMeasurementError::new(
                "host text-measure callback returned an invalid result record",
            ));
        }
        if native_result.handled == 0 {
            return Ok(None);
        }

        let Some(kind) = merman_bindings_core::HostTextMeasurementResultKind::from_external_code(
            native_result.result_kind,
        ) else {
            return Err(merman_bindings_core::HostTextMeasurementError::new(
                "host text-measure callback returned an unknown result kind",
            ));
        };
        if kind
            != merman_bindings_core::HostTextMeasurementResultKind::expected_for_operation(
                request.operation,
            )
        {
            return Err(merman_bindings_core::HostTextMeasurementError::new(
                "host text-measure callback returned the wrong result kind",
            ));
        }

        Ok(Some(
            merman_bindings_core::host_text_measurement_from_values(
                Some(kind),
                merman_bindings_core::HostTextMeasurementValues {
                    width: native_result.width,
                    height: native_result.height,
                    line_count: native_result.line_count,
                    length: native_result.length,
                    bbox_left: native_result.bbox_left,
                    bbox_right: native_result.bbox_right,
                    raw_width: (native_result.has_raw_width != 0)
                        .then_some(native_result.raw_width),
                },
            ),
        ))
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
            unsafe { write_failure_if_possible(out_result, operation, &failure) };
            failure.status
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
        let output_size = unsafe { read_record_struct_size(out_api) };
        validate_struct_size::<MermanNativeApi>(output_size, "out_api")?;
        if request.expected_abi_version != MERMAN_NATIVE_ABI_VERSION {
            return Err(NativeFailure::new(
                MERMAN_NATIVE_STATUS_ABI_MISMATCH,
                format!(
                    "native ABI {} was requested; this library implements {}",
                    request.expected_abi_version, MERMAN_NATIVE_ABI_VERSION
                ),
            ));
        }
        let expected_digest = unsafe {
            native_slice_bytes(
                request.expected_layout_descriptor_digest,
                "request.expected_layout_descriptor_digest",
            )
        }?;
        if expected_digest != MERMAN_NATIVE_ABI_LAYOUT_DESCRIPTOR_DIGEST.as_bytes() {
            return Err(NativeFailure::new(
                MERMAN_NATIVE_STATUS_ABI_LAYOUT_MISMATCH,
                "native ABI descriptor digest does not match this library",
            ));
        }

        let capability_catalog_digest = runtime_catalog_digest_bytes()?;
        let api = MermanNativeApi {
            struct_size: native_struct_size::<MermanNativeApi>(),
            abi_version: MERMAN_NATIVE_ABI_VERSION,
            layout_descriptor_digest: static_slice(
                MERMAN_NATIVE_ABI_LAYOUT_DESCRIPTOR_DIGEST.as_bytes(),
            ),
            capability_catalog_digest: static_slice(capability_catalog_digest),
            package_version: static_slice(PACKAGE_VERSION),
            runtime_catalog: Some(native_runtime_catalog),
            engine_new: Some(native_engine_new),
            engine_free: Some(native_engine_free),
            execute_collect: Some(native_execute_collect),
            result_free: Some(native_result_free),
        };
        unsafe { ptr::write(out_api, api) };
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
        Ok(catalog) => {
            unsafe {
                write_native_result(
                    out_result,
                    MERMAN_NATIVE_STATUS_OK,
                    MERMAN_NATIVE_OPERATION_NONE,
                    None,
                    Vec::new(),
                    catalog.to_vec(),
                );
            }
            MERMAN_NATIVE_STATUS_OK
        }
        Err(failure) => {
            unsafe { write_native_failure(out_result, MERMAN_NATIVE_OPERATION_NONE, &failure) };
            failure.status
        }
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
        unsafe { write_native_failure(out_result, MERMAN_NATIVE_OPERATION_NONE, &failure) };
        return failure.status;
    }
    unsafe { ptr::write(out_engine, 0) };

    let outcome = (|| {
        let config = unsafe { read_engine_config(config) }?;
        let options_json =
            unsafe { native_slice_bytes(config.options_json, "config.options_json") }?;
        let coordinator = Arc::new(NativeExecutionCoordinator::new());
        let engine =
            binding_engine_for_transport(options_json).map_err(native_failure_from_binding)?;

        #[cfg(feature = "svg")]
        let engine = if let Some(callback) = config.text_measure {
            let measurer = NativeHostTextMeasurer::new(
                callback,
                config.text_measure_user_data,
                Arc::clone(&coordinator),
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

        let state = Arc::new(NativeEngineState {
            engine,
            coordinator: Arc::clone(&coordinator),
        });
        let token = engine_registry()
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .register(state)?;
        Ok(token)
    })();

    match outcome {
        Ok(token) => {
            unsafe { ptr::write(out_engine, token) };
            unsafe {
                write_native_result(
                    out_result,
                    MERMAN_NATIVE_STATUS_OK,
                    MERMAN_NATIVE_OPERATION_NONE,
                    None,
                    Vec::new(),
                    native_success_json("engine-new"),
                );
            }
            MERMAN_NATIVE_STATUS_OK
        }
        Err(failure) => {
            unsafe { write_native_failure(out_result, MERMAN_NATIVE_OPERATION_NONE, &failure) };
            failure.status
        }
    }
}

unsafe extern "C" fn native_engine_free(engine: MermanNativeEngineToken) -> MermanNativeStatus {
    status_boundary(|| {
        let _lifecycle_guard = engine_lifecycle_lock()
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        if callback_is_active(engine) {
            return MERMAN_NATIVE_STATUS_REENTRANT_CALL;
        }
        let retired = engine_registry()
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .retire(engine);
        if retired.is_some() {
            MERMAN_NATIVE_STATUS_OK
        } else {
            MERMAN_NATIVE_STATUS_INVALID_ENGINE
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
            unsafe { write_native_failure(out_result, MERMAN_NATIVE_OPERATION_NONE, &failure) };
            return failure.status;
        }
    };
    let operation = normalized_operation(request.operation);
    match execute_with_engine(engine, request, |execution| {
        unsafe {
            write_native_result(
                out_result,
                MERMAN_NATIVE_STATUS_OK,
                execution.operation,
                Some(execution.media_type),
                execution.data,
                execution.metadata_json,
            );
        }
        Ok(())
    }) {
        Ok(()) => MERMAN_NATIVE_STATUS_OK,
        Err(failure) => {
            unsafe { write_native_failure(out_result, operation, &failure) };
            failure.status
        }
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
        release_result_allocation(result);
        ptr::write(
            result,
            empty_native_result(MERMAN_NATIVE_STATUS_OK, MERMAN_NATIVE_OPERATION_NONE, None),
        );
    }));
}

#[cfg(test)]
mod tests {
    use super::*;

    #[repr(C)]
    struct StructSizeOnly {
        struct_size: u32,
    }

    fn api_table() -> MermanNativeApi {
        let mut api = MermanNativeApi {
            struct_size: native_struct_size::<MermanNativeApi>(),
            abi_version: 0,
            layout_descriptor_digest: static_slice(&[]),
            capability_catalog_digest: static_slice(&[]),
            package_version: static_slice(&[]),
            runtime_catalog: None,
            engine_new: None,
            engine_free: None,
            execute_collect: None,
            result_free: None,
        };
        let request = MermanNativeApiRequest {
            struct_size: native_struct_size::<MermanNativeApiRequest>(),
            expected_abi_version: MERMAN_NATIVE_ABI_VERSION,
            expected_layout_descriptor_digest: borrowed_slice(
                MERMAN_NATIVE_ABI_LAYOUT_DESCRIPTOR_DIGEST.as_bytes(),
            ),
        };
        let status = unsafe { merman_get_native_api(&request, &mut api) };
        assert_eq!(status, MERMAN_NATIVE_STATUS_OK);
        api
    }

    fn native_result() -> MermanNativeResult {
        empty_native_result(MERMAN_NATIVE_STATUS_OK, MERMAN_NATIVE_OPERATION_NONE, None)
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
        context.free_status = unsafe { native_engine_free(context.token) };

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
        let mut api = MermanNativeApi {
            struct_size: native_struct_size::<MermanNativeApi>(),
            abi_version: 0,
            layout_descriptor_digest: static_slice(&[]),
            capability_catalog_digest: static_slice(&[]),
            package_version: static_slice(&[]),
            runtime_catalog: None,
            engine_new: None,
            engine_free: None,
            execute_collect: None,
            result_free: None,
        };
        let wrong_version = MermanNativeApiRequest {
            struct_size: native_struct_size::<MermanNativeApiRequest>(),
            expected_abi_version: 2,
            expected_layout_descriptor_digest: borrowed_slice(
                MERMAN_NATIVE_ABI_LAYOUT_DESCRIPTOR_DIGEST.as_bytes(),
            ),
        };
        assert_eq!(
            unsafe { merman_get_native_api(&wrong_version, &mut api) },
            MERMAN_NATIVE_STATUS_ABI_MISMATCH
        );

        let wrong_digest = MermanNativeApiRequest {
            struct_size: native_struct_size::<MermanNativeApiRequest>(),
            expected_abi_version: MERMAN_NATIVE_ABI_VERSION,
            expected_layout_descriptor_digest: borrowed_slice(b"sha256:wrong"),
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
            expected_layout_descriptor_digest: borrowed_slice(
                MERMAN_NATIVE_ABI_LAYOUT_DESCRIPTOR_DIGEST.as_bytes(),
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
            unsafe { api.engine_free.unwrap()(token) },
            MERMAN_NATIVE_STATUS_OK
        );
    }

    #[test]
    fn native_slices_reject_lengths_larger_than_isize_max() {
        let slice = MermanNativeSlice {
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
                "package_version",
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
        assert!(catalog.get("runtime_contract").is_none());
        assert!(catalog.get("capability_vocabulary").is_none());

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
    fn result_output_only_requires_an_initialized_struct_size() {
        let api = api_table();
        let mut raw_result = std::mem::MaybeUninit::<MermanNativeResult>::uninit();
        unsafe {
            ptr::addr_of_mut!((*raw_result.as_mut_ptr()).struct_size)
                .write(native_struct_size::<MermanNativeResult>());
        }

        assert_eq!(
            unsafe { api.runtime_catalog.unwrap()(raw_result.as_mut_ptr()) },
            MERMAN_NATIVE_STATUS_OK
        );

        let result = unsafe { raw_result.assume_init_mut() };
        assert_eq!(result.status, MERMAN_NATIVE_STATUS_OK);
        assert!(!result.metadata_or_error_json.data.is_null());
        unsafe { api.result_free.unwrap()(result) };
    }

    #[test]
    fn result_ownership_is_bound_to_the_written_record_address() {
        let mut original = native_result();
        unsafe {
            write_native_result(
                &mut original,
                MERMAN_NATIVE_STATUS_OK,
                MERMAN_NATIVE_OPERATION_SEMANTIC_JSON,
                Some("application/json"),
                b"owned by Merman".to_vec(),
                b"{\"ok\":true}".to_vec(),
            );
        }
        let original_key = (&mut original as *mut MermanNativeResult) as usize;
        assert!(
            allocation_registry()
                .lock()
                .unwrap_or_else(|poisoned| poisoned.into_inner())
                .results
                .contains_key(&original_key)
        );

        let mut copied = unsafe { ptr::read(&original) };
        unsafe { native_result_free(&mut copied) };
        assert!(
            allocation_registry()
                .lock()
                .unwrap_or_else(|poisoned| poisoned.into_inner())
                .results
                .contains_key(&original_key)
        );
        let original_data =
            unsafe { std::slice::from_raw_parts(original.data.data, original.data.len) };
        assert_eq!(original_data, b"owned by Merman");

        unsafe { native_result_free(&mut original) };
        assert!(
            !allocation_registry()
                .lock()
                .unwrap_or_else(|poisoned| poisoned.into_inner())
                .results
                .contains_key(&original_key)
        );
    }

    #[test]
    fn result_free_is_idempotent_and_ignores_unowned_buffer_fields() {
        let mut current = native_result();
        unsafe {
            write_native_result(
                &mut current,
                MERMAN_NATIVE_STATUS_OK,
                MERMAN_NATIVE_OPERATION_SEMANTIC_JSON,
                Some("application/json"),
                b"current allocation".to_vec(),
                Vec::new(),
            );
        }
        let mut forged_copy = native_result();
        forged_copy.data = current.data;
        unsafe { native_result_free(&mut forged_copy) };
        let current_data =
            unsafe { std::slice::from_raw_parts(current.data.data, current.data.len) };
        assert_eq!(current_data, b"current allocation");

        let mut foreign = b"host allocation".to_vec();
        let mut result = native_result();
        result.data = MermanNativeBuffer {
            struct_size: native_struct_size::<MermanNativeBuffer>(),
            data: foreign.as_mut_ptr(),
            len: foreign.len(),
        };

        unsafe { native_result_free(&mut result) };
        assert_eq!(foreign, b"host allocation");
        assert!(result.data.data.is_null());
        assert!(result.metadata_or_error_json.data.is_null());

        unsafe { native_result_free(&mut result) };
        assert_eq!(foreign, b"host allocation");

        unsafe { native_result_free(&mut current) };
    }

    #[test]
    fn result_free_reads_only_the_initialized_struct_size_prefix() {
        let mut raw_result = std::mem::MaybeUninit::<MermanNativeResult>::uninit();
        unsafe {
            ptr::addr_of_mut!((*raw_result.as_mut_ptr()).struct_size)
                .write(native_struct_size::<MermanNativeResult>());
            native_result_free(raw_result.as_mut_ptr());
        }
        let result = unsafe { raw_result.assume_init() };
        assert_eq!(result.status, MERMAN_NATIVE_STATUS_OK);
        assert!(result.data.data.is_null());
        assert!(result.metadata_or_error_json.data.is_null());
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
            unsafe { api.engine_free.unwrap()(token) },
            MERMAN_NATIVE_STATUS_OK
        );
        assert_eq!(
            unsafe { api.engine_free.unwrap()(token) },
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
            unsafe { api.engine_free.unwrap()(token) },
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
            unsafe { api.engine_free.unwrap()(token) },
            MERMAN_NATIVE_STATUS_OK
        );
    }

    #[test]
    fn acquired_engine_state_survives_token_retirement() {
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
            acquired.coordinator.token.load(Ordering::Acquire),
            token,
            "the callback guard must use the published token in every build profile"
        );
        assert_eq!(
            unsafe { api.engine_free.unwrap()(token) },
            MERMAN_NATIVE_STATUS_OK
        );
        let failure = match acquire_engine(token) {
            Ok(_) => panic!("retired token must reject new calls"),
            Err(failure) => failure,
        };
        assert_eq!(failure.status, MERMAN_NATIVE_STATUS_INVALID_ENGINE);

        let operation = acquired
            .coordinator
            .enter_operation()
            .expect("an already acquired engine remains alive after retirement");
        drop(operation);
        assert_eq!(
            unsafe { api.engine_free.unwrap()(token) },
            MERMAN_NATIVE_STATUS_INVALID_ENGINE
        );
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
                    unsafe { api.engine_free.unwrap()(token) },
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
            unsafe { api.engine_free.unwrap()(token) },
            MERMAN_NATIVE_STATUS_OK
        );
        assert_eq!(
            unsafe { api.engine_free.unwrap()(other_token) },
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
            unsafe { api.engine_free.unwrap()(token) },
            MERMAN_NATIVE_STATUS_OK
        );
    }

    #[cfg(feature = "svg")]
    #[test]
    fn engine_free_is_rejected_while_any_thread_is_in_a_host_callback() {
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
            unsafe { api.engine_free.unwrap()(token) },
            MERMAN_NATIVE_STATUS_REENTRANT_CALL
        );
        context.proceed.wait();
        let (status, rendered_svg) = execution.join().expect("native execution thread");
        assert_eq!(status, MERMAN_NATIVE_STATUS_OK);
        assert!(rendered_svg.starts_with(b"<svg"));

        assert_eq!(
            unsafe { api.engine_free.unwrap()(token) },
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
            unsafe { api.result_free.unwrap()(&mut result) };
        }

        assert_eq!(
            unsafe { api.engine_free.unwrap()(token) },
            MERMAN_NATIVE_STATUS_OK
        );
    }

    #[cfg(not(feature = "svg"))]
    #[test]
    fn unavailable_svg_is_a_typed_native_error() {
        let api = api_table();
        let mut config_result = native_result();
        let mut token = 0;
        let config = native_config();
        assert_eq!(
            unsafe { api.engine_new.unwrap()(&config, &mut token, &mut config_result) },
            MERMAN_NATIVE_STATUS_OK
        );
        unsafe { api.result_free.unwrap()(&mut config_result) };

        let request = native_request(MERMAN_NATIVE_OPERATION_SVG, b"flowchart TD\nA --> B");
        let mut result = native_result();
        assert_eq!(
            unsafe { api.execute_collect.unwrap()(token, &request, &mut result) },
            MERMAN_NATIVE_STATUS_UNSUPPORTED_OPERATION
        );
        assert_eq!(result.status, MERMAN_NATIVE_STATUS_UNSUPPORTED_OPERATION);
        let error = result_json(&result);
        assert_eq!(error["kind"], MERMAN_NATIVE_ERROR_KIND_MISSING_CAPABILITY);
        assert_eq!(error["capability_id"], "svg");
        unsafe { api.result_free.unwrap()(&mut result) };
        assert_eq!(
            unsafe { api.engine_free.unwrap()(token) },
            MERMAN_NATIVE_STATUS_OK
        );
    }
}
