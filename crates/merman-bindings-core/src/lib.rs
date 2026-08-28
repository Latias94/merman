#![forbid(unsafe_code)]

//! Safe shared facade used by external binding crates.
//!
//! This crate owns transport-neutral options parsing, semantic operations, renderer setup,
//! result-code classification, and byte payload production. Unsafe transport concerns such as raw
//! pointers and owned C buffers remain in `merman-ffi`.

mod artifact_contract;
mod capability;
mod catalog_contract;
mod common;
mod engine;
mod key_set;
mod lifecycle;
mod metadata;
mod metadata_registry;
mod operation;
mod operation_contract;
mod option_contract;
mod payload_contract;
mod resource_contract;
mod service_contract;
mod services;
mod svg_plan;
#[cfg(feature = "svg")]
mod text_measurement;
mod transport_contract;

#[cfg(feature = "ascii")]
mod ascii;
#[cfg(feature = "svg")]
mod render;

pub use artifact_contract::{ArtifactContractSpec, ValidatedArtifactContract};
pub use capability::{
    CAPABILITY_DESCRIPTOR_DIGEST, CAPABILITY_DESCRIPTOR_SCHEMA_VERSION, CapabilityDescriptor,
    CapabilityKey, OperationKey, OperationSpec, OutputDescriptor, OutputKey, TargetDescriptor,
    TargetKey, TransportCompiledExtensionKey,
};
pub use catalog_contract::{
    RUNTIME_CATALOG_FIELD_IDENTIFIER_PATTERN, RUNTIME_CATALOG_IDENTIFIER_PATTERN,
    RUNTIME_CATALOG_MAX_SAFE_INTEGER,
};
pub use common::{
    BINDING_OPTIONS_SCHEMA_VERSION, BINDING_RESULT_PAYLOAD_VERSION,
    BindingCancellationErrorDetails, BindingDiagnosticErrorDetails, BindingDiagnosticSpan,
    BindingError, BindingErrorKind, BindingIconRegistryErrorDetails, BindingResourceErrorDetails,
    BindingResourceLimitCause, BindingRuntimePolicy, BindingStatus, apply_resource_ceiling_json,
    binding_error_payload_json_bytes, error_payload_json_bytes, render_payload_json_bytes,
    render_resource_options_unavailable, resource_options_json,
};
#[doc(hidden)]
pub use common::{BindingJsSafeResourceErrorDetails, binding_error_js_payload_json_bytes};
pub use engine::BindingEngine;
pub use lifecycle::{
    BindingCallbackAdmission, BindingEngineAdmission, BindingEngineAdmissionError,
    BindingEngineAdmissionMode, BindingOperationAdmission,
};
pub use merman::{OperationControl, OperationPhase};
pub use metadata::{
    BindingAsciiCapability, BindingAsciiCapabilityEvidence, BindingDiagramFamilyCapability,
    PRESENTATION_CATALOG_SCHEMA_VERSION, RUNTIME_CATALOG_SCHEMA_VERSION, RuleCatalogEntry,
    RuntimeCapabilities, RuntimeCatalog, RuntimeConstructorResourceLimit,
    RuntimeConstructorServiceContract, RuntimeEmbeddedImageContract, RuntimeEmbeddedImageLimits,
    RuntimeOutputContract, RuntimePayloadSchema, RuntimeRegistryContract, RuntimeResourceContract,
    RuntimeResourceLimit, RuntimeResourceProfile, RuntimeSystemFontContract,
    TEXT_MEASUREMENT_PROVIDER_DETERMINISTIC, TEXT_MEASUREMENT_PROVIDER_HOST_CALLBACK,
    TextMeasurementCapabilities, ascii_capabilities, ascii_capabilities_json,
    ascii_diagrammatic_diagrams, ascii_supported_diagrams, ascii_supported_diagrams_json,
    configurable_lint_rule_catalog, configurable_lint_rule_catalog_json,
    diagram_family_capabilities, diagram_family_capabilities_json, lint_rule_catalog,
    lint_rule_catalog_json, runtime_constructor_resource_limits, supported_diagrams,
    supported_diagrams_json, supported_themes, supported_themes_json,
};
pub use metadata_registry::{MetadataKey, MetadataSpec};
pub use operation::{
    BindingAsciiOutputPlan, BindingOperationKind, BindingOperationMetadata,
    BindingOperationRequest, BindingOperationResult, BindingOutputPlan,
    BindingPdfFilterImageOutputPlan, BindingRasterOutputPlan, BindingUnknownOutputPlan,
    compiled_operation_kind_ids, execute_once,
};
pub use operation_contract::{
    BINDING_OPERATION_METADATA_CONTRACT_SCHEMA_VERSION, BindingJsonFieldContract,
    BindingOperationExpectation, BindingOperationMetadataContract, BindingOutputPlanContract,
    BindingUnavailableOperationExpectation, binding_operation_expectations,
    binding_operation_expectations_json, operation_metadata_contract,
    operation_metadata_contract_json,
};
pub use option_contract::{BindingOptionGroupKey, BindingOptionGroupSpec};
pub use payload_contract::{BINDING_OPERATION_SCHEMA_VERSION, BindingPayloadSchemaKey};
pub use resource_contract::{
    BindingResourceContract, BindingResourceLimitDescriptor, BindingResourceProfileDescriptor,
    binding_resource_contract,
};
pub use service_contract::{
    ConstructorServiceKey, RuntimePolicyExposure, TextMeasurementProviderKey,
    TextMeasurementProviderSource,
};
pub use services::BindingEngineServices;
#[cfg(feature = "svg")]
pub use services::{BindingIconRegistry, build_icon_registry};
pub use svg_plan::{
    SVG_PLAN_SCHEMA_VERSION, SvgPlanPayload, SvgPlanPresentationAspect, svg_plan_json,
};

