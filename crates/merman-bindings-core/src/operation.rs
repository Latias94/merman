use crate::common::BindingResourceScope;
use crate::{BindingEngine, BindingError, BindingStatus};
use serde::Serialize;

#[allow(dead_code)]
mod capability_operations {
    include!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../capabilities/generated/capability_surface.rs"
    ));
}

pub use capability_operations::{OperationKey, OperationSpec};

/// Stable schema version for binding operation metadata and per-operation options.
pub const BINDING_OPERATION_SCHEMA_VERSION: u32 = 1;

/// A stable, transport-neutral operation selected from the canonical capability descriptor.
///
/// Operation IDs, capability prerequisites, media types, and URI requirements come exclusively
/// from `capabilities/feature-surface-v1.json`. Transport-specific numeric codes are deliberately
/// outside this type.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BindingOperationKind(OperationKey);

impl BindingOperationKind {
    pub fn all() -> impl Iterator<Item = Self> + 'static {
        OperationKey::ALL.iter().copied().map(Self)
    }

    pub fn from_id(id: &str) -> Result<Self, BindingError> {
        OperationKey::from_id(id)
            .map(Self)
            .ok_or_else(|| BindingError::unknown_operation(format!("unknown operation `{id}`")))
    }

    #[must_use]
    pub const fn key(self) -> OperationKey {
        self.0
    }

    #[must_use]
    pub const fn operation_id(self) -> &'static str {
        self.0.spec().id
    }

    #[must_use]
    pub const fn media_type(self) -> &'static str {
        self.0.spec().media_type
    }

    /// Returns the optional feature-surface capability required by this operation.
    ///
    /// Semantic JSON deliberately returns `None`: canonical parsing is a base binding operation,
    /// not a fake `semantic` feature.
    #[must_use]
    pub const fn required_capability_id(self) -> Option<&'static str> {
        self.0.spec().capability_id
    }

    #[must_use]
    pub const fn requires_uri(self) -> bool {
        self.0.spec().requires_uri
    }

    pub(crate) const fn resource_scope(self) -> BindingResourceScope {
        match self.key() {
            OperationKey::AnalysisJson
            | OperationKey::AnalysisFactsJson
            | OperationKey::ValidationJson
            | OperationKey::DocumentAnalysisJson
            | OperationKey::DocumentAnalysisFactsJson => BindingResourceScope::Analysis,
            OperationKey::SemanticJson | OperationKey::Ascii | OperationKey::SvgPlanJson => {
                BindingResourceScope::Model
            }
            OperationKey::LayoutJson => BindingResourceScope::Layout,
            OperationKey::Svg | OperationKey::Png | OperationKey::Jpeg | OperationKey::Pdf => {
                BindingResourceScope::Render
            }
        }
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

/// Executes one operation through the same transport-neutral semantics as a reusable engine.
pub fn execute_once(
    request: BindingOperationRequest<'_>,
) -> Result<BindingOperationResult, BindingError> {
    let operation = resolve_operation_request(&request)?;
    crate::common::validate_one_shot_resource_options(
        request.options_json,
        operation.resource_scope(),
    )?;
    BindingEngine::from_options(request.options_json)?.execute_resolved(
        operation,
        request.source,
        request.uri,
    )
}

impl BindingEngine {
    /// Executes one operation against this immutable reusable engine.
    pub fn execute(
        &self,
        request: BindingOperationRequest<'_>,
    ) -> Result<BindingOperationResult, BindingError> {
        let operation = resolve_operation_request(&request)?;
        let request_engine = self.for_request_options(operation, request.options_json)?;
        let engine = request_engine.as_ref().unwrap_or(self);
        engine.execute_resolved(operation, request.source, request.uri)
    }

    fn execute_resolved(
        &self,
        operation: BindingOperationKind,
        source: &[u8],
        uri: Option<&[u8]>,
    ) -> Result<BindingOperationResult, BindingError> {
        let data = match operation.key() {
            OperationKey::Svg => self.render_svg(source),
            OperationKey::SvgPlanJson => self.svg_plan_json(source),
            OperationKey::Png => self.render_png(source),
            OperationKey::Jpeg => self.render_jpeg(source),
            OperationKey::Pdf => self.render_pdf(source),
            OperationKey::Ascii => self.render_ascii(source),
            OperationKey::SemanticJson => self.parse_json(source),
            OperationKey::LayoutJson => self.layout_json(source),
            OperationKey::AnalysisJson => self.analyze_json(source),
            OperationKey::AnalysisFactsJson => self.analysis_facts_json(source),
            OperationKey::ValidationJson => self.validate_json(source),
            OperationKey::DocumentAnalysisJson => {
                self.analyze_document_json(source, uri.expect("validated document URI presence"))
            }
            OperationKey::DocumentAnalysisFactsJson => self
                .analyze_document_facts_json(source, uri.expect("validated document URI presence")),
        }?;

        let metadata_json = serde_json::to_vec(&BindingOperationMetadata {
            version: BINDING_OPERATION_SCHEMA_VERSION,
            operation_id: operation.operation_id(),
            media_type: operation.media_type(),
            runtime_policy: self.runtime_policy_id(),
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

fn resolve_operation_request(
    request: &BindingOperationRequest<'_>,
) -> Result<BindingOperationKind, BindingError> {
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
    Ok(operation)
}

pub fn compiled_operation_kind_ids() -> Vec<&'static str> {
    BindingOperationKind::all()
        .filter(|operation| operation.is_compiled())
        .map(|operation| operation.operation_id())
        .collect()
}

impl BindingOperationKind {
    const fn is_compiled(self) -> bool {
        match self.key() {
            OperationKey::SemanticJson => true,
            OperationKey::Svg | OperationKey::LayoutJson | OperationKey::SvgPlanJson => {
                cfg!(feature = "svg")
            }
            OperationKey::Png => cfg!(feature = "png"),
            OperationKey::Jpeg => cfg!(feature = "jpeg"),
            OperationKey::Pdf => cfg!(feature = "pdf"),
            OperationKey::Ascii => cfg!(feature = "ascii"),
            OperationKey::AnalysisJson
            | OperationKey::AnalysisFactsJson
            | OperationKey::ValidationJson
            | OperationKey::DocumentAnalysisJson
            | OperationKey::DocumentAnalysisFactsJson => cfg!(feature = "analysis"),
        }
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
            assert_eq!(operation.key().spec().id, operation.operation_id());
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
    fn execute_once_uses_the_same_operation_result_contract() {
        let request = BindingOperationRequest {
            operation_id: "semantic-json",
            source: b"flowchart TD\nA --> B",
            uri: None,
            options_json: b"",
        };
        let one_shot = execute_once(request).unwrap();
        let reusable = BindingEngine::from_options(b"")
            .unwrap()
            .execute(request)
            .unwrap();

        assert_eq!(one_shot, reusable);
    }

    #[test]
    fn one_shot_options_reject_limits_owned_by_another_operation() {
        let error = execute_once(BindingOperationRequest {
            operation_id: "semantic-json",
            source: b"flowchart TD\nA --> B",
            uri: None,
            options_json: br#"{"resources":{"limits":{"max_svg_bytes":1024}}}"#,
        })
        .unwrap_err();

        assert_eq!(error.status(), BindingStatus::InvalidArgument);
        assert!(error.message().contains("max_svg_bytes"));
        assert!(error.message().contains("semantic-model"));
    }

    #[test]
    fn reusable_request_resource_overlays_only_tighten_the_constructor_ceiling() {
        let engine = BindingEngine::from_options(
            br#"{"resources":{"profile":"constrained","limits":{"max_source_bytes":64}}}"#,
        )
        .unwrap();

        for options_json in [
            br#"{"resources":{"profile":"trusted-native"}}"#.as_slice(),
            br#"{"resources":{"limits":{"max_source_bytes":65}}}"#.as_slice(),
            br#"{"resources":null}"#.as_slice(),
        ] {
            let error = engine
                .execute(BindingOperationRequest {
                    operation_id: "semantic-json",
                    source: b"flowchart TD\nA --> B",
                    uri: None,
                    options_json,
                })
                .unwrap_err();
            assert_eq!(error.status(), BindingStatus::OptionsJsonError);
        }
    }

    #[test]
    fn request_resource_tightening_does_not_mutate_the_reusable_engine() {
        let engine = BindingEngine::from_options(
            br#"{"resources":{"profile":"constrained","limits":{"max_source_bytes":64}}}"#,
        )
        .unwrap();
        let request = BindingOperationRequest {
            operation_id: "semantic-json",
            source: b"flowchart TD\nA --> B",
            uri: None,
            options_json: br#"{"resources":{"limits":{"max_source_bytes":4}}}"#,
        };
        let error = engine.execute(request).unwrap_err();
        assert_eq!(error.status(), BindingStatus::ResourceLimitExceeded);

        let baseline = engine
            .execute(BindingOperationRequest {
                options_json: b"",
                ..request
            })
            .unwrap();
        assert!(!baseline.data.is_empty());
    }

    #[test]
    fn one_shot_operation_may_choose_a_nondefault_profile() {
        let result = execute_once(BindingOperationRequest {
            operation_id: "semantic-json",
            source: b"flowchart TD\nA --> B",
            uri: None,
            options_json: br#"{"resources":{"profile":"trusted-native"}}"#,
        })
        .unwrap();

        assert!(!result.data.is_empty());
    }

    #[cfg(feature = "svg")]
    #[test]
    fn reusable_constructor_accepts_artifact_union_but_request_scope_rejects_sibling_limit() {
        let engine = BindingEngine::from_options(
            br#"{"resources":{"profile":"constrained","limits":{"max_svg_bytes":1048576}}}"#,
        )
        .unwrap();
        let error = engine
            .execute(BindingOperationRequest {
                operation_id: "semantic-json",
                source: b"flowchart TD\nA --> B",
                uri: None,
                options_json: br#"{"resources":{"limits":{"max_svg_bytes":524288}}}"#,
            })
            .unwrap_err();

        assert_eq!(error.status(), BindingStatus::InvalidArgument);
        assert!(error.message().contains("max_svg_bytes"));
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
