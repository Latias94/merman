#[cfg(not(feature = "svg"))]
compile_error!("the Node candidates must be built with the direct static SVG feature set");

#[cfg(not(feature = "layout-cytoscape"))]
compile_error!("the Node candidates require the static layout-cytoscape capability recipe");

#[cfg(not(feature = "layout-dagre"))]
compile_error!("the Node candidates require the static layout-dagre capability recipe");

#[cfg(not(feature = "layout-elk"))]
compile_error!("the Node candidates require the static layout-elk capability recipe");

use std::borrow::Cow;
use std::time::Duration;

#[cfg(not(target_arch = "wasm32"))]
use merman_bindings_core::transport_boundary::catch_transport_unwind;
use merman_bindings_core::{
    BINDING_OPERATION_SCHEMA_VERSION, BindingEngine, BindingEngineServices, BindingError,
    BindingErrorBody, BindingErrorKind, BindingStatus, OperationControl, OperationKey,
    OperationPhase, RUNTIME_CATALOG_MAX_SAFE_INTEGER, build_icon_registry_from_json,
};
use serde::Serialize;
use serde_json::Value;

mod contract;
#[cfg(feature = "transport-napi")]
mod direct_svg;
mod json;

use contract::{NODE_BINDING_RESULT_PAYLOAD_VERSION, node_artifact_contract, node_wire_contract};
pub(crate) use contract::{NodeTransportKind, runtime_catalog_wire, transport_identity_wire};
#[cfg(feature = "transport-napi")]
pub(crate) use direct_svg::{
    DirectSvgOptions, direct_svg_field_limits, execute_svg_with_admitted_control,
};
use json::{
    NodeOperationRequest, decode_operation_request_options, deserialize_bounded_json, ensure_field,
    parse_operation_request_framing, serialize_bounded_json, validate_binding_options,
    validate_bounded_json_text,
};
#[cfg(feature = "transport-napi")]
use json::{OPERATION_OPTIONS_JSON_LABEL, scan_bounded_json_text, validate_document_bytes};
#[cfg(feature = "transport-napi")]
pub(crate) use json::{caller_options_error, ensure_field_bytes};

#[derive(Debug, Serialize)]
struct SuccessEnvelope<'a> {
    version: u32,
    ok: bool,
    result: SuccessResult<'a>,
}

#[derive(Debug, Serialize)]
struct SuccessResult<'a> {
    operation_id: &'static str,
    media_type: &'static str,
    data: &'a str,
    #[serde(skip_serializing_if = "Option::is_none")]
    metadata_json: Option<&'a str>,
}

impl<'a> SuccessEnvelope<'a> {
    fn new(operation: OperationKey, data: &'a str, metadata_json: Option<&'a str>) -> Self {
        Self {
            version: NODE_BINDING_RESULT_PAYLOAD_VERSION,
            ok: true,
            result: SuccessResult {
                operation_id: operation.id(),
                media_type: operation.spec().media_type,
                data,
                metadata_json,
            },
        }
    }
}

#[derive(Debug, Serialize)]
struct ErrorEnvelope<'a> {
    version: u32,
    ok: bool,
    error: BindingErrorBody<'a, Value>,
}

#[cfg(test)]
pub(crate) fn create_engine(options_json: &str) -> Result<BindingEngine, BindingError> {
    create_engine_with_icon_packs(options_json, None)
}

pub(crate) fn create_engine_with_icon_packs(
    options_json: &str,
    icon_packs_json: Option<&str>,
) -> Result<BindingEngine, BindingError> {
    binding_boundary(|| {
        validate_binding_options(options_json)?;
        let mut services = BindingEngineServices::new();
        if let Some(packs) = icon_packs_json {
            services =
                services.with_icon_registry(build_icon_registry_from_json(packs.as_bytes())?);
        }
        node_artifact_contract().create_engine_with_services(options_json.as_bytes(), services)
    })
}

pub(crate) fn disposed_error() -> BindingError {
    BindingError::invalid_argument("Node transport engine has been disposed")
}