/// Parses Mermaid into the canonical semantic JSON model without requiring any render backend.
pub fn parse_json(source: &[u8], options_json: &[u8]) -> Result<Vec<u8>, BindingError> {
    execute_once_data("semantic-json", source, None, options_json)
}

#[cfg(feature = "analysis")]
pub use merman_analysis::{ANALYSIS_FACTS_PAYLOAD_VERSION, ANALYSIS_PAYLOAD_VERSION};

#[cfg(feature = "ascii")]
pub use ascii::render_ascii;
#[cfg(feature = "svg")]
pub use merman::svg::{
    HostMeasurementResult, HostTextMeasurement, HostTextMeasurementError,
    HostTextMeasurementRequest, HostTextMeasurer, IconPack, IconRegistryResourceLimitDescriptor,
    IconRegistryResourceLimitId, TEXT_MEASUREMENT_PROTOCOL_VERSION, TextMeasurementOperation,
    TextMeasurementPhase, TextMetrics, TextStyle, WrapMode,
    icon_registry_resource_limit_descriptors, validate_host_text_measurement,
};
#[cfg(feature = "jpeg")]
pub use render::render_jpeg;
#[cfg(feature = "pdf")]
pub use render::render_pdf;
#[cfg(feature = "png")]
pub use render::render_png;
#[cfg(feature = "svg")]
pub use render::{layout_json, render_svg};
#[cfg(feature = "svg")]
pub use text_measurement::{
    HostTextMeasurementRecord, HostTextMeasurementResultKind, HostTextMeasurementTransportFields,
    decode_host_text_measurement, host_text_measurement_transport_fields,
};
pub use transport_contract::{BindingTransportExposureSpec, BindingTransportKey};

#[cfg(not(feature = "ascii"))]
pub fn render_ascii(source: &[u8], options_json: &[u8]) -> Result<Vec<u8>, BindingError> {
    execute_once_data("ascii", source, None, options_json)
}

#[cfg(feature = "ascii")]
pub fn render_ascii_result(
    source: &[u8],
    options_json: &[u8],
) -> Result<BindingOperationResult, BindingError> {
    execute_once(BindingOperationRequest::new("ascii", source).with_options_json(options_json))
}

pub fn analyze_json(source: &[u8], options_json: &[u8]) -> Result<Vec<u8>, BindingError> {
    execute_once_data("analysis-json", source, None, options_json)
}

pub fn analysis_facts_json(source: &[u8], options_json: &[u8]) -> Result<Vec<u8>, BindingError> {
    execute_once_data("analysis-facts-json", source, None, options_json)
}

pub fn analyze_document_json(
    source: &[u8],
    uri: &[u8],
    options_json: &[u8],
) -> Result<Vec<u8>, BindingError> {
    execute_once_data("document-analysis-json", source, Some(uri), options_json)
}

pub fn analyze_document_facts_json(
    source: &[u8],
    uri: &[u8],
    options_json: &[u8],
) -> Result<Vec<u8>, BindingError> {
    execute_once_data(
        "document-analysis-facts-json",
        source,
        Some(uri),
        options_json,
    )
}

