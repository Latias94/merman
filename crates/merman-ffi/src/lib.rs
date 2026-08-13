#![deny(unsafe_op_in_unsafe_fn)]

//! Native ABI 3 exports for embedding Merman in C-compatible hosts.
//!
//! The only exported C symbol is [`merman_get_native_api`]. Hosts discover a size-tagged
//! function table and execute every operation through the shared binding operation path. No raw Rust
//! allocation or Rust object pointer crosses this boundary.

use merman::OperationControl;
use merman_bindings_core::{
    BindingEngine, BindingEngineAdmission, BindingEngineAdmissionError, BindingEngineAdmissionMode,
    BindingEngineServices, BindingError, BindingErrorKind, BindingIconRegistryErrorDetails,
    BindingOperationRequest, BindingResourceErrorDetails, BindingStatus, OperationKey,
    ValidatedArtifactContract,
};
#[cfg(feature = "svg")]
use merman_bindings_core::{
    HostMeasurementResult, IconPack, IconRegistryResourceLimitId, build_icon_registry,
};
use sha2::{Digest, Sha256};
use std::collections::BTreeMap;
use std::mem::{align_of, size_of};
use std::panic::{AssertUnwindSafe, catch_unwind};
use std::ptr;
use std::sync::{Arc, Mutex, OnceLock};
use std::time::Duration;

include!("generated/text_measurement_abi.rs");
include!("generated/abi3.rs");

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
    details: Option<Box<NativeFailureDetails>>,
    message: Box<str>,
}

#[derive(Debug, Default)]
struct NativeFailureDetails {
    resource: Option<BindingResourceErrorDetails>,
    icon_registry: Option<BindingIconRegistryErrorDetails>,
    cancellation: Option<NativeCancellationDetails>,
}

#[derive(Debug, Clone, Copy)]
struct NativeCancellationDetails {
    reason: &'static str,
    phase: &'static str,
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
                Box::new(NativeFailureDetails {
                    resource: Some(resource),
                    ..NativeFailureDetails::default()
                })
            }),
            message: message.into().into_boxed_str(),
        }
    }

    #[cfg(not(feature = "svg"))]
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

#[derive(Default)]
struct NativeEngineRegistry {
    last_counter: u64,
    engines: BTreeMap<MermanNativeEngineToken, NativeEngineEntry>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum NativeEnginePublication {
    Reserved,
    Published,
}

struct NativeEngineEntry {
    publication: NativeEnginePublication,
    state: Arc<NativeEngineState>,
}

impl NativeEngineRegistry {
    fn issue_token(&mut self) -> Result<MermanNativeEngineToken, NativeFailure> {
        issue_domain_token(
            &mut self.last_counter,
            MERMAN_NATIVE_ENGINE_TOKEN_DOMAIN_TAG,
            "native engine token space is exhausted",
        )
    }

    #[cfg(test)]
    fn publish(&mut self, token: MermanNativeEngineToken, engine: Arc<NativeEngineState>) {
        debug_assert!(token_has_domain(
            token,
            MERMAN_NATIVE_ENGINE_TOKEN_DOMAIN_TAG
        ));
        let previous = self.engines.insert(
            token,
            NativeEngineEntry {
                publication: NativeEnginePublication::Published,
                state: engine,
            },
        );
        debug_assert!(previous.is_none(), "native engine tokens are never reused");
    }

    fn try_reserve(
        &mut self,
        engine: Arc<NativeEngineState>,
    ) -> Result<MermanNativeEngineToken, (Box<NativeFailure>, Arc<NativeEngineState>)> {
        let token = match self.issue_token() {
            Ok(token) => token,
            Err(failure) => return Err((Box::new(failure), engine)),
        };
        let previous = self.engines.insert(
            token,
            NativeEngineEntry {
                publication: NativeEnginePublication::Reserved,
                state: engine,
            },
        );
        debug_assert!(previous.is_none(), "native engine tokens are never reused");
        Ok(token)
    }

