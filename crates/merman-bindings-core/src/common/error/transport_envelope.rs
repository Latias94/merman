use serde::Serialize;
use serde_json::Value;

use super::{
    BindingCancellationErrorDetails, BindingDiagnosticErrorDetails, BindingError, BindingErrorKind,
    BindingIconRegistryErrorDetails, BindingResourceErrorDetails, BindingStatus,
};
use crate::common::BINDING_RESULT_PAYLOAD_VERSION;

/// Fields shared by the flat and nested binding error envelopes.
#[doc(hidden)]
#[derive(Debug, Serialize)]
pub struct BindingErrorBody<'a, D> {
    code: i32,
    code_name: &'static str,
    kind: &'static str,
    capability_id: Option<&'static str>,
    #[serde(skip_serializing_if = "Option::is_none")]
    details: Option<D>,
    message: &'a str,
}

impl<'a, D> BindingErrorBody<'a, D> {
    fn new(
        status: BindingStatus,
        kind: BindingErrorKind,
        capability_id: Option<&'static str>,
        details: Option<D>,
        message: &'a str,
    ) -> Self {
        Self {
            code: status.code(),
            code_name: status.code_name(),
            kind: kind.id(),
            capability_id,
            details,
            message,
        }
    }

    fn from_error(error: &BindingError, details: Option<D>, message: &'a str) -> Self {
        Self::new(
            error.status(),
            error.kind(),
            error.capability_id(),
            details,
            message,
        )
    }
}

impl<'a> BindingErrorBody<'a, Value> {
    /// Builds a body with JS-safe resource counts and the key order of the JSON details map.
    pub fn with_js_details(
        error: &BindingError,
        message: &'a str,
    ) -> Result<Self, serde_json::Error> {
        Ok(Self::from_error(
            error,
            binding_error_js_details_json(error)?,
            message,
        ))
    }
}

#[derive(Serialize)]
struct FlatErrorPayload<'a, D> {
    version: u32,
    ok: bool,
    #[serde(flatten)]
    error: BindingErrorBody<'a, D>,
}

#[derive(Serialize)]
struct ErrorDetails<'a, R> {
    #[serde(skip_serializing_if = "Option::is_none")]
    resource: Option<R>,
    #[serde(skip_serializing_if = "Option::is_none")]
    diagnostic: Option<&'a BindingDiagnosticErrorDetails>,
    #[serde(skip_serializing_if = "Option::is_none")]
    icon_registry: Option<&'a BindingIconRegistryErrorDetails>,
    #[serde(skip_serializing_if = "Option::is_none")]
    cancellation: Option<&'a BindingCancellationErrorDetails>,
}

pub fn error_payload_json_bytes(status: BindingStatus, message: &str) -> Vec<u8> {
    flat_error_json_bytes(BindingErrorBody::<()>::new(
        status,
        BindingErrorKind::Generic,
        None,
        None,
        message,
    ))
}

pub fn binding_error_payload_json_bytes(error: &BindingError) -> Vec<u8> {
    flat_binding_error_json_bytes(error, std::convert::identity)
}

/// Serializes a binding error for JavaScript transports without losing wide resource counts.
#[doc(hidden)]
pub fn binding_error_js_payload_json_bytes(error: &BindingError) -> Vec<u8> {
    flat_binding_error_json_bytes(error, BindingResourceErrorDetails::js_safe_json)
}

/// Projects complete error details with ordinary JSON integer counts.
#[doc(hidden)]
pub fn binding_error_details_json(
    error: &BindingError,
) -> Result<Option<Value>, serde_json::Error> {
    project_details(error, std::convert::identity)
        .map(serde_json::to_value)
        .transpose()
}

/// Projects the complete error details for JavaScript transports.
///
/// Resource counts above JavaScript's exact integer range use canonical decimal strings.
#[doc(hidden)]
pub fn binding_error_js_details_json(
    error: &BindingError,
) -> Result<Option<Value>, serde_json::Error> {
    project_details(error, BindingResourceErrorDetails::js_safe_json)
        .map(serde_json::to_value)
        .transpose()
}

fn project_details<R>(
    error: &BindingError,
    resource: impl FnOnce(BindingResourceErrorDetails) -> R,
) -> Option<ErrorDetails<'_, R>> {
    error.details.as_deref().map(|details| ErrorDetails {
        resource: details.resource.map(resource),
        diagnostic: details.diagnostic.as_ref(),
        icon_registry: details.icon_registry.as_ref(),
        cancellation: details.cancellation.as_ref(),
    })
}

fn flat_binding_error_json_bytes<R: Serialize>(
    error: &BindingError,
    resource: impl FnOnce(BindingResourceErrorDetails) -> R,
) -> Vec<u8> {
    let details = project_details(error, resource).filter(|details| {
        details.resource.is_some()
            || details.diagnostic.is_some()
            || details.icon_registry.is_some()
            || details.cancellation.is_some()
    });
    flat_error_json_bytes(BindingErrorBody::from_error(
        error,
        details,
        error.message(),
    ))
}

fn flat_error_json_bytes(body: BindingErrorBody<'_, impl Serialize>) -> Vec<u8> {
    let payload = FlatErrorPayload {
        version: BINDING_RESULT_PAYLOAD_VERSION,
        ok: false,
        error: body,
    };
    serde_json::to_vec(&payload).unwrap_or_else(|_| {
        format!(
            r#"{{"version":{},"ok":false,"code":{},"code_name":"{}","kind":"generic","capability_id":null,"message":"internal error payload serialization failed"}}"#,
            BINDING_RESULT_PAYLOAD_VERSION,
            BindingStatus::InternalError.code(),
            BindingStatus::InternalError.code_name()
        )
        .into_bytes()
    })
}

#[cfg(test)]
mod tests {
    use super::{
        BindingError, binding_error_js_payload_json_bytes, binding_error_payload_json_bytes,
    };

    #[test]
    fn flat_error_payload_keeps_field_order_and_resource_count_encoding() {
        let error = BindingError::resource_limit(
            "layout_model",
            "max_layout_work_units",
            9_007_199_254_740_992,
            9_007_199_254_740_991,
            "interactive",
            "too large",
        );
        let prefix = concat!(
            r#"{"version":1,"ok":false,"code":10,"code_name":"MERMAN_RESOURCE_LIMIT_EXCEEDED","#,
            r#""kind":"generic","capability_id":null,"details":{"resource":{"cause":"ceiling","#,
            r#""limit_id":"max_layout_work_units","phase":"layout_model","actual":"#,
        );
        let suffix = r#","max":9007199254740991,"profile":"interactive"}},"message":"too large"}"#;
        assert_eq!(
            binding_error_payload_json_bytes(&error),
            format!("{prefix}9007199254740992{suffix}").as_bytes(),
        );
        assert_eq!(
            binding_error_js_payload_json_bytes(&error),
            format!(r#"{prefix}"9007199254740992"{suffix}"#).as_bytes(),
        );
    }
}