pub fn validate_json(source: &[u8], options_json: &[u8]) -> Result<Vec<u8>, BindingError> {
    execute_once_data("validation-json", source, None, options_json)
}

#[cfg(not(feature = "svg"))]
pub fn render_svg(source: &[u8], options_json: &[u8]) -> Result<Vec<u8>, BindingError> {
    execute_once_data("svg", source, None, options_json)
}

#[cfg(not(feature = "svg"))]
pub fn layout_json(source: &[u8], options_json: &[u8]) -> Result<Vec<u8>, BindingError> {
    execute_once_data("layout-json", source, None, options_json)
}

#[cfg(not(feature = "png"))]
pub fn render_png(source: &[u8], options_json: &[u8]) -> Result<Vec<u8>, BindingError> {
    execute_once_data("png", source, None, options_json)
}

#[cfg(not(feature = "jpeg"))]
pub fn render_jpeg(source: &[u8], options_json: &[u8]) -> Result<Vec<u8>, BindingError> {
    execute_once_data("jpeg", source, None, options_json)
}

#[cfg(not(feature = "pdf"))]
pub fn render_pdf(source: &[u8], options_json: &[u8]) -> Result<Vec<u8>, BindingError> {
    execute_once_data("pdf", source, None, options_json)
}

pub fn render_png_result(
    source: &[u8],
    options_json: &[u8],
) -> Result<BindingOperationResult, BindingError> {
    execute_once(BindingOperationRequest::new("png", source).with_options_json(options_json))
}

pub fn render_jpeg_result(
    source: &[u8],
    options_json: &[u8],
) -> Result<BindingOperationResult, BindingError> {
    execute_once(BindingOperationRequest::new("jpeg", source).with_options_json(options_json))
}

pub fn render_pdf_result(
    source: &[u8],
    options_json: &[u8],
) -> Result<BindingOperationResult, BindingError> {
    execute_once(BindingOperationRequest::new("pdf", source).with_options_json(options_json))
}

fn execute_once_data(
    operation_id: &str,
    source: &[u8],
    uri: Option<&[u8]>,
    options_json: &[u8],
) -> Result<Vec<u8>, BindingError> {
    operation::execute_once_data(
        BindingOperationRequest::new(operation_id, source)
            .with_optional_uri(uri)
            .with_options_json(options_json),
    )
}

#[cfg(all(
    test,
    any(
        not(feature = "svg"),
        not(feature = "ascii"),
        not(feature = "analysis")
    )
))]
mod tests {
    use super::*;
    #[cfg(feature = "analysis")]
    use serde_json::Value;

    fn assert_missing_capability(error: &BindingError, capability_id: &str) {
        assert_eq!(error.status(), BindingStatus::UnsupportedOperation);
        assert_eq!(error.kind(), BindingErrorKind::MissingCapability);
        assert_eq!(error.capability_id(), Some(capability_id));
    }

    #[cfg(not(feature = "svg"))]
    #[test]
    fn render_and_layout_entry_points_report_missing_svg_capability() {
        let err = render_svg(b"flowchart TD\nA", b"").unwrap_err();
        assert_missing_capability(&err, "svg");

        assert!(!parse_json(b"flowchart TD\nA", b"").unwrap().is_empty());

        let err = layout_json(b"flowchart TD\nA", b"").unwrap_err();
        assert_missing_capability(&err, "svg");

        let err = render_png_result(b"flowchart TD\nA", b"").unwrap_err();
        assert_missing_capability(&err, "png");
    }

    #[cfg(not(feature = "ascii"))]
    #[test]
    fn ascii_entry_point_reports_missing_ascii_feature() {
        let err = render_ascii(b"flowchart TD\nA", b"").unwrap_err();
        assert_missing_capability(&err, "ascii");
    }

    #[cfg(not(feature = "analysis"))]
    #[test]
    fn analysis_entry_points_report_missing_analysis_feature() {
        let err = analyze_json(b"flowchart TD\nA", b"").unwrap_err();
        assert_missing_capability(&err, "analysis");

        let err = analysis_facts_json(b"flowchart TD\nA", b"").unwrap_err();
        assert_missing_capability(&err, "analysis");

        let err =
            analyze_document_json(b"flowchart TD\nA", b"file:///tmp/example.mmd", b"").unwrap_err();
        assert_missing_capability(&err, "analysis");

        let err = analyze_document_facts_json(b"flowchart TD\nA", b"file:///tmp/example.mmd", b"")
            .unwrap_err();
        assert_missing_capability(&err, "analysis");

        let err = validate_json(b"flowchart TD\nA", b"").unwrap_err();
        assert_missing_capability(&err, "analysis");
    }

