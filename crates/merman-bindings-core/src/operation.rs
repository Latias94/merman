use crate::{BindingEngine, BindingError, BindingStatus};
use serde::Serialize;

mod native_operations {
    include!("generated/native_operations.rs");
}

use native_operations::{
    NATIVE_OPERATIONS, NativeOperationProjection, native_operation_by_code, native_operation_by_id,
};

/// Stable schema version for binding operation metadata and per-operation options.
pub const BINDING_OPERATION_SCHEMA_VERSION: u32 = 1;

/// A stable, transport-neutral operation selected from the canonical capability descriptor.
///
/// Native ABI numeric codes come from `abi/merman-v3.json`; operation IDs, capability
/// prerequisites, media types, and URI requirements come from
/// `capabilities/feature-surface-v1.json`. This wrapper intentionally does not expose a second
/// enum with manually maintained semantics.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BindingOperationKind(&'static NativeOperationProjection);

impl BindingOperationKind {
    pub fn all() -> impl Iterator<Item = Self> + 'static {
        NATIVE_OPERATIONS
            .iter()
            .filter(|operation| operation.operation_id.is_some())
            .map(Self)
    }

    pub fn from_id(id: &str) -> Result<Self, BindingError> {
        native_operation_by_id(id)
            .map(Self)
            .ok_or_else(|| BindingError::unknown_operation(format!("unknown operation `{id}`")))
    }

    pub fn from_native_code(code: i32) -> Result<Self, BindingError> {
        native_operation_by_code(code)
            .filter(|operation| operation.operation_id.is_some())
            .map(Self)
            .ok_or_else(|| {
                BindingError::unknown_operation(format!("unknown operation code `{code}`"))
            })
    }

    #[must_use]
    pub fn operation_id(self) -> &'static str {
        self.0
            .operation_id
            .expect("BindingOperationKind never wraps the native metadata-only operation")
    }

    #[must_use]
    pub const fn native_code(self) -> i32 {
        self.0.code
    }

    #[must_use]
    pub fn media_type(self) -> &'static str {
        self.0
            .media_type
            .expect("BindingOperationKind always has a generated media type")
    }

    /// Returns the optional feature-surface capability required by this operation.
    ///
    /// Semantic JSON deliberately returns `None`: canonical parsing is a base binding operation,
    /// not a fake `semantic` feature.
    #[must_use]
    pub const fn required_capability_id(self) -> Option<&'static str> {
        self.0.capability_id
    }

    #[must_use]
    pub const fn requires_uri(self) -> bool {
        self.0.requires_uri
    }
}

/// Borrowed request consumed by the shared binding execution path.
#[derive(Debug, Clone, Copy)]
pub struct BindingOperationRequest<'a> {
    pub operation_id: &'a str,
    pub source: &'a [u8],
    pub uri: Option<&'a [u8]>,
    pub options_json: &'a [u8],
}

/// Owned result from a binding operation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BindingOperationResult {
    pub operation: BindingOperationKind,
    pub media_type: &'static str,
    pub data: Vec<u8>,
    pub metadata_json: Vec<u8>,
}

#[derive(Debug, Serialize)]
struct BindingOperationMetadata<'a> {
    version: u32,
    operation_id: &'a str,
    media_type: &'a str,
    runtime_policy: &'a str,
    byte_length: usize,
}