    fn acquire(&self, token: MermanNativeEngineToken) -> Option<Arc<NativeEngineState>> {
        self.engines.get(&token).and_then(|entry| {
            (entry.publication == NativeEnginePublication::Published)
                .then(|| Arc::clone(&entry.state))
        })
    }

    fn cancel_reservation(
        &mut self,
        token: MermanNativeEngineToken,
    ) -> Option<Arc<NativeEngineState>> {
        if self
            .engines
            .get(&token)
            .is_none_or(|entry| entry.publication != NativeEnginePublication::Reserved)
        {
            return None;
        }
        self.engines.remove(&token).map(|entry| entry.state)
    }

    fn retire(&mut self, token: MermanNativeEngineToken) -> Option<Arc<NativeEngineState>> {
        if self
            .engines
            .get(&token)
            .is_none_or(|entry| entry.publication != NativeEnginePublication::Published)
        {
            return None;
        }
        self.engines.remove(&token).map(|entry| entry.state)
    }
}

#[derive(Default)]
struct NativeAllocationRegistry {
    last_counter: u64,
    results: BTreeMap<u64, NativeResultAllocation>,
}

#[derive(Default)]
struct NativeOperationControlRegistry {
    last_counter: u64,
    controls: BTreeMap<MermanNativeOperationControlToken, Arc<OperationControl>>,
}

impl NativeOperationControlRegistry {
    fn issue(
        &mut self,
        timeout_ms: Option<u64>,
    ) -> Result<MermanNativeOperationControlToken, NativeFailure> {
        let control = match timeout_ms {
            Some(timeout_ms) => {
                OperationControl::new().with_deadline(Duration::from_millis(timeout_ms))
            }
            None => OperationControl::new(),
        };
        let token = issue_domain_token(
            &mut self.last_counter,
            MERMAN_NATIVE_OPERATION_CONTROL_TOKEN_DOMAIN_TAG,
            "native operation-control token space is exhausted",
        )?;
        let previous = self.controls.insert(token, Arc::new(control));
        debug_assert!(
            previous.is_none(),
            "operation-control tokens are never reused"
        );
        Ok(token)
    }

    fn acquire(&self, token: MermanNativeOperationControlToken) -> Option<Arc<OperationControl>> {
        self.controls.get(&token).cloned()
    }

    fn release(&mut self, token: MermanNativeOperationControlToken) -> bool {
        self.controls.remove(&token).is_some()
    }
}

struct NativeResultAllocation {
    _data: Vec<u8>,
    _metadata_or_error_json: Vec<u8>,
}

impl NativeAllocationRegistry {
    fn issue_token(&mut self) -> Result<u64, NativeFailure> {
        issue_domain_token(
            &mut self.last_counter,
            MERMAN_NATIVE_RESULT_TOKEN_DOMAIN_TAG,
            "native result allocation token space is exhausted",
        )
    }