pub(crate) fn metadata_wire_for_engine(
    engine: &BindingEngine,
    id: &str,
) -> Result<String, BindingError> {
    binding_boundary(|| metadata_wire_inner_for_engine(engine, id))
}

fn metadata_wire_inner_for_engine(
    engine: &BindingEngine,
    id: &str,
) -> Result<String, BindingError> {
    let contract = node_wire_contract();
    if id.is_empty() {
        return Err(BindingError::invalid_argument(
            "Node metadata id must be non-empty",
        ));
    }
    ensure_field(id, "metadata id", contract.fields.metadata_id_utf8_bytes)
        .map_err(caller_argument_error)?;
    let bytes = engine.metadata_json(id)?;
    let text = String::from_utf8(bytes)
        .map_err(|error| BindingError::internal(format!("Node metadata was not UTF-8: {error}")))?;
    deserialize_bounded_json::<Value>(&text, "metadata", contract.documents.metadata)
        .map_err(producer_error)?;
    Ok(text)
}

#[cfg(test)]
pub(crate) fn execute_wire(engine: &BindingEngine, request_json: &str) -> String {
    execute_wire_with_control(engine, request_json, admitted_operation_control(None))
}

#[cfg(test)]
pub(crate) fn execute_wire_with_control(
    engine: &BindingEngine,
    request_json: &str,
    control: OperationControl,
) -> String {
    execute_wire_with_admitted_control(engine, request_json, control, None)
}

pub(crate) fn execute_wire_with_admitted_control(
    engine: &BindingEngine,
    request_json: &str,
    control: OperationControl,
    admitted_timeout_ms: Option<u32>,
) -> String {
    match binding_boundary(|| {
        execute_wire_inner(engine, request_json, control, admitted_timeout_ms)
    }) {
        Ok(response) => response,
        Err(error) => error_envelope(&error),
    }
}

pub(crate) fn admitted_operation_control(timeout_ms: Option<u32>) -> OperationControl {
    timeout_ms.map_or_else(OperationControl::new, |timeout_ms| {
        OperationControl::new().with_deadline(Duration::from_millis(u64::from(timeout_ms)))
    })
}

fn execute_wire_inner(
    engine: &BindingEngine,
    request_json: &str,
    control: OperationControl,
    admitted_timeout_ms: Option<u32>,
) -> Result<String, BindingError> {
    let request = parse_operation_request_framing(request_json)?;
    let operation_request = request.binding_request();
    let operation = operation_request.resolve_operation()?;
    validate_wire_control(&request, &control, admitted_timeout_ms)?;
    let options = decode_operation_request_options(request.options_json.as_deref())?;
    control
        .checkpoint_at(OperationPhase::Admission)
        .map_err(BindingError::cancelled)?;
    let operation_request = operation_request.with_control(control);
    if request.include_metadata {
        success_envelope(engine.execute_with_decoded_options(operation_request, options)?)
    } else {
        success_data_envelope(
            operation.key(),
            engine.execute_data_with_decoded_options(operation_request, options)?,
            None,
        )
    }
}

fn validate_wire_control(
    request: &NodeOperationRequest,
    control: &OperationControl,
    admitted_timeout_ms: Option<u32>,
) -> Result<(), BindingError> {
    control
        .checkpoint_at(OperationPhase::Admission)
        .map_err(BindingError::cancelled)?;
    let request_timeout_ms = request.timeout_ms();
    match (request_timeout_ms, admitted_timeout_ms) {
        (Some(requested), Some(admitted)) if requested != admitted => {
            return Err(BindingError::invalid_argument(
                "Node operation-control timeout does not match transport admission",
            ));
        }
        (None, Some(_)) => {
            return Err(BindingError::invalid_argument(
                "Node transport admitted a timeout missing from the operation request",
            ));
        }
        (Some(_), None) => {
            return Err(BindingError::invalid_argument(
                "Node operation request timeout is missing transport admission",
            ));
        }
        (Some(_), Some(_)) | (None, None) => {}
    }
    Ok(())
}

