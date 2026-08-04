use merman_bindings_core::{
    ArtifactContractSpec, BindingEngine, BindingError, BindingOperationRequest,
    BindingPayloadSchemaKey, BindingStatus, BindingTransportKey, CapabilityKey, OperationKey,
    RuntimePolicyExposure, TargetKey, ValidatedArtifactContract,
};
use serde::{Deserialize, Serialize};

const NODE_TRANSPORT_API_VERSION: u32 = 1;
const NODE_BINDING_RESULT_PAYLOAD_VERSION: u32 = BindingPayloadSchemaKey::BindingResult.version();
const NODE_TARGET: TargetKey = if cfg!(target_arch = "wasm32") {
    TargetKey::Web
} else {
    TargetKey::Native
};
const NODE_STATIC_SVG_CAPABILITIES: &[CapabilityKey] = &[
    #[cfg(feature = "layout-cytoscape")]
    CapabilityKey::LayoutCytoscape,
    #[cfg(feature = "layout-elk")]
    CapabilityKey::LayoutElk,
    #[cfg(feature = "math")]
    CapabilityKey::Math,
];
const NODE_STATIC_SVG_OPERATIONS: &[OperationKey] = &[
    #[cfg(feature = "svg")]
    OperationKey::LayoutJson,
    OperationKey::SemanticJson,
    #[cfg(feature = "svg")]
    OperationKey::Svg,
    #[cfg(feature = "svg")]
    OperationKey::SvgPlanJson,
];
static ARTIFACT_CONTRACT: ValidatedArtifactContract =
    ArtifactContractSpec::new(NODE_TARGET, BindingTransportKey::Node)
        .with_operations(NODE_STATIC_SVG_OPERATIONS)
        .with_supplemental_capabilities(NODE_STATIC_SVG_CAPABILITIES)
        .with_all_available_metadata()
        .with_runtime_policy_exposure(RuntimePolicyExposure::DeterministicOnly)
        .materialize();

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
    node_artifact_contract().create_engine(options_json.as_bytes())
}

fn node_artifact_contract() -> &'static ValidatedArtifactContract {
    &ARTIFACT_CONTRACT
}

pub(crate) fn runtime_catalog_wire() -> Result<String, BindingError> {
    let bytes = node_artifact_contract().runtime_catalog_json(NODE_TRANSPORT_API_VERSION)?;
    String::from_utf8(bytes).map_err(|error| {
        BindingError::internal(format!("Node runtime catalog was not UTF-8: {error}"))
    })
}