    #[cfg(feature = "analysis")]
    #[test]
    fn analyze_json_reports_payload_for_empty_source() {
        let json: Value = serde_json::from_slice(&analyze_json(b"", b"").unwrap()).unwrap();
        assert_eq!(json["version"], ANALYSIS_PAYLOAD_VERSION);
        assert_eq!(json["valid"], false);
        assert_eq!(json["diagnostics"][0]["code_name"], "MERMAN_NO_DIAGRAM");
    }

    #[cfg(feature = "analysis")]
    #[test]
    fn analysis_facts_json_reports_parser_backed_syntax_facts() {
        let json: Value =
            serde_json::from_slice(&analysis_facts_json(b"flowchart TD\nA-->B\n", b"").unwrap())
                .unwrap();

        assert_eq!(json["version"], ANALYSIS_FACTS_PAYLOAD_VERSION);
        assert_eq!(json["valid"], true);
        assert_eq!(json["diagrams"][0]["kind"], "whole_document");
        assert_eq!(
            json["diagrams"][0]["syntax"]["diagram_type"],
            "flowchart-v2"
        );
        assert_eq!(
            json["diagrams"][0]["syntax"]["fact_source"],
            "parser_complete"
        );
        assert_eq!(json["diagrams"][0]["syntax"]["source_mapped_spans"], true);
        assert!(
            json["diagrams"][0]["syntax"]["node_ids"]
                .as_array()
                .unwrap()
                .iter()
                .any(|id| id == "A")
        );
        assert!(
            json["diagrams"][0]["syntax"]["semantic_items"]
                .as_array()
                .unwrap()
                .iter()
                .any(|item| {
                    item["name"] == "A" && item["rename_policy"] == "flowchart_node_id"
                })
        );
    }

    #[cfg(feature = "analysis")]
    #[test]
    fn analyze_document_json_reports_markdown_source_and_host_ranges() {
        let source = b"before\n```mermaid\nflowchart TD\nA-->\n```\nafter\n";
        let json: Value = serde_json::from_slice(
            &analyze_document_json(source, b"file:///tmp/example.md", b"").unwrap(),
        )
        .unwrap();

        assert_eq!(json["valid"], false);
        assert_eq!(json["source"]["kind"], "markdown");
        assert_eq!(json["source"]["path"], "file:///tmp/example.md");
        assert_eq!(json["diagnostics"][0]["span"]["line"], 4);
        assert!(
            json["diagnostics"][0]["related"]
                .as_array()
                .unwrap()
                .iter()
                .any(|related| related["message"] == "Mermaid fence 1")
        );
    }

    #[cfg(feature = "analysis")]
    #[test]
    fn analyze_document_json_reports_mdx_source_with_uri_fragment() {
        let source = b"before\n```mermaid\nflowchart TD\nA-->\n```\nafter\n";
        let json: Value = serde_json::from_slice(
            &analyze_document_json(source, b"file:///tmp/example.mdx?rev=1#fence", b"").unwrap(),
        )
        .unwrap();

        assert_eq!(json["valid"], false);
        assert_eq!(json["source"]["kind"], "mdx");
        assert_eq!(json["source"]["language"], "mdx");
        assert_eq!(
            json["source"]["path"],
            "file:///tmp/example.mdx?rev=1#fence"
        );
        assert_eq!(json["diagnostics"][0]["span"]["line"], 4);
    }

    #[cfg(feature = "analysis")]
    #[test]
    fn analyze_document_json_enforces_the_host_document_diagram_limit() {
        let source = concat!(
            "```mermaid\nflowchart TD\nA-->B\n```\n",
            "```mermaid\nsequenceDiagram\nA->>B: hi\n```\n",
        );
        let options = br#"{
            "resources": {
                "limits": { "max_document_diagrams": 1 }
            }
        }"#;
        let json: Value = serde_json::from_slice(
            &analyze_document_json(source.as_bytes(), b"file:///tmp/limited.md", options).unwrap(),
        )
        .unwrap();

