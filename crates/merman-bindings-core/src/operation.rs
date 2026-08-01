use crate::resource_contract::BindingResourceScope;
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
            | OperationKey::ValidationJson => BindingResourceScope::AnalysisDiagram,
            OperationKey::DocumentAnalysisJson | OperationKey::DocumentAnalysisFactsJson => {
                BindingResourceScope::DocumentAnalysis
            }
            OperationKey::SemanticJson | OperationKey::SvgPlanJson => BindingResourceScope::Model,
            OperationKey::Ascii => BindingResourceScope::Ascii,
            OperationKey::LayoutJson => BindingResourceScope::Layout,
            OperationKey::Svg => BindingResourceScope::Svg,
            OperationKey::Png => BindingResourceScope::Png,
            OperationKey::Jpeg => BindingResourceScope::Jpeg,
            OperationKey::Pdf => BindingResourceScope::Pdf,
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
#[non_exhaustive]
pub struct BindingOperationResult {
    pub operation: BindingOperationKind,
    pub media_type: &'static str,
    pub data: Vec<u8>,
    pub metadata_json: Vec<u8>,
}

#[derive(Debug)]
pub(crate) struct BindingOperationOutput {
    data: Vec<u8>,
    output_plan: Option<BindingOutputPlan>,
}

#[derive(Debug, Clone, Copy, Serialize)]
#[serde(tag = "kind", rename_all = "kebab-case")]
enum BindingOutputPlan {
    #[cfg(any(feature = "png", feature = "jpeg"))]
    Raster {
        requested_width_px: f64,
        requested_height_px: f64,
        width_px: u32,
        height_px: u32,
        requested_scale: f64,
        effective_scale: f64,
        limited: bool,
    },
    #[cfg(feature = "pdf")]
    PdfFilterImages {
        filtered_groups: usize,
        requested_scale: f32,
        effective_scale: f32,
        requested_image_pixels: u64,
        effective_image_pixels: u64,
        limited: bool,
    },
}

impl BindingOperationOutput {
    pub(crate) fn plain(data: Vec<u8>) -> Self {
        Self {
            data,
            output_plan: None,
        }
    }

    #[cfg(any(feature = "png", feature = "jpeg"))]
    pub(crate) fn raster(data: Vec<u8>, plan: merman::svg::export::RasterPlan) -> Self {
        Self {
            data,
            output_plan: Some(BindingOutputPlan::Raster {
                requested_width_px: plan.requested_width_px,
                requested_height_px: plan.requested_height_px,
                width_px: plan.width_px,
                height_px: plan.height_px,
                requested_scale: plan.requested_scale,
                effective_scale: plan.effective_scale,
                limited: plan.limited,
            }),
        }
    }

    #[cfg(feature = "pdf")]
    pub(crate) fn pdf(data: Vec<u8>, plan: merman::svg::export::PdfFilterImagePlan) -> Self {
        Self {
            data,
            output_plan: Some(BindingOutputPlan::PdfFilterImages {
                filtered_groups: plan.filtered_groups,
                requested_scale: plan.requested_scale,
                effective_scale: plan.effective_scale,
                requested_image_pixels: plan.requested_image_pixels,
                effective_image_pixels: plan.effective_image_pixels,
                limited: plan.limited,
            }),
        }
    }
}

