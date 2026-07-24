use merman_bindings_core::{BindingEngine, BindingError, BindingOperationRequest, BindingStatus};
use serde::{Deserialize, Serialize};

const NODE_WIRE_VERSION: u32 = 1;

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct NodeOperationRequest {
    operation_id: String,
    source: String,
    uri: Option<String>,
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
    message: &'a str,
}

pub(crate) fn create_engine(options_json: &str) -> Result<BindingEngine, BindingError> {
    BindingEngine::from_options(options_json.as_bytes())
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
        options_json: b"",
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
    serialize_envelope(&ErrorEnvelope {
        version: NODE_WIRE_VERSION,
        ok: false,
        error: ErrorPayload {
            code: error.status().code(),
            code_name: error.status().code_name(),
            kind: error.kind().id(),
            capability_id: error.capability_id(),
            message: error.message(),
        },
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