    fn publish(&mut self, token: u64, allocation: NativeResultAllocation) {
        debug_assert!(token_has_domain(
            token,
            MERMAN_NATIVE_RESULT_TOKEN_DOMAIN_TAG
        ));
        let previous = self.results.insert(token, allocation);
        debug_assert!(
            previous.is_none(),
            "native result allocation tokens are never reused"
        );
    }
}

fn issue_domain_token(
    last_counter: &mut u64,
    domain_tag: u64,
    exhausted_message: &'static str,
) -> Result<u64, NativeFailure> {
    let counter = last_counter
        .checked_add(1)
        .filter(|counter| *counter <= MERMAN_NATIVE_TOKEN_COUNTER_MAX)
        .ok_or_else(|| {
            NativeFailure::new(MERMAN_NATIVE_STATUS_INTERNAL_ERROR, exhausted_message)
        })?;
    let token = (counter << MERMAN_NATIVE_TOKEN_COUNTER_SHIFT) | domain_tag;
    debug_assert_ne!(token, 0);
    debug_assert!(token <= i64::MAX as u64);
    debug_assert!(token_has_domain(token, domain_tag));
    *last_counter = counter;
    Ok(token)
}

fn token_has_domain(token: u64, domain_tag: u64) -> bool {
    token != 0 && token & MERMAN_NATIVE_TOKEN_DOMAIN_MASK == domain_tag
}

static ENGINE_REGISTRY: OnceLock<Mutex<NativeEngineRegistry>> = OnceLock::new();
static ALLOCATION_REGISTRY: OnceLock<Mutex<NativeAllocationRegistry>> = OnceLock::new();
static OPERATION_CONTROL_REGISTRY: OnceLock<Mutex<NativeOperationControlRegistry>> =
    OnceLock::new();
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

fn native_error_json(failure: &NativeFailure) -> Vec<u8> {
    let mut payload = serde_json::json!({
        "version": MERMAN_NATIVE_RESULT_SCHEMA_VERSION,
        "ok": false,
        "status": failure.status,
        "status_name": merman_native_status_name(failure.status),
        "kind": merman_native_error_kind_name(failure.kind),
        "capability_id": failure.capability_id,
        "message": failure.message.as_ref(),
    });
    if let Some(failure_details) = failure.details.as_deref() {
        let mut details = serde_json::Map::new();
        if let Some(resource) = failure_details.resource {
            details.insert("resource".to_string(), serde_json::json!(resource));
        }
        if let Some(icon_registry) = failure_details.icon_registry.as_ref() {
            details.insert(
                "icon_registry".to_string(),
                serde_json::json!(icon_registry),
            );
        }
        if let Some(cancellation) = failure_details.cancellation {
            details.insert(
                "cancellation".to_string(),
                serde_json::json!({
                    "reason": cancellation.reason,
                    "phase": cancellation.phase,
                }),
            );
        }
        payload["details"] = serde_json::Value::Object(details);
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
        BindingStatus::Cancelled => MERMAN_NATIVE_STATUS_CANCELLED,
    };
    let resource = error.resource_details();
    let icon_registry = error.icon_registry_details().cloned();
    let cancellation = error
        .cancellation_details()
        .map(|details| NativeCancellationDetails {
            reason: details.reason,
            phase: details.phase,
        });
    let mut failure = NativeFailure::classified(
        status,
        error.kind(),
        error.capability_id(),
        None,
        error.message(),
    );
    if resource.is_some() || icon_registry.is_some() || cancellation.is_some() {
        failure.details = Some(Box::new(NativeFailureDetails {
            resource,
            icon_registry,
            cancellation,
        }));
    }
    failure
}

fn native_artifact_contract() -> &'static ValidatedArtifactContract {
    &ARTIFACT_CONTRACT
}

fn engine_registry() -> &'static Mutex<NativeEngineRegistry> {
    ENGINE_REGISTRY.get_or_init(|| Mutex::new(NativeEngineRegistry::default()))
}

fn allocation_registry() -> &'static Mutex<NativeAllocationRegistry> {
    ALLOCATION_REGISTRY.get_or_init(|| Mutex::new(NativeAllocationRegistry::default()))
}

fn operation_control_registry() -> &'static Mutex<NativeOperationControlRegistry> {
    OPERATION_CONTROL_REGISTRY.get_or_init(|| Mutex::new(NativeOperationControlRegistry::default()))
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
    let retired = {
        let mut registry = allocation_registry()
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        registry.results.remove(&token)
    };
    drop(retired);
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