#[derive(Debug, Serialize)]
struct BindingOperationMetadata<'a> {
    version: u32,
    operation_id: &'a str,
    media_type: &'a str,
    runtime_policy: &'a str,
    byte_length: usize,
    #[serde(skip_serializing_if = "Option::is_none")]
    output_plan: Option<&'a BindingOutputPlan>,
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
        if request.options_json.is_empty() {
            return self.execute_resolved(operation, request.source, request.uri);
        }
        let output = self
            .execute_request_overlay(operation, request.source, request.uri, request.options_json)?
            .map_or_else(
                || self.execute_resolved_output(operation, request.source, request.uri),
                Ok,
            )?;
        operation_result(operation, self.runtime_policy_id(), output)
    }

    fn execute_resolved(
        &self,
        operation: BindingOperationKind,
        source: &[u8],
        uri: Option<&[u8]>,
    ) -> Result<BindingOperationResult, BindingError> {
        let output = self.execute_resolved_output(operation, source, uri)?;
        operation_result(operation, self.runtime_policy_id(), output)
    }

    fn execute_resolved_output(
        &self,
        operation: BindingOperationKind,
        source: &[u8],
        uri: Option<&[u8]>,
    ) -> Result<BindingOperationOutput, BindingError> {
        match operation.key() {
            OperationKey::Png => self.render_png_output(source),
            OperationKey::Jpeg => self.render_jpeg_output(source),
            OperationKey::Pdf => self.render_pdf_output(source),
            OperationKey::Svg => self.render_svg(source).map(BindingOperationOutput::plain),
            OperationKey::SvgPlanJson => self
                .svg_plan_json(source)
                .map(BindingOperationOutput::plain),
            OperationKey::Ascii => self.render_ascii(source).map(BindingOperationOutput::plain),
            OperationKey::SemanticJson => {
                self.parse_json(source).map(BindingOperationOutput::plain)
            }
            OperationKey::LayoutJson => self.layout_json(source).map(BindingOperationOutput::plain),
            OperationKey::AnalysisJson => {
                self.analyze_json(source).map(BindingOperationOutput::plain)
            }
            OperationKey::AnalysisFactsJson => self
                .analysis_facts_json(source)
                .map(BindingOperationOutput::plain),
            OperationKey::ValidationJson => self
                .validate_json(source)
                .map(BindingOperationOutput::plain),
            OperationKey::DocumentAnalysisJson => self
                .analyze_document_json(source, uri.expect("validated document URI presence"))
                .map(BindingOperationOutput::plain),
            OperationKey::DocumentAnalysisFactsJson => self
                .analyze_document_facts_json(source, uri.expect("validated document URI presence"))
                .map(BindingOperationOutput::plain),
        }
    }
}