        assert_eq!(json["valid"], false);
        assert_eq!(
            json["diagnostics"][0]["id"],
            "merman.resource.document_diagrams_exceeded"
        );
        assert_eq!(json["diagnostics"][0]["span"]["line"], 5);
    }

    #[cfg(feature = "analysis")]
    #[test]
    fn analyze_document_facts_json_reports_markdown_fence_facts_with_host_ranges() {
        let source = b"before\n```mermaid\nflowchart TD\nA@{\n  shape: rou\n}\n```\nafter\n";
        let json: Value = serde_json::from_slice(
            &analyze_document_facts_json(source, b"file:///tmp/example.md", b"").unwrap(),
        )
        .unwrap();

        assert_eq!(json["version"], ANALYSIS_FACTS_PAYLOAD_VERSION);
        assert_eq!(json["source"]["kind"], "markdown");
        assert_eq!(json["diagrams"][0]["source_id"], "mermaid-fence-1");
        assert_eq!(json["diagrams"][0]["kind"], "mermaid_fence");
        assert_eq!(json["diagrams"][0]["syntax"]["parser_backed"], true);
        assert!(
            json["diagrams"][0]["syntax"]["expected_syntax"]
                .as_array()
                .unwrap()
                .iter()
                .any(|expected| {
                    expected["kind"] == "shape" && expected["span"]["document"].is_object()
                })
        );
    }

    #[cfg(feature = "analysis")]
    #[test]
    fn validate_json_reports_legacy_projection_for_empty_source() {
        let json: Value = serde_json::from_slice(&validate_json(b"", b"").unwrap()).unwrap();
        assert_eq!(json["valid"], false);
        assert_eq!(json["code_name"], BindingStatus::NoDiagram.code_name());
        assert_eq!(json["error"], "no Mermaid diagram detected");
    }

    #[cfg(feature = "analysis")]
    #[test]
    fn validate_json_reports_non_ok_status_for_lint_errors_without_public_codes() {
        let json: Value = serde_json::from_slice(
            &validate_json(
                b"gitGraph\ncommit id:\"working on MDR\"\ncommit id:\"working on MDR\"\n",
                br#"{"lint":{"rule_severities":[{"rule_id":"merman.git_graph.duplicate_commit_id","severity":"error"}]}}"#,
            )
            .unwrap(),
        )
        .unwrap();

        assert_eq!(json["valid"], false);
        assert_eq!(json["code_name"], BindingStatus::ParseError.code_name());
        assert_eq!(json["code"], BindingStatus::ParseError.code());
        assert_ne!(json["code_name"], BindingStatus::Ok.code_name());
        assert!(
            json["error"]
                .as_str()
                .is_some_and(|message| message.contains("already exists")),
            "unexpected validation payload: {json}"
        );
    }

    #[cfg(feature = "analysis")]
    #[test]
    fn analyze_json_honors_lint_rule_configuration() {
        let payload: Value = serde_json::from_slice(
            &analyze_json(
                b"gitGraph\ncommit id:\"working on MDR\"\ncommit id:\"working on MDR\"\n",
                br#"{"lint":{"disable_rules":["merman.git_graph.duplicate_commit_id"]}}"#,
            )
            .unwrap(),
        )
        .unwrap();

        assert_eq!(payload["valid"], true);
        assert!(payload["diagnostics"].as_array().unwrap().is_empty());
    }

    #[cfg(feature = "analysis")]
    #[test]
    fn analyze_json_honors_lint_severity_overrides() {
        let payload: Value = serde_json::from_slice(
            &analyze_json(
                b"gitGraph\ncommit id:\"working on MDR\"\ncommit id:\"working on MDR\"\n",
                br#"{"lint":{"rule_severities":[{"rule_id":"merman.git_graph.duplicate_commit_id","severity":"hint"}]}}"#,
            )
            .unwrap(),
        )
        .unwrap();

        assert_eq!(payload["valid"], true);
        assert_eq!(payload["summary"]["hints"], 1);
        assert_eq!(
            payload["diagnostics"][0]["id"].as_str(),
            Some("merman.git_graph.duplicate_commit_id")
        );
        assert_eq!(payload["diagnostics"][0]["severity"].as_str(), Some("hint"));
    }

    #[cfg(feature = "analysis")]
    #[test]
    fn analyze_json_rejects_unknown_lint_rule_ids() {
        let err = analyze_json(
            b"flowchart TD\nA-->B\n",
            br#"{"lint":{"disable_rules":["merman.unknown.rule"]}}"#,
        )
        .unwrap_err();

        assert_eq!(err.status(), BindingStatus::InvalidArgument);
        assert!(
            err.message().contains("configurable analysis rule id"),
            "unexpected error: {err:?}"
        );
    }
}