impl BindingEngine {
    /// Executes the one transport-neutral operation path used by native and generated bindings.
    pub fn execute(
        &self,
        request: BindingOperationRequest<'_>,
    ) -> Result<BindingOperationResult, BindingError> {
        let operation = BindingOperationKind::from_id(request.operation_id)?;
        if operation.requires_uri() != request.uri.is_some() {
            return Err(BindingError::new(
                BindingStatus::InvalidArgument,
                format!(
                    "operation `{}` {} a document URI",
                    operation.operation_id(),
                    if operation.requires_uri() {
                        "requires"
                    } else {
                        "does not accept"
                    }
                ),
            ));
        }

        let request_engine = self.for_request_options(request.options_json)?;
        let engine = request_engine.as_ref().unwrap_or(self);
        let data = match operation.operation_id() {
            "svg" => engine.render_svg(request.source),
            "svg-plan-json" => engine.svg_plan_json(request.source),
            "png" => engine.render_png(request.source),
            "jpeg" => engine.render_jpeg(request.source),
            "pdf" => engine.render_pdf(request.source),
            "ascii" => engine.render_ascii(request.source),
            "semantic-json" => engine.parse_json(request.source),
            "layout-json" => engine.layout_json(request.source),
            "analysis-json" => engine.analyze_json(request.source),
            "analysis-facts-json" => engine.analysis_facts_json(request.source),
            "validation-json" => engine.validate_json(request.source),
            "document-analysis-json" => engine.analyze_document_json(
                request.source,
                request.uri.expect("validated document URI presence"),
            ),
            "document-analysis-facts-json" => engine.analyze_document_facts_json(
                request.source,
                request.uri.expect("validated document URI presence"),
            ),
            operation_id => {
                return Err(BindingError::new(
                    BindingStatus::UnsupportedOperation,
                    format!(
                        "native ABI declares operation `{operation_id}` without a bindings-core dispatch"
                    ),
                ));
            }
        }?;

        let metadata_json = serde_json::to_vec(&BindingOperationMetadata {
            version: BINDING_OPERATION_SCHEMA_VERSION,
            operation_id: operation.operation_id(),
            media_type: operation.media_type(),
            runtime_policy: engine.runtime_policy_id(),
            byte_length: data.len(),
        })
        .map_err(|error| {
            BindingError::new(
                BindingStatus::InternalError,
                format!("failed to serialize operation metadata: {error}"),
            )
        })?;

        Ok(BindingOperationResult {
            operation,
            media_type: operation.media_type(),
            data,
            metadata_json,
        })
    }
}

pub fn compiled_operation_kind_ids() -> Vec<&'static str> {
    BindingOperationKind::all()
        .filter(|operation| operation.is_compiled())
        .map(|operation| operation.operation_id())
        .collect()
}