pub(crate) fn error_envelope(error: &BindingError) -> String {
    match transport_unwind_boundary(|| try_error_envelope(error)) {
        Ok(Ok(envelope)) => envelope,
        Ok(Err(_)) => fallback_error_envelope(),
        Err(()) => fallback_panic_envelope(),
    }
}

fn success_envelope(
    result: merman_bindings_core::BindingOperationResult,
) -> Result<String, BindingError> {
    let contract = node_wire_contract();
    let (operation, media_type, data, metadata) = result.into_parts();
    let operation_id = operation.operation_id();
    let data_length = u64::try_from(data.len()).map_err(|_| {
        BindingError::internal("Node response data length exceeds the unsigned 64-bit range")
    })?;
    ensure_field(
        operation_id,
        "response operation_id",
        contract.fields.operation_id_utf8_bytes,
    )
    .map_err(producer_error)?;
    ensure_field(
        media_type,
        "response media_type",
        contract.fields.media_type_utf8_bytes,
    )
    .map_err(producer_error)?;
    if metadata.version() != BINDING_OPERATION_SCHEMA_VERSION
        || metadata.operation_id() != operation_id
        || metadata.media_type() != media_type
        || metadata.runtime_policy() != "deterministic"
        || metadata.byte_length() != data_length
    {
        return Err(BindingError::internal(
            "Node operation metadata does not match its result envelope",
        ));
    }

    let metadata_json = String::from_utf8(metadata.into_json_bytes()).map_err(|error| {
        BindingError::internal(format!("binding metadata was not UTF-8: {error}"))
    })?;
    ensure_field(
        &metadata_json,
        "response metadata_json",
        contract.fields.metadata_json_utf8_bytes,
    )
    .map_err(producer_error)?;
    deserialize_bounded_json::<Value>(
        &metadata_json,
        "nested operation metadata",
        contract.documents.metadata,
    )
    .map_err(producer_error)?;

    success_data_envelope(operation.key(), data, Some(&metadata_json))
}

fn success_data_envelope(
    operation: OperationKey,
    data: Vec<u8>,
    metadata_json: Option<&str>,
) -> Result<String, BindingError> {
    let data = response_data_text(operation, data)?;
    serialize_bounded_json(
        &SuccessEnvelope::new(operation, &data, metadata_json),
        "response",
        node_wire_contract().documents.response,
    )
    .map_err(producer_error)
}

fn response_data_text(operation: OperationKey, data: Vec<u8>) -> Result<String, BindingError> {
    let data = String::from_utf8(data).map_err(|error| {
        let operation_id = operation.id();
        BindingError::internal(format!(
            "Node static-SVG candidate received non-UTF-8 output for `{operation_id}`: {error}"
        ))
    })?;
    ensure_field(
        &data,
        "response data",
        node_wire_contract().fields.data_utf8_bytes,
    )
    .map_err(producer_error)?;
    Ok(data)
}

fn try_error_envelope(error: &BindingError) -> Result<String, String> {
    let contract = node_wire_contract();
    let fields = contract.fields;
    validate_error_relation(error)?;
    ensure_field(
        error.status().code_name(),
        "error code_name",
        fields.error_code_name_utf8_bytes,
    )?;
    ensure_field(
        error.kind().id(),
        "error kind",
        fields.error_kind_utf8_bytes,
    )?;
    if let Some(capability_id) = error.capability_id() {
        ensure_field(
            capability_id,
            "error capability_id",
            fields.capability_id_utf8_bytes,
        )?;
    }
    let diagnostic = error.diagnostic_details();
    if diagnostic
        .and_then(|details| details.span)
        .is_some_and(|span| {
            span.start > RUNTIME_CATALOG_MAX_SAFE_INTEGER
                || span.end > RUNTIME_CATALOG_MAX_SAFE_INTEGER
        })
    {
        return Err("error diagnostic span exceeds the JSON-safe integer range".to_owned());
    }
    let message = bounded_text(error.message(), fields.error_message_utf8_bytes);
    let error = BindingErrorBody::with_js_details(error, &message)
        .map_err(|error| format!("failed to encode binding error details: {error}"))?;
    let envelope = ErrorEnvelope {
        version: NODE_BINDING_RESULT_PAYLOAD_VERSION,
        ok: false,
        error,
    };
    let json = serialize_bounded_json(&envelope, "error", contract.documents.error)?;
    validate_bounded_json_text(&json, "response", contract.documents.response)?;
    Ok(json)
}