fn operation_result(
    operation: BindingOperationKind,
    runtime_policy_id: &'static str,
    output: BindingOperationOutput,
) -> Result<BindingOperationResult, BindingError> {
    let BindingOperationOutput { data, output_plan } = output;
    let metadata_json = serde_json::to_vec(&BindingOperationMetadata {
        version: BINDING_OPERATION_SCHEMA_VERSION,
        operation_id: operation.operation_id(),
        media_type: operation.media_type(),
        runtime_policy: runtime_policy_id,
        byte_length: data.len(),
        output_plan: output_plan.as_ref(),
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
                options_json: br#"{"analysis":{"resources":{"limits":{"max_source_bytes":2048}}}}"#,
            })
            .unwrap();
        assert!(!result.data.is_empty());
    }

    #[test]
    fn reusable_semantic_output_is_stable_across_empty_version_and_real_overlays() {
        let engine =
            BindingEngine::from_options(br#"{"parse":{"suppress_errors":false},"version":2}"#)
                .unwrap();
        let execute = |options_json| {
            engine
                .execute(BindingOperationRequest {
                    operation_id: "semantic-json",
                    source: b"flowchart TD\nA --> B",
                    uri: None,
                    options_json,
                })
                .unwrap()
        };

        let empty = execute(b"");
        for unchanged in [
            br#"{}"#.as_slice(),
            br#"{"version":2}"#.as_slice(),
            b"{\n  \"version\": 2\n}".as_slice(),
        ] {
            let result = execute(unchanged);
            assert_eq!(result.data, empty.data);
            assert_eq!(result.metadata_json, empty.metadata_json);
        }
        let real = execute(br#"{"parse":{"suppress_errors":true}}"#);

        assert_eq!(real.data, empty.data);
        assert_eq!(real.metadata_json, empty.metadata_json);
    }

    #[test]
    fn reusable_engine_rejects_ambiguous_analysis_wrappers_at_construction() {
        let error = BindingEngine::from_options(
            br#"{
                "merman": { "fixed_today": "2025-01-01" },
                "analysis": {}
            }"#,
        )
        .err()
        .expect("ambiguous wrappers must fail before a reusable engine is created");

        assert_eq!(error.status(), BindingStatus::OptionsJsonError);
        assert!(
            error
                .message()
                .contains("must not contain both `analysis` and `merman` wrappers"),
            "unexpected error: {error:?}"
        );
    }

    #[test]
    fn unknown_operation_precedes_invalid_request_options() {
        let engine = BindingEngine::from_options(b"").unwrap();
        let error = engine
            .execute(BindingOperationRequest {
                operation_id: "unknown-operation",
                source: b"flowchart TD\nA --> B",
                uri: None,
                options_json: b"{",
            })
            .expect_err("operation resolution runs before request option parsing");

        assert_eq!(error.status(), BindingStatus::UnsupportedOperation);
        assert_eq!(error.kind(), crate::BindingErrorKind::UnknownOperation);
        assert!(error.message().contains("unknown operation"));
    }

    #[test]
    fn uri_presence_validation_precedes_invalid_request_options() {
        let engine = BindingEngine::from_options(b"").unwrap();

        let missing = engine
            .execute(BindingOperationRequest {
                operation_id: "document-analysis-json",
                source: b"flowchart TD\nA --> B",
                uri: None,
                options_json: b"{",
            })
            .expect_err("missing URI is rejected before malformed options");
        assert_eq!(missing.status(), BindingStatus::InvalidArgument);
        assert!(missing.message().contains("requires a document URI"));

        let unexpected = engine
            .execute(BindingOperationRequest {
                operation_id: "semantic-json",
                source: b"flowchart TD\nA --> B",
                uri: Some(b"file:///diagram.mmd"),
                options_json: b"{",
            })
            .expect_err("unexpected URI is rejected before malformed options");
        assert_eq!(unexpected.status(), BindingStatus::InvalidArgument);
        assert!(
            unexpected
                .message()
                .contains("does not accept a document URI")
        );
    }

    #[cfg(feature = "analysis")]
    #[test]
    fn invalid_request_options_precede_invalid_document_uri_bytes() {
        let engine = BindingEngine::from_options(b"").unwrap();
        let invalid_uri = [0xff];

        let options_error = engine
            .execute(BindingOperationRequest {
                operation_id: "document-analysis-json",
                source: b"flowchart TD\nA --> B",
                uri: Some(&invalid_uri),
                options_json: b"{",
            })
            .expect_err("options are parsed before URI bytes are decoded");
        assert_eq!(options_error.status(), BindingStatus::OptionsJsonError);

        let uri_error = engine
            .execute(BindingOperationRequest {
                operation_id: "document-analysis-json",
                source: b"flowchart TD\nA --> B",
                uri: Some(&invalid_uri),
                options_json: b"",
            })
            .expect_err("valid options allow execution to reach URI decoding");
        assert_eq!(uri_error.status(), BindingStatus::Utf8Error);
    }

    #[cfg(feature = "svg")]
    #[test]
    fn semantic_request_preserves_render_option_validation_before_source_errors() {
        let engine = BindingEngine::from_options(b"").unwrap();
        let invalid_source = [0xff];
        let error = engine
            .execute(BindingOperationRequest {
                operation_id: "semantic-json",
                source: &invalid_source,
                uri: None,
                options_json: br#"{"svg":{"pipeline":"invalid-pipeline"}}"#,
            })
            .expect_err("artifact-wide request validation checks the render domain first");

        assert_eq!(error.status(), BindingStatus::InvalidArgument);
        assert!(
            error
                .message()
                .contains("unsupported svg.pipeline: invalid-pipeline"),
            "unexpected error: {error:?}"
        );
    }

    #[cfg(all(feature = "analysis", feature = "svg"))]
    #[test]
    fn semantic_request_preserves_analysis_before_render_validation_order() {
        let engine = BindingEngine::from_options(b"").unwrap();
        let error = engine
            .execute(BindingOperationRequest {
                operation_id: "semantic-json",
                source: b"flowchart TD\nA --> B",
                uri: None,
                options_json: br#"{
                    "lint": { "profile": "invalid-profile" },
                    "svg": { "pipeline": "invalid-pipeline" }
                }"#,
            })
            .expect_err("artifact-wide request validation checks analysis before rendering");

        assert_eq!(error.status(), BindingStatus::InvalidArgument);
        assert!(
            error
                .message()
                .contains("lint.profile must be core, recommended, or strict"),
            "unexpected error: {error:?}"
        );
        assert!(!error.message().contains("svg.pipeline"));
    }

    #[cfg(not(feature = "png"))]
    #[test]
    fn invalid_request_options_precede_missing_operation_capability() {
        let engine = BindingEngine::from_options(b"").unwrap();
        let options_error = engine
            .execute(BindingOperationRequest {
                operation_id: "png",
                source: b"flowchart TD\nA --> B",
                uri: None,
                options_json: b"{",
            })
            .expect_err("request options are validated before operation execution");
        assert_eq!(options_error.status(), BindingStatus::OptionsJsonError);

        let capability_error = engine
            .execute(BindingOperationRequest {
                operation_id: "png",
                source: b"flowchart TD\nA --> B",
                uri: None,
                options_json: b"",
            })
            .expect_err("valid options reach the missing capability check");
        assert_eq!(
            capability_error.status(),
            BindingStatus::UnsupportedOperation
        );
        assert_eq!(
            capability_error.kind(),
            crate::BindingErrorKind::MissingCapability
        );
        assert_eq!(capability_error.capability_id(), Some("png"));
    }

    #[test]
    fn every_compiled_operation_shares_one_shot_and_reusable_result_contracts() {
        let engine = BindingEngine::from_options(b"").unwrap();

        for operation in BindingOperationKind::all().filter(|operation| operation.is_compiled()) {
            for options_json in [
                b"".as_slice(),
                br#"{"parse":{"suppress_errors":false}}"#.as_slice(),
            ] {
                let request = BindingOperationRequest {
                    operation_id: operation.operation_id(),
                    source: b"flowchart TD\nA --> B",
                    uri: operation
                        .requires_uri()
                        .then_some(b"file:///diagram.mmd".as_slice()),
                    options_json,
                };
                let one_shot = execute_once(request).unwrap_or_else(|error| {
                    panic!(
                        "one-shot operation `{}` failed: {}",
                        operation.operation_id(),
                        error.message()
                    )
                });
                let reusable = engine.execute(request).unwrap_or_else(|error| {
                    panic!(
                        "reusable operation `{}` failed: {}",
                        operation.operation_id(),
                        error.message()
                    )
                });

                assert_eq!(
                    one_shot,
                    reusable,
                    "operation={}, options={}",
                    operation.operation_id(),
                    String::from_utf8_lossy(options_json)
                );
                assert_eq!(one_shot.operation, operation);
                assert_eq!(one_shot.media_type, operation.media_type());
                let metadata: serde_json::Value =
                    serde_json::from_slice(&one_shot.metadata_json).unwrap();
                assert_eq!(
                    metadata["operation_id"],
                    operation.operation_id(),
                    "operation={}",
                    operation.operation_id()
                );
                assert_eq!(
                    metadata["media_type"],
                    operation.media_type(),
                    "operation={}",
                    operation.operation_id()
                );
                assert_eq!(
                    metadata["byte_length"],
                    one_shot.data.len(),
                    "operation={}",
                    operation.operation_id()
                );
            }
        }
    }

    #[cfg(feature = "svg")]
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

    #[cfg(feature = "ascii")]
    #[test]
    fn ascii_request_cannot_widen_the_constructor_grid_ceiling() {
        let engine =
            BindingEngine::from_options(br#"{"resources":{"limits":{"max_ascii_grid_cells":1}}}"#)
                .unwrap();

        let error = engine
            .execute(BindingOperationRequest {
                operation_id: "ascii",
                source: b"flowchart TD\nA --> B",
                uri: None,
                options_json: br#"{"resources":{"limits":{"max_ascii_grid_cells":2}}}"#,
            })
            .unwrap_err();

        assert_eq!(error.status(), BindingStatus::OptionsJsonError);
        assert!(error.message().contains("max_ascii_grid_cells"));
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

    #[cfg(all(feature = "png", feature = "jpeg", feature = "pdf"))]
    #[test]
    fn output_options_are_artifact_wide_at_construction_and_operation_scoped_per_request() {
        let engine = BindingEngine::from_options(
            br#"{"raster":{"scale":2},"jpeg":{"quality":85},"pdf":{"background":"white"}}"#,
        )
        .expect("constructor accepts the compiled artifact option union");
        engine
            .execute(BindingOperationRequest {
                operation_id: "semantic-json",
                source: b"flowchart TD\nA --> B",
                uri: None,
                options_json: b"",
            })
            .expect("unrelated constructor options do not affect semantic operations");

        let error = execute_once(BindingOperationRequest {
            operation_id: "semantic-json",
            source: b"flowchart TD\nA --> B",
            uri: None,
            options_json: br#"{"raster":{"scale":2}}"#,
        })
        .unwrap_err();
        assert_eq!(error.status(), BindingStatus::OptionsJsonError);
        assert!(error.message().contains("raster"));
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

    #[cfg(feature = "svg")]
    #[test]
    fn elk_layout_request_follows_the_resolved_dependency_feature_set() {
        let engine = BindingEngine::new(b"").unwrap();
        let result = engine.execute(BindingOperationRequest {
            operation_id: "svg",
            source: b"---\nconfig:\n  layout: elk\n---\nflowchart TD\n  A --> B\n",
            uri: None,
            options_json: b"",
        });

        if merman::svg::layout_elk_available() {
            assert_eq!(result.unwrap().media_type, "image/svg+xml");
        } else {
            let error = result.expect_err("ELK is not compiled");
            assert_eq!(error.status(), BindingStatus::UnsupportedOperation);
            assert_eq!(error.kind(), crate::BindingErrorKind::MissingCapability);
            assert_eq!(error.capability_id(), Some("layout-elk"));
            assert!(error.message().contains("`layout-elk`"));
        }
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
        let metadata: serde_json::Value = serde_json::from_slice(&result.metadata_json).unwrap();
        assert_eq!(metadata["output_plan"]["kind"], "raster");
        assert_eq!(metadata["output_plan"]["limited"], false);
        assert_eq!(metadata["output_plan"]["requested_scale"], 1.0);
        assert_eq!(metadata["output_plan"]["effective_scale"], 1.0);
    }

    #[cfg(feature = "png")]
    #[test]
    fn generic_png_operation_reports_resource_limited_effective_plan() {
        let result = execute_once(BindingOperationRequest {
            operation_id: "png",
            source: b"flowchart TD\nA --> B",
            uri: None,
            options_json: br#"{
                "version": 2,
                "raster": {"scale": 20},
                "resources": {"limits": {"max_raster_pixels": 4096}}
            }"#,
        })
        .unwrap();

        let metadata: serde_json::Value = serde_json::from_slice(&result.metadata_json).unwrap();
        let plan = &metadata["output_plan"];
        assert_eq!(plan["kind"], "raster");
        assert_eq!(plan["limited"], true);
        assert_eq!(plan["requested_scale"], 20.0);
        assert!(
            plan["effective_scale"].as_f64().unwrap() < plan["requested_scale"].as_f64().unwrap()
        );
        assert!(plan["width_px"].as_u64().unwrap() * plan["height_px"].as_u64().unwrap() <= 4096);
    }

    #[cfg(feature = "png")]
    #[test]
    fn reusable_png_request_overlay_reports_its_effective_plan_without_mutating_the_engine() {
        let engine = BindingEngine::from_options(
            br#"{"version":2,"resources":{"profile":"trusted-native"}}"#,
        )
        .unwrap();
        let limited = engine
            .execute(BindingOperationRequest {
                operation_id: "png",
                source: b"flowchart TD\nA --> B",
                uri: None,
                options_json: br#"{
                    "version": 2,
                    "raster": {"scale": 20},
                    "resources": {"limits": {"max_raster_pixels": 4096}}
                }"#,
            })
            .unwrap();
        let limited_metadata: serde_json::Value =
            serde_json::from_slice(&limited.metadata_json).unwrap();
        assert_eq!(limited_metadata["output_plan"]["limited"], true);
        assert!(
            limited_metadata["output_plan"]["width_px"]
                .as_u64()
                .unwrap()
                * limited_metadata["output_plan"]["height_px"]
                    .as_u64()
                    .unwrap()
                <= 4096
        );

        let baseline = engine
            .execute(BindingOperationRequest {
                operation_id: "png",
                source: b"flowchart TD\nA --> B",
                uri: None,
                options_json: b"",
            })
            .unwrap();
        let baseline_metadata: serde_json::Value =
            serde_json::from_slice(&baseline.metadata_json).unwrap();
        assert_eq!(baseline_metadata["output_plan"]["limited"], false);
        assert_eq!(baseline_metadata["output_plan"]["requested_scale"], 1.0);
    }

    #[cfg(feature = "pdf")]
    #[test]
    fn generic_pdf_operation_reports_effective_filter_plan() {
        let result = execute_once(BindingOperationRequest {
            operation_id: "pdf",
            source: b"flowchart TD\nA --> B",
            uri: None,
            options_json: br#"{"version":2,"pdf":{"filterScale":2.5}}"#,
        })
        .unwrap();

        let metadata: serde_json::Value = serde_json::from_slice(&result.metadata_json).unwrap();
        let plan = &metadata["output_plan"];
        assert_eq!(plan["kind"], "pdf-filter-images");
        assert_eq!(plan["requested_scale"], 2.5);
        assert_eq!(plan["effective_scale"], 2.5);
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
