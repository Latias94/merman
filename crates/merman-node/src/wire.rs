use merman_bindings_core::{
    BindingEngine, BindingError, BindingOperationRequest, BindingStatus,
    TextMeasurementProviderProjection,
};
use serde::{Deserialize, Serialize};

const NODE_WIRE_VERSION: u32 = 1;

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct NodeOperationRequest {
    operation_id: String,
    source: String,
    uri: Option<String>,
    #[serde(default)]
    options_json: Option<String>,
}

#[derive(Debug, Serialize)]
struct SuccessEnvelope {
    version: u32,
    ok: bool,
    result: SuccessResult,
}

#[derive(Debug, Serialize)]
struct SuccessResult {
    operation_id: String,
    media_type: String,
    data: String,
    metadata_json: String,
}

#[derive(Debug, Serialize)]
struct ErrorEnvelope<'a> {
    version: u32,
    ok: bool,
    error: ErrorPayload<'a>,
}

#[derive(Debug, Serialize)]
struct ErrorPayload<'a> {
    code: i32,
    code_name: &'a str,
    kind: &'a str,
    capability_id: Option<&'a str>,
    #[serde(skip_serializing_if = "Option::is_none")]
    details: Option<ErrorDetails>,
    message: &'a str,
}

#[derive(Debug, Serialize)]
struct ErrorDetails {
    resource: merman_bindings_core::BindingResourceErrorDetails,
}

pub(crate) fn create_engine(options_json: &str) -> Result<BindingEngine, BindingError> {
    BindingEngine::from_options(options_json.as_bytes())
}

pub(crate) fn runtime_catalog_wire() -> Result<String, BindingError> {
    let surface = merman_bindings_core::binding_transport_capability_surface()
        .project_to_descriptor_target("native", TextMeasurementProviderProjection::VendoredOnly)?;
    serde_json::to_string(&merman_bindings_core::runtime_catalog_for(
        NODE_WIRE_VERSION,
        surface,
    ))
    .map_err(|error| {
        BindingError::new(
            BindingStatus::InternalError,
            format!("failed to serialize the Node runtime catalog: {error}"),
        )
    })
}

pub(crate) fn execute_wire(engine: &BindingEngine, request_json: &str) -> String {
    let request = match serde_json::from_str::<NodeOperationRequest>(request_json) {
        Ok(request) => request,
        Err(error) => {
            return error_envelope(&BindingError::new(
                BindingStatus::OptionsJsonError,
                format!("invalid Node operation request JSON: {error}"),
            ));
        }
    };

    let result = engine.execute(BindingOperationRequest {
        operation_id: &request.operation_id,
        source: request.source.as_bytes(),
        uri: request.uri.as_deref().map(str::as_bytes),
        options_json: request.options_json.as_deref().map_or(b"", str::as_bytes),
    });

    match result {
        Ok(result) => {
            let data = match String::from_utf8(result.data) {
                Ok(data) => data,
                Err(error) => {
                    return error_envelope(&BindingError::new(
                        BindingStatus::InternalError,
                        format!(
                            "Node static-SVG candidate received non-UTF-8 output for `{}`: {error}",
                            result.operation.operation_id()
                        ),
                    ));
                }
            };
            let metadata_json = match String::from_utf8(result.metadata_json) {
                Ok(metadata_json) => metadata_json,
                Err(error) => {
                    return error_envelope(&BindingError::new(
                        BindingStatus::InternalError,
                        format!("binding metadata was not UTF-8: {error}"),
                    ));
                }
            };
            serialize_envelope(&SuccessEnvelope {
                version: NODE_WIRE_VERSION,
                ok: true,
                result: SuccessResult {
                    operation_id: result.operation.operation_id().to_owned(),
                    media_type: result.media_type.to_owned(),
                    data,
                    metadata_json,
                },
            })
        }
        Err(error) => error_envelope(&error),
    }
}

pub(crate) fn error_envelope(error: &BindingError) -> String {
    serialize_envelope(&error_value(error))
}

pub(crate) fn error_value(error: &BindingError) -> serde_json::Value {
    serde_json::to_value(ErrorEnvelope {
        version: NODE_WIRE_VERSION,
        ok: false,
        error: ErrorPayload {
            code: error.status().code(),
            code_name: error.status().code_name(),
            kind: error.kind().id(),
            capability_id: error.capability_id(),
            details: error
                .resource_details()
                .map(|resource| ErrorDetails { resource }),
            message: error.message(),
        },
    })
    .unwrap_or_else(|serialization_error| {
        serde_json::json!({
            "version": NODE_WIRE_VERSION,
            "ok": false,
            "error": {
                "code": 9,
                "code_name": "MERMAN_INTERNAL_ERROR",
                "kind": "generic",
                "capability_id": null,
                "message": format!("failed to serialize Node response: {serialization_error}"),
            },
        })
    })
}

fn serialize_envelope(value: &impl Serialize) -> String {
    serde_json::to_string(value).unwrap_or_else(|error| {
        format!(
            "{{\"version\":{NODE_WIRE_VERSION},\"ok\":false,\"error\":{{\"code\":9,\"code_name\":\"MERMAN_INTERNAL_ERROR\",\"kind\":\"generic\",\"capability_id\":null,\"message\":{}}}}}",
            serde_json::to_string(&format!("failed to serialize Node response: {error}"))
                .unwrap_or_else(|_| "\"failed to serialize Node response\"".to_owned())
        )
    })
}

#[cfg(test)]
mod tests {
    use super::{error_envelope, runtime_catalog_wire};

    #[test]
    fn runtime_catalog_reports_only_callable_text_measurement_providers() {
        let catalog: serde_json::Value =
            serde_json::from_str(&runtime_catalog_wire().unwrap()).unwrap();
        assert_eq!(
            catalog["capabilities"]["text_measurement"]["provider_ids"],
            serde_json::json!(["vendored"])
        );
        assert!(
            !catalog["capabilities"]["capability_ids"]
                .as_array()
                .expect("runtime capability IDs")
                .iter()
                .any(|id| id == "system-timing")
        );
        assert!(
            !catalog["capabilities"]["system_adapter_ids"]
                .as_array()
                .expect("runtime system adapter IDs")
                .iter()
                .any(|id| id == "system-timing")
        );
    }

    #[test]
    fn error_wire_preserves_structured_resource_details() {
        let error = merman_bindings_core::BindingError::resource_limit(
            "embedded_image_decode",
            "max_embedded_image_bytes",
            5,
            4,
            "constrained",
            "embedded image is too large",
        );
        let payload: serde_json::Value =
            serde_json::from_str(&error_envelope(&error)).expect("Node error envelope");

        assert_eq!(
            payload["error"]["details"]["resource"]["limit_id"],
            "max_embedded_image_bytes"
        );
        assert_eq!(
            payload["error"]["details"]["resource"]["profile"],
            "constrained"
        );
    }
}