fn validate_error_relation(error: &BindingError) -> Result<(), String> {
    if error.status() == BindingStatus::Ok {
        return Err("error envelopes cannot carry MERMAN_OK".to_owned());
    }
    let valid = match error.kind() {
        BindingErrorKind::Generic => error.capability_id().is_none(),
        BindingErrorKind::UnknownOperation => {
            error.status() == BindingStatus::UnsupportedOperation && error.capability_id().is_none()
        }
        BindingErrorKind::MissingCapability => {
            error.status() == BindingStatus::UnsupportedOperation && error.capability_id().is_some()
        }
        BindingErrorKind::Busy => error.status() == BindingStatus::Busy,
        BindingErrorKind::ReentrantCall => {
            error.status() == BindingStatus::InvalidArgument && error.capability_id().is_none()
        }
    };
    if !valid {
        return Err("binding error kind/status/capability relation is inconsistent".to_owned());
    }
    Ok(())
}

fn fallback_error_envelope() -> String {
    format!(
        "{{\"version\":{NODE_BINDING_RESULT_PAYLOAD_VERSION},\"ok\":false,\"error\":{{\"code\":9,\"code_name\":\"MERMAN_INTERNAL_ERROR\",\"kind\":\"generic\",\"capability_id\":null,\"message\":\"failed to encode a bounded Node transport error\"}}}}"
    )
}

fn fallback_panic_envelope() -> String {
    format!(
        "{{\"version\":{NODE_BINDING_RESULT_PAYLOAD_VERSION},\"ok\":false,\"error\":{{\"code\":8,\"code_name\":\"MERMAN_PANIC\",\"kind\":\"generic\",\"capability_id\":null,\"message\":\"a Rust panic was caught at the Node transport boundary\"}}}}"
    )
}

fn panic_error() -> BindingError {
    BindingError::new(
        BindingStatus::Panic,
        "a Rust panic was caught at the Node transport boundary",
    )
}

fn binding_boundary<T>(
    operation: impl FnOnce() -> Result<T, BindingError>,
) -> Result<T, BindingError> {
    match transport_unwind_boundary(operation) {
        Ok(result) => result,
        Err(()) => Err(panic_error()),
    }
}

#[cfg(not(target_arch = "wasm32"))]
fn transport_unwind_boundary<T>(operation: impl FnOnce() -> T) -> Result<T, ()> {
    catch_transport_unwind(operation).ok_or(())
}

#[cfg(target_arch = "wasm32")]
fn transport_unwind_boundary<T>(operation: impl FnOnce() -> T) -> Result<T, ()> {
    // The wasm-size profile uses panic=abort, so the WebAssembly candidate cannot promise the
    // recoverable native unwind boundary. All ordinary typed failures still share the same wire.
    Ok(operation())
}

fn bounded_text(value: &str, max_utf8_bytes: usize) -> Cow<'_, str> {
    if value.len() <= max_utf8_bytes {
        return Cow::Borrowed(value);
    }
    let suffix = "...";
    let mut end = max_utf8_bytes.saturating_sub(suffix.len());
    while !value.is_char_boundary(end) {
        end = end.saturating_sub(1);
    }
    Cow::Owned(format!("{}{}", &value[..end], suffix))
}

fn caller_argument_error(message: String) -> BindingError {
    BindingError::invalid_argument(format!("invalid Node transport field: {message}"))
}

fn producer_error(message: String) -> BindingError {
    BindingError::internal(format!("invalid Node transport output: {message}"))
}

#[cfg(test)]
mod tests;
