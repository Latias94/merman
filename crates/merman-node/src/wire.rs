use merman_bindings_core::{
    ArtifactCapabilitySurface, BindingEngine, BindingError, BindingOperationRequest, BindingStatus,
    TextMeasurementProviderProjection,
};
use serde::{Deserialize, Serialize};

const NODE_WIRE_VERSION: u32 = 1;
const NODE_STATIC_SVG_CAPABILITY_IDS: &[&str] = &["layout-cytoscape", "layout-elk", "math", "svg"];
const NODE_STATIC_SVG_OUTPUT_IDS: &[&str] = &["svg"];
const NODE_STATIC_SVG_OPERATION_IDS: &[&str] =
    &["layout-json", "semantic-json", "svg", "svg-plan-json"];

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
    if serde_json::from_str::<serde_json::Value>(options_json)
        .ok()
        .and_then(|value| value.get("runtime_policy").cloned())
        .is_some_and(|policy| policy == "native")
    {
        return Err(BindingError::missing_capability(
            "system-clock",
            "runtime_policy=native is not available in the Node static-SVG transport",
        ));
    }
    BindingEngine::from_options(options_json.as_bytes())
}

fn node_static_svg_capability_surface() -> Result<ArtifactCapabilitySurface, BindingError> {
    let resolved = merman_bindings_core::binding_transport_capability_surface()
        .project_to_descriptor_target("native", TextMeasurementProviderProjection::VendoredOnly)?
        .runtime_capabilities();
    let capability_ids = resolved
        .capability_ids
        .into_iter()
        .filter(|id| NODE_STATIC_SVG_CAPABILITY_IDS.contains(id))
        .collect();
    let output_ids = resolved
        .output_ids
        .into_iter()
        .filter(|id| NODE_STATIC_SVG_OUTPUT_IDS.contains(id))
        .collect();
    let operation_ids = resolved
        .operation_ids
        .into_iter()
        .filter(|id| NODE_STATIC_SVG_OPERATION_IDS.contains(id))
        .collect();

    ArtifactCapabilitySurface::new_with_operation_ids(
        capability_ids,
        output_ids,
        operation_ids,
        Vec::new(),
        resolved.text_measurement,
    )
}

pub(crate) fn runtime_catalog_wire() -> Result<String, BindingError> {
    serde_json::to_string(&merman_bindings_core::runtime_catalog_for(
        NODE_WIRE_VERSION,
        node_static_svg_capability_surface()?,
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

    if !NODE_STATIC_SVG_OPERATION_IDS.contains(&request.operation_id.as_str()) {
        let error = match merman_bindings_core::BindingOperationKind::from_id(&request.operation_id)
        {
            Ok(operation) => operation.required_capability_id().map_or_else(
                || {
                    BindingError::unknown_operation(format!(
                        "operation `{}` is not exposed by the Node static-SVG transport",
                        request.operation_id
                    ))
                },
                |capability_id| {
                    BindingError::missing_capability(
                        capability_id,
                        format!(
                            "operation `{}` requires capability `{capability_id}`, which is not available in the Node static-SVG transport",
                            request.operation_id
                        ),
                    )
                },
            ),
            Err(error) => error,
        };
        return error_envelope(&error);
    }

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
    use super::{
        NODE_STATIC_SVG_CAPABILITY_IDS, NODE_STATIC_SVG_OPERATION_IDS, NODE_STATIC_SVG_OUTPUT_IDS,
        create_engine, error_envelope, execute_wire, runtime_catalog_wire,
    };

    #[test]
    fn runtime_catalog_reports_only_callable_text_measurement_providers() {
        let catalog: serde_json::Value =
            serde_json::from_str(&runtime_catalog_wire().unwrap()).unwrap();
        let capabilities = &catalog["capabilities"];
        let resolved = merman_bindings_core::binding_transport_capability_surface()
            .project_to_descriptor_target(
                "native",
                merman_bindings_core::TextMeasurementProviderProjection::VendoredOnly,
            )
            .unwrap()
            .runtime_capabilities();
        let expected_capability_ids = resolved
            .capability_ids
            .iter()
            .copied()
            .filter(|id| NODE_STATIC_SVG_CAPABILITY_IDS.contains(id))
            .collect::<Vec<_>>();
        let expected_output_ids = resolved
            .output_ids
            .iter()
            .copied()
            .filter(|id| NODE_STATIC_SVG_OUTPUT_IDS.contains(id))
            .collect::<Vec<_>>();
        let expected_operation_ids = resolved
            .operation_ids
            .iter()
            .copied()
            .filter(|id| NODE_STATIC_SVG_OPERATION_IDS.contains(id))
            .collect::<Vec<_>>();
        assert_eq!(
            capabilities["capability_ids"],
            serde_json::json!(expected_capability_ids)
        );
        assert_eq!(
            capabilities["output_ids"],
            serde_json::json!(expected_output_ids)
        );
        assert_eq!(
            capabilities["operation_ids"],
            serde_json::json!(expected_operation_ids)
        );
        assert_eq!(capabilities["system_adapter_ids"], serde_json::json!([]));
        if expected_capability_ids.contains(&"svg") {
            assert_eq!(
                capabilities["text_measurement"]["provider_ids"],
                serde_json::json!(["vendored"])
            );
        } else {
            assert!(capabilities["text_measurement"].is_null());
        }
    }

    #[test]
    fn static_svg_transport_preserves_missing_and_unknown_operation_errors() {
        let native = match create_engine(r#"{"runtime_policy":"native"}"#) {
            Ok(_) => panic!("Node static-SVG transport accepted native runtime policy"),
            Err(error) => error,
        };
        assert_eq!(
            native.status(),
            merman_bindings_core::BindingStatus::UnsupportedOperation
        );
        assert_eq!(
            native.kind(),
            merman_bindings_core::BindingErrorKind::MissingCapability
        );
        assert_eq!(native.capability_id(), Some("system-clock"));

        let engine = create_engine("").unwrap();
        let missing: serde_json::Value = serde_json::from_str(&execute_wire(
            &engine,
            r#"{"operation_id":"png","source":"flowchart TD\nA-->B","uri":null}"#,
        ))
        .expect("Node error envelope");
        assert_eq!(
            missing["error"]["code_name"],
            "MERMAN_UNSUPPORTED_OPERATION"
        );
        assert_eq!(missing["error"]["kind"], "missing-capability");
        assert_eq!(missing["error"]["capability_id"], "png");
        assert!(
            missing["error"]["message"]
                .as_str()
                .is_some_and(|message| message.contains("Node static-SVG transport"))
        );

        let unknown: serde_json::Value = serde_json::from_str(&execute_wire(
            &engine,
            r#"{"operation_id":"bitmap","source":"flowchart TD\nA-->B","uri":null}"#,
        ))
        .expect("Node error envelope");
        assert_eq!(unknown["error"]["kind"], "unknown-operation");
        assert!(unknown["error"]["capability_id"].is_null());
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