pub(crate) fn metadata_wire(id: &str) -> Result<String, BindingError> {
    let bytes = node_artifact_contract().metadata_json(id)?;
    String::from_utf8(bytes)
        .map_err(|error| BindingError::internal(format!("Node metadata was not UTF-8: {error}")))
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

    let result = engine.execute(
        BindingOperationRequest::new(&request.operation_id, request.source.as_bytes())
            .with_optional_uri(request.uri.as_deref().map(str::as_bytes))
            .with_options_json(request.options_json.as_deref().map_or(b"", str::as_bytes)),
    );

    match result {
        Ok(result) => {
            let (operation, media_type, data, metadata) = result.into_parts();
            let data = match String::from_utf8(data) {
                Ok(data) => data,
                Err(error) => {
                    return error_envelope(&BindingError::new(
                        BindingStatus::InternalError,
                        format!(
                            "Node static-SVG candidate received non-UTF-8 output for `{}`: {error}",
                            operation.operation_id()
                        ),
                    ));
                }
            };
            let metadata_json = match String::from_utf8(metadata.into_json_bytes()) {
                Ok(metadata_json) => metadata_json,
                Err(error) => {
                    return error_envelope(&BindingError::new(
                        BindingStatus::InternalError,
                        format!("binding metadata was not UTF-8: {error}"),
                    ));
                }
            };
            serialize_envelope(&SuccessEnvelope {
                version: NODE_BINDING_RESULT_PAYLOAD_VERSION,
                ok: true,
                result: SuccessResult {
                    operation_id: operation.operation_id().to_owned(),
                    media_type: media_type.to_owned(),
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
        version: NODE_BINDING_RESULT_PAYLOAD_VERSION,
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
            "version": NODE_BINDING_RESULT_PAYLOAD_VERSION,
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
            "{{\"version\":{NODE_BINDING_RESULT_PAYLOAD_VERSION},\"ok\":false,\"error\":{{\"code\":9,\"code_name\":\"MERMAN_INTERNAL_ERROR\",\"kind\":\"generic\",\"capability_id\":null,\"message\":{}}}}}",
            serde_json::to_string(&format!("failed to serialize Node response: {error}"))
                .unwrap_or_else(|_| "\"failed to serialize Node response\"".to_owned())
        )
    })
}

#[cfg(test)]
mod tests {
    use super::{
        NODE_STATIC_SVG_CAPABILITIES, NODE_STATIC_SVG_OPERATIONS, create_engine, error_envelope,
        execute_wire, metadata_wire, node_artifact_contract, runtime_catalog_wire,
    };

    #[test]
    fn runtime_catalog_reports_only_callable_text_measurement_providers() {
        let catalog: serde_json::Value =
            serde_json::from_str(&runtime_catalog_wire().unwrap()).unwrap();
        let capabilities = &catalog["capabilities"];
        let mut expected_capability_ids = NODE_STATIC_SVG_CAPABILITIES
            .iter()
            .map(|key| key.id())
            .chain(["svg"])
            .collect::<Vec<_>>();
        expected_capability_ids.sort_unstable();
        let expected_operation_ids = NODE_STATIC_SVG_OPERATIONS
            .iter()
            .map(|key| key.id())
            .collect::<Vec<_>>();
        assert_eq!(
            capabilities["capability_ids"],
            serde_json::json!(expected_capability_ids)
        );
        assert_eq!(capabilities["output_ids"], serde_json::json!(["svg"]));
        assert_eq!(
            capabilities["operation_ids"],
            serde_json::json!(expected_operation_ids)
        );
        assert_eq!(capabilities["system_adapter_ids"], serde_json::json!([]));
        assert_eq!(
            capabilities["text_measurement"]["provider_ids"],
            serde_json::json!(["vendored"])
        );
    }

    #[test]
    fn runtime_catalog_metadata_ids_match_the_contract_dispatcher() {
        let catalog: serde_json::Value =
            serde_json::from_str(&runtime_catalog_wire().unwrap()).unwrap();
        let advertised_ids = catalog["metadata_ids"]
            .as_array()
            .expect("Node runtime metadata IDs")
            .iter()
            .map(|id| id.as_str().expect("metadata ID string"))
            .collect::<Vec<_>>();
        let expected_ids = node_artifact_contract()
            .metadata_keys()
            .map(merman_bindings_core::MetadataKey::id)
            .collect::<Vec<_>>();
        assert_eq!(advertised_ids, expected_ids);

        for id in advertised_ids {
            let payload: serde_json::Value =
                serde_json::from_str(&metadata_wire(id).unwrap_or_else(|error| {
                    panic!("advertised Node metadata `{id}` failed: {error:?}")
                }))
                .unwrap_or_else(|error| panic!("Node metadata `{id}` was not JSON: {error}"));
            assert!(!payload.is_null(), "Node metadata `{id}` returned null");
        }

        for key in merman_bindings_core::MetadataKey::ALL {
            if !expected_ids.contains(&key.id()) {
                let error = match metadata_wire(key.id()) {
                    Ok(_) => panic!("unadvertised metadata `{}` succeeded", key.id()),
                    Err(error) => error,
                };
                assert_eq!(
                    error.status(),
                    merman_bindings_core::BindingStatus::UnsupportedOperation
                );
            }
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
            merman_bindings_core::BindingStatus::OptionsJsonError
        );
        assert_eq!(
            native.kind(),
            merman_bindings_core::BindingErrorKind::Generic
        );
        assert_eq!(native.capability_id(), None);
        assert!(native.message().contains("not exposed by target"));

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
                .is_some_and(|message| message.contains("not exposed by target"))
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