unsafe fn result_is_writable(out_result: *mut MermanNativeResult) -> Result<(), NativeFailure> {
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

unsafe fn write_native_result(
    out_result: *mut MermanNativeResult,
    status: MermanNativeStatus,
    operation: MermanNativeOperationCode,
    media_type: Option<&'static str>,
    data: Vec<u8>,
    metadata_or_error_json: Vec<u8>,
) -> MermanNativeStatus {
    // Issue first so an exhausted token space drops caller-result buffers only after the
    // allocation-registry lock has been released.
    let allocation_token = {
        let mut registry = allocation_registry()
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        registry.issue_token()
    };
    let allocation_token = match allocation_token {
        Ok(token) => token,
        Err(failure) => return failure.status,
    };
    let (allocation, data, metadata_or_error_json) =
        prepare_result_allocation(data, metadata_or_error_json);
    let mut registry = allocation_registry()
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    registry.publish(allocation_token, allocation);
    unsafe {
        write_registered_native_result(
            out_result,
            allocation_token,
            status,
            operation,
            media_type,
            data,
            metadata_or_error_json,
        )
    }
}

#[cfg(test)]
unsafe fn write_native_result_with_registry(
    registry: &mut NativeAllocationRegistry,
    out_result: *mut MermanNativeResult,
    status: MermanNativeStatus,
    operation: MermanNativeOperationCode,
    media_type: Option<&'static str>,
    data: Vec<u8>,
    metadata_or_error_json: Vec<u8>,
) -> MermanNativeStatus {
    let allocation_token = match registry.issue_token() {
        Ok(token) => token,
        Err(failure) => return failure.status,
    };
    let (allocation, data, metadata_or_error_json) =
        prepare_result_allocation(data, metadata_or_error_json);
    registry.publish(allocation_token, allocation);
    unsafe {
        write_registered_native_result(
            out_result,
            allocation_token,
            status,
            operation,
            media_type,
            data,
            metadata_or_error_json,
        )
    }
}

unsafe fn write_registered_native_result(
    out_result: *mut MermanNativeResult,
    allocation_token: u64,
    status: MermanNativeStatus,
    operation: MermanNativeOperationCode,
    media_type: Option<&'static str>,
    data: MermanNativeBuffer,
    metadata_or_error_json: MermanNativeBuffer,
) -> MermanNativeStatus {
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
    let registry = operation_control_registry()
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    registry
        .acquire(token)
        .map(|control| Some((*control).clone()))
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
            .expect("every C ABI 3 operation must retain its frozen numeric code"),
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
            engine_new_with_services: Some(native_engine_new_with_services),
            operation_control_new: Some(native_operation_control_new),
            operation_control_cancel: Some(native_operation_control_cancel),
            operation_control_release: Some(native_operation_control_release),
            execute_collect_controlled: Some(native_execute_collect_controlled),
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
    token: Option<MermanNativeEngineToken>,
}

impl<'registry> PendingNativeEngine<'registry> {
    fn reserve(
        registry: &'registry Mutex<NativeEngineRegistry>,
        state: Arc<NativeEngineState>,
    ) -> Result<Self, NativeFailure> {
        let publication = {
            let mut registry = registry
                .lock()
                .unwrap_or_else(|poisoned| poisoned.into_inner());
            registry.try_reserve(state)
        };
        let token = match publication {
            Ok(token) => token,
            Err((failure, state)) => {
                drop(state);
                return Err(*failure);
            }
        };
        Ok(Self {
            registry,
            token: Some(token),
        })
    }

    fn token(&self) -> MermanNativeEngineToken {
        self.token
            .expect("a pending native engine owns one reserved token")
    }

    unsafe fn commit(mut self, out_engine: *mut MermanNativeEngineToken) {
        let token = self.token();
        let mut registry = self
            .registry
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        let entry = registry
            .engines
            .get_mut(&token)
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
        self.token = None;
    }
}

impl Drop for PendingNativeEngine<'_> {
    fn drop(&mut self) {
        let Some(token) = self.token.take() else {
            return;
        };
        let cancelled = {
            let mut registry = self
                .registry
                .lock()
                .unwrap_or_else(|poisoned| poisoned.into_inner());
            registry.cancel_reservation(token)
        };
        if let Some(state) = cancelled {
            let _ = state.admission.try_close();
            drop(state);
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
            engine_registry(),
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
    let pending = PendingNativeEngine::reserve(registry, state)?;
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
        let retired = {
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
    let timeout = (has_timeout_ms == 1).then_some(timeout_ms);
    let token = {
        let mut registry = operation_control_registry()
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        match registry.issue(timeout) {
            Ok(token) => token,
            Err(failure) => {
                return unsafe {
                    write_native_failure(out_result, MERMAN_NATIVE_OPERATION_NONE, &failure)
                };
            }
        }
    };
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
        let _ = operation_control_registry()
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .release(token);
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
        let control = {
            let registry = operation_control_registry()
                .lock()
                .unwrap_or_else(|poisoned| poisoned.into_inner());
            registry.acquire(control)
        };
        let Some(control) = control else {
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
        let mut registry = operation_control_registry()
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        if registry.release(control) {
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
    let _ = catch_unwind(AssertUnwindSafe(|| unsafe {
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
        assert!(api.engine_new_with_services.is_some());
        assert!(api.operation_control_new.is_some());
        assert!(api.operation_control_cancel.is_some());
        assert!(api.operation_control_release.is_some());
        assert!(api.execute_collect_controlled.is_some());
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
            MERMAN_NATIVE_API_EXECUTE_COLLECT_CONTROLLED_PREFIX_SIZE,
            native_struct_size::<MermanNativeApi>()
        );
        assert!(buffer.api.metadata_collect.is_some());
        assert!(buffer.api.engine_new_with_services.is_some());
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
            unsafe {
                api.execute_collect_controlled.unwrap()(engine, control, &request, &mut result)
            },
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
            allocation_registry()
                .lock()
                .unwrap_or_else(|poisoned| poisoned.into_inner())
                .results
                .contains_key(&allocation_token),
            "misaligned result_free must be a no-op rather than releasing an unreadable record"
        );

        let mut recovered = unsafe { ptr::read_unaligned(misaligned_live_result) };
        unsafe { api.result_free.unwrap()(&mut recovered) };
        assert!(
            !allocation_registry()
                .lock()
                .unwrap_or_else(|poisoned| poisoned.into_inner())
                .results
                .contains_key(&allocation_token)
        );
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
            unsafe {
                api.metadata_collect.unwrap()(overflowing_native_slice(), &mut metadata_result)
            },
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
    fn engine_new_writes_structured_failure_for_null_out_engine() {
        let api = api_table();
        let config = native_config();
        let mut result = native_result();

        let status = unsafe { api.engine_new.unwrap()(&config, ptr::null_mut(), &mut result) };

        assert_eq!(status, MERMAN_NATIVE_STATUS_INVALID_ARGUMENT);
        assert_structured_invalid_out_engine_failure(
            &api,
            &mut result,
            "out_engine must not be null",
        );
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
        assert_structured_invalid_out_engine_failure(
            &api,
            &mut result,
            "out_engine must not be null",
        );
    }

    #[test]
    fn engine_new_with_services_writes_structured_failure_for_misaligned_out_engine() {
        let api = api_table();
        let config = native_services_config(native_config(), &[]);
        let (_engine_storage, out_engine) = misaligned_record(0 as MermanNativeEngineToken);
        let mut result = native_result();

        let status =
            unsafe { api.engine_new_with_services.unwrap()(&config, out_engine, &mut result) };

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
        assert_structured_invalid_out_engine_failure(
            &api,
            &mut result,
            "out_engine must not be null",
        );

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
                unsafe {
                    api.engine_new_with_services.unwrap()(&config, &mut candidate, &mut result)
                },
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
            let failure_details = failure
                .details
                .expect("structured failure must retain details");
            let resource = failure_details
                .resource
                .expect("resource failure must retain resource details");
            assert_eq!(resource.limit_id, limit.stable_id());
            assert_eq!(resource.phase, limit.descriptor().phase);
            assert_eq!(resource.actual, actual);
            assert_eq!(resource.max, limit.fixed_value());
            assert_eq!(resource.profile, "constructor-fixed");
            let details = failure_details
                .icon_registry
                .expect("resource failure must retain icon registry details");
            assert_eq!(details.kind_id, "resource_limit_exceeded");
            assert_eq!(details.pack_index, Some(pack_index));
            assert_eq!(details.registration_name, None);
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
        overlapping_array.icon_packs =
            ptr::addr_of!(overlapping_array).cast::<MermanNativeIconPack>();
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
                api.engine_new_with_services.unwrap()(
                    &early_semantic_error,
                    &mut engine,
                    &mut result,
                )
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
                api.engine_new_with_services.unwrap()(
                    &oversized_output_array,
                    &mut engine,
                    &mut result,
                )
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
        let config =
            native_services_config(native_config(), std::slice::from_ref(&malformed_alias));
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
            serde_json::json!(["host-text-measurement", "icon-registry"])
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
        let capability_ids = catalog["capabilities"]["capability_ids"]
            .as_array()
            .unwrap();
        for (capability_id, expected) in [
            ("layout-cytoscape", cfg!(feature = "layout-cytoscape")),
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
            last_counter: MERMAN_NATIVE_TOKEN_COUNTER_MAX,
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
    fn engine_and_result_tokens_are_disjoint_positive_signed_64_domains() {
        let mut engines = NativeEngineRegistry::default();
        let mut results = NativeAllocationRegistry::default();

        let engine = engines.issue_token().expect("first engine token");
        let result = results.issue_token().expect("first result token");
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

        engines.last_counter = MERMAN_NATIVE_TOKEN_COUNTER_MAX - 1;
        results.last_counter = MERMAN_NATIVE_TOKEN_COUNTER_MAX - 1;
        let final_engine = engines.issue_token().expect("last engine token");
        let final_result = results.issue_token().expect("last result token");
        assert!(i64::try_from(final_engine).is_ok());
        assert!(i64::try_from(final_result).is_ok());
        assert!(engines.issue_token().is_err());
        assert!(results.issue_token().is_err());
    }

    #[test]
    fn engine_token_exhaustion_returns_state_for_lock_free_destruction() {
        let state = Arc::new(NativeEngineState {
            engine: native_artifact_contract()
                .create_engine(&[])
                .expect("test engine"),
            admission: BindingEngineAdmission::new(BindingEngineAdmissionMode::Concurrent),
        });
        let weak = Arc::downgrade(&state);
        let mut registry = NativeEngineRegistry {
            last_counter: MERMAN_NATIVE_TOKEN_COUNTER_MAX,
            engines: BTreeMap::new(),
        };

        let (failure, returned_state) = registry
            .try_reserve(state)
            .expect_err("exhausted engine token space must reject publication");
        assert_eq!(failure.status, MERMAN_NATIVE_STATUS_INTERNAL_ERROR);
        assert!(registry.engines.is_empty());
        assert!(weak.upgrade().is_some());
        drop(returned_state);
        assert!(
            weak.upgrade().is_none(),
            "the caller can destroy the complete engine graph after releasing the registry lock"
        );
    }

    #[test]
    fn reserved_engine_is_not_acquirable_until_success_outputs_are_committed() {
        let registry = Mutex::new(NativeEngineRegistry::default());
        let state = Arc::new(NativeEngineState {
            engine: native_artifact_contract()
                .create_engine(&[])
                .expect("test engine"),
            admission: BindingEngineAdmission::new(BindingEngineAdmissionMode::Concurrent),
        });
        let predicted_token =
            (1 << MERMAN_NATIVE_TOKEN_COUNTER_SHIFT) | MERMAN_NATIVE_ENGINE_TOKEN_DOMAIN_TAG;
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
                    assert_eq!(
                        registry.engines[&predicted_token].publication,
                        NativeEnginePublication::Reserved
                    );
                    assert!(registry.acquire(predicted_token).is_none());
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
            .acquire(token)
            .expect("successful commit publishes the engine");
        let _ = published.admission.try_close();
        drop(published);
        let retired = registry
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .retire(token);
        drop(retired);
    }

    #[test]
    fn engine_publication_rolls_back_result_failure_and_panic_before_exposure() {
        let registry = Mutex::new(NativeEngineRegistry::default());
        let state = Arc::new(NativeEngineState {
            engine: native_artifact_contract()
                .create_engine(&[])
                .expect("test engine"),
            admission: BindingEngineAdmission::new(BindingEngineAdmissionMode::Concurrent),
        });
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
                    assert!(
                        registry
                            .acquire(registry.engines.keys().copied().next().unwrap())
                            .is_none()
                    );
                    MERMAN_NATIVE_STATUS_INTERNAL_ERROR
                },
            )
        }
        .expect("a result-writer status is not a constructor failure");
        assert_eq!(status, MERMAN_NATIVE_STATUS_INTERNAL_ERROR);
        assert_eq!(token, 0);
        assert_eq!(result.allocation_token, 0);
        assert!(
            registry
                .lock()
                .unwrap_or_else(|poisoned| poisoned.into_inner())
                .engines
                .is_empty()
        );
        assert!(weak.upgrade().is_none());

        let registry = Mutex::new(NativeEngineRegistry::default());
        let state = Arc::new(NativeEngineState {
            engine: native_artifact_contract()
                .create_engine(&[])
                .expect("test engine"),
            admission: BindingEngineAdmission::new(BindingEngineAdmissionMode::Concurrent),
        });
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
                    assert!(
                        registry
                            .acquire(registry.engines.keys().copied().next().unwrap())
                            .is_none()
                    );
                    panic!("synthetic result writer panic")
                },
            );
        }));
        assert!(unwind.is_err());
        assert_eq!(token, 0);
        assert_eq!(result.allocation_token, 0);
        assert!(
            registry
                .lock()
                .unwrap_or_else(|poisoned| poisoned.into_inner())
                .engines
                .is_empty()
        );
        assert!(weak.upgrade().is_none());
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
        let state = Arc::new(NativeEngineState {
            engine: native_artifact_contract()
                .create_engine(&[])
                .expect("test engine"),
            admission: BindingEngineAdmission::new(BindingEngineAdmissionMode::Concurrent),
        });
        let mut engines = NativeEngineRegistry::default();
        let token = engines.issue_token().expect("engine token");
        engines.publish(token, Arc::clone(&state));

        let mut results = NativeAllocationRegistry {
            last_counter: MERMAN_NATIVE_TOKEN_COUNTER_MAX,
            results: BTreeMap::new(),
        };
        let mut result = native_result();
        assert_eq!(
            unsafe {
                write_native_result_with_registry(
                    &mut results,
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
        assert!(results.results.is_empty());
        assert_eq!(result.allocation_token, 0);

        let acquired = engines.acquire(token).expect("engine remains published");
        let operation = acquired
            .admission
            .enter_operation()
            .expect("engine remains usable");
        drop(operation);
        assert!(engines.retire(token).is_some());
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

    #[cfg(all(
        feature = "svg",
        not(feature = "layout-cytoscape"),
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

        for (capability_id, source) in [
            (
                "layout-cytoscape",
                b"architecture-beta\n  service api(server)[API]".as_slice(),
            ),
            (
                "layout-elk",
                b"---\nconfig:\n  layout: elk\n---\nflowchart TD\nA --> B".as_slice(),
            ),
            ("math", b"flowchart TD\nA[\"$$x^2$$\"] --> B".as_slice()),
        ] {
            assert!(!capability_ids.iter().any(|id| id == capability_id));

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
                serde_json::json!([capability_id])
            );
            assert_eq!(
                plan["missing_capability_ids"],
                serde_json::json!([capability_id])
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
            assert_eq!(error["capability_id"], capability_id);
            unsafe { api.result_free.unwrap()(&mut render_result) };
        }

        let ratex_request = native_request_with_options(
            MERMAN_NATIVE_OPERATION_SVG,
            b"flowchart TD\nA --> B",
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