impl BindingOperationKind {
    fn is_compiled(self) -> bool {
        let operation_id = self.operation_id();
        operation_id == "semantic-json"
            || (cfg!(feature = "svg")
                && matches!(operation_id, "svg" | "layout-json" | "svg-plan-json"))
            || (cfg!(feature = "png") && operation_id == "png")
            || (cfg!(feature = "jpeg") && operation_id == "jpeg")
            || (cfg!(feature = "pdf") && operation_id == "pdf")
            || (cfg!(feature = "ascii") && operation_id == "ascii")
            || (cfg!(feature = "analysis")
                && matches!(
                    operation_id,
                    "analysis-json"
                        | "analysis-facts-json"
                        | "validation-json"
                        | "document-analysis-json"
                        | "document-analysis-facts-json"
                ))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn descriptor_owned_operation_ids_round_trip() {
        let operations = BindingOperationKind::all().collect::<Vec<_>>();
        assert_eq!(operations.len(), 13);
        for operation in operations {
            assert_eq!(
                BindingOperationKind::from_id(operation.operation_id()).unwrap(),
                operation
            );
            assert_eq!(
                BindingOperationKind::from_native_code(operation.native_code()).unwrap(),
                operation
            );
            assert!(!operation.media_type().is_empty());
        }
    }

    #[test]
    fn svg_capability_planning_is_a_descriptor_owned_operation() {
        let operation = BindingOperationKind::from_id("svg-plan-json").unwrap();

        assert_eq!(operation.required_capability_id(), Some("svg"));
        assert_eq!(operation.media_type(), "application/json");
        assert!(!operation.requires_uri());
    }

    #[test]
    fn unknown_operation_is_a_typed_error() {
        let error = BindingOperationKind::from_id("bitmap").unwrap_err();
        assert_eq!(error.status(), BindingStatus::UnsupportedOperation);
        assert_eq!(error.kind(), crate::BindingErrorKind::UnknownOperation);
        assert_eq!(error.capability_id(), None);
        assert!(error.message().contains("unknown operation `bitmap`"));
    }

    #[test]
    fn semantic_parse_is_a_base_operation_not_a_fake_feature_capability() {
        let semantic = BindingOperationKind::from_id("semantic-json").unwrap();
        assert_eq!(semantic.required_capability_id(), None);
        assert!(!semantic.requires_uri());

        for operation in BindingOperationKind::all() {
            if operation.requires_uri() {
                assert!(operation.operation_id().starts_with("document-analysis-"));
            }
        }
    }

    #[test]
    fn transport_options_default_to_deterministic_policy() {
        let engine = BindingEngine::from_options(b"").unwrap();
        let result = engine
            .execute(BindingOperationRequest {
                operation_id: "semantic-json",
                source: b"flowchart TD\nA --> B",
                uri: None,
                options_json: b"",
            })
            .unwrap();
        let metadata: serde_json::Value = serde_json::from_slice(&result.metadata_json).unwrap();

        assert_eq!(metadata["runtime_policy"], "deterministic");
    }

    #[test]
    fn operation_request_cannot_override_engine_runtime_policy() {
        let engine = BindingEngine::from_options(b"").unwrap();
        let error = engine
            .execute(BindingOperationRequest {
                operation_id: "semantic-json",
                source: b"flowchart TD\nA --> B",
                uri: None,
                options_json: br#"{"runtime_policy":"native"}"#,
            })
            .unwrap_err();

        assert_eq!(error.status(), BindingStatus::OptionsJsonError);
        assert!(error.message().contains("cannot set runtime_policy"));
    }

    #[test]
    fn operation_request_merges_wrapped_options_over_direct_engine_options() {
        let engine =
            BindingEngine::from_options(br#"{"resources":{"limits":{"max_source_bytes":4096}}}"#)
                .unwrap();
        let result = engine
            .execute(BindingOperationRequest {
                operation_id: "semantic-json",
                source: b"flowchart TD\nA --> B",
                uri: None,
                options_json: br#"{"analysis":{"lint":{"profile":"recommended"}}}"#,
            })
            .unwrap();
        assert!(!result.data.is_empty());
    }

    #[test]
    fn explicit_native_policy_matches_the_owner_compiled_adapter_probe() {
        let compiled = merman::runtime::compiled_system_adapter_ids();
        let missing = ["system-clock", "system-timezone", "system-random"]
            .into_iter()
            .find(|capability| !compiled.contains(capability));

        match missing {
            Some(expected_capability) => {
                let error = match BindingEngine::from_options(br#"{"runtime_policy":"native"}"#) {
                    Ok(_) => panic!("owner probe reported a missing native adapter"),
                    Err(error) => error,
                };
                assert_eq!(error.status(), BindingStatus::UnsupportedOperation);
                assert_eq!(error.kind(), crate::BindingErrorKind::MissingCapability);
                assert_eq!(error.capability_id(), Some(expected_capability));
                assert!(error.message().contains("runtime capability `system-"));
                assert!(error.message().contains("not compiled into this artifact"));

                let free_function_error =
                    crate::parse_json(b"flowchart TD\nA --> B", br#"{"runtime_policy":"native"}"#)
                        .expect_err("free functions must honor the selected runtime policy");
                assert_eq!(
                    free_function_error.kind(),
                    crate::BindingErrorKind::MissingCapability
                );
                assert_eq!(
                    free_function_error.capability_id(),
                    Some(expected_capability)
                );
            }
            None => {
                let engine =
                    BindingEngine::from_options(br#"{"runtime_policy":"native"}"#).unwrap();
                let result = engine
                    .execute(BindingOperationRequest {
                        operation_id: "semantic-json",
                        source: b"flowchart TD\nA --> B",
                        uri: None,
                        options_json: b"",
                    })
                    .unwrap();
                let metadata: serde_json::Value =
                    serde_json::from_slice(&result.metadata_json).unwrap();

                assert_eq!(metadata["runtime_policy"], "native");
            }
        }
    }

    #[cfg(feature = "svg")]
    #[test]
    fn generic_svg_operation_produces_versioned_metadata() {
        let engine = BindingEngine::new(b"").unwrap();
        let result = engine
            .execute(BindingOperationRequest {
                operation_id: "svg",
                source: b"flowchart TD\nA --> B",
                uri: None,
                options_json: b"",
            })
            .unwrap();

        assert_eq!(result.operation.operation_id(), "svg");
        assert_eq!(result.media_type, "image/svg+xml");
        assert!(result.data.starts_with(b"<svg"));
        let metadata: serde_json::Value = serde_json::from_slice(&result.metadata_json).unwrap();
        assert_eq!(metadata["version"], BINDING_OPERATION_SCHEMA_VERSION);
        assert_eq!(metadata["operation_id"], "svg");
        assert_eq!(metadata["byte_length"], result.data.len());
        assert_eq!(metadata["runtime_policy"], "deterministic");
    }

    #[cfg(feature = "svg")]
    #[test]
    fn generic_svg_plan_operation_reports_required_and_missing_capabilities() {
        let engine = BindingEngine::new(b"").unwrap();
        let result = engine
            .execute(BindingOperationRequest {
                operation_id: "svg-plan-json",
                source: b"flowchart TD\nA --> B",
                uri: None,
                options_json: b"",
            })
            .unwrap();

        assert_eq!(result.operation.operation_id(), "svg-plan-json");
        assert_eq!(result.media_type, "application/json");
        let plan: serde_json::Value = serde_json::from_slice(&result.data).unwrap();
        assert_eq!(plan["planned_operation_id"], "svg");
        assert_eq!(plan["missing_capability_ids"], serde_json::json!([]));
        assert_eq!(plan["ready"], true);
    }

    #[cfg(feature = "svg")]
    #[test]
    fn request_options_override_nested_engine_options_without_mutating_the_baseline() {
        let engine = BindingEngine::new(
            br#"{
                "environment": { "text_measurement": "deterministic" },
                "svg": { "diagram_id": "base engine", "pipeline": "readable" }
            }"#,
        )
        .unwrap();
        let request_result = engine
            .execute(BindingOperationRequest {
                operation_id: "svg",
                source: b"flowchart TD\nA --> B",
                uri: None,
                options_json: br#"{"svg":{"diagram_id":"request override"}}"#,
            })
            .unwrap();
        let request_svg = String::from_utf8(request_result.data).unwrap();
        assert!(request_svg.contains("id=\"request-override\""));
        assert!(request_svg.contains("data-merman-foreignobject"));

        let baseline_result = engine
            .execute(BindingOperationRequest {
                operation_id: "svg",
                source: b"flowchart TD\nA --> B",
                uri: None,
                options_json: b"",
            })
            .unwrap();
        let baseline_svg = String::from_utf8(baseline_result.data).unwrap();
        assert!(baseline_svg.contains("id=\"base-engine\""));
    }

    #[cfg(all(feature = "svg", not(feature = "layout-elk")))]
    #[test]
    fn missing_layout_engine_is_a_typed_capability_error() {
        let engine = BindingEngine::new(b"").unwrap();
        let error = engine
            .execute(BindingOperationRequest {
                operation_id: "svg",
                source: b"---\nconfig:\n  layout: elk\n---\nflowchart TD\n  A --> B\n",
                uri: None,
                options_json: b"",
            })
            .expect_err("ELK is not compiled");

        assert_eq!(error.status(), BindingStatus::UnsupportedOperation);
        assert_eq!(error.kind(), crate::BindingErrorKind::MissingCapability);
        assert_eq!(error.capability_id(), Some("layout-elk"));
        assert!(error.message().contains("`layout-elk`"));
    }

    #[cfg(feature = "png")]
    #[test]
    fn generic_png_operation_exposes_a_real_binary_output() {
        let engine = BindingEngine::new(b"").unwrap();
        let result = engine
            .execute(BindingOperationRequest {
                operation_id: "png",
                source: b"flowchart TD\nA --> B",
                uri: None,
                options_json: b"",
            })
            .unwrap();

        assert_eq!(result.media_type, "image/png");
        assert!(result.data.starts_with(b"\x89PNG\r\n\x1a\n"));
    }

    #[cfg(not(feature = "png"))]
    #[test]
    fn unavailable_operation_is_reported_before_execution() {
        let engine = BindingEngine::new(b"").unwrap();
        let error = engine
            .execute(BindingOperationRequest {
                operation_id: "png",
                source: b"flowchart TD\nA --> B",
                uri: None,
                options_json: b"",
            })
            .expect_err("PNG is not compiled");

        assert_eq!(error.status(), BindingStatus::UnsupportedOperation);
        assert_eq!(error.kind(), crate::BindingErrorKind::MissingCapability);
        assert_eq!(error.capability_id(), Some("png"));
        assert!(error.message().contains("png feature"));
    }
}
