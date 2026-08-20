#[cfg(feature = "svg")]
use std::sync::Arc;
use std::time::Duration;

use merman::{
    OperationControl, OperationPhase, ParseOptions, RenderError, RenderOutput, RenderRequest,
    Renderer, SemanticArtifact,
    resources::{InputResourceLimitId, InputResourcePolicy},
};

#[cfg(feature = "ascii")]
use merman::{
    AsciiRequest, OperationResourceDomain, OperationResourceLimitExceeded,
    OperationResourceOverride, OperationResourceProvenance, RenderTarget,
    ascii::{AsciiResourceLimitId, AsciiResourcePolicy},
};

#[cfg(feature = "svg")]
#[derive(Debug)]
struct CancellingTextMeasurer {
    control: OperationControl,
}

#[cfg(feature = "svg")]
impl merman::svg::HostTextMeasurer for CancellingTextMeasurer {
    fn measure(
        &self,
        _request: merman::svg::HostTextMeasurementRequest<'_>,
    ) -> merman::svg::HostMeasurementResult {
        self.control.cancel();
        Ok(None)
    }
}

#[cfg(feature = "svg")]
#[derive(Debug)]
struct EvidenceMathRenderer;

#[cfg(feature = "svg")]
impl merman::svg::MathRenderer for EvidenceMathRenderer {
    fn render_html_label(&self, _text: &str, _config: &merman::MermaidConfig) -> Option<String> {
        Some("<span>rendered math</span>".to_string())
    }

    fn measure_sequence_html_label(
        &self,
        _text: &str,
        _config: &merman::MermaidConfig,
    ) -> Option<merman::svg::TextMetrics> {
        Some(merman::svg::TextMetrics {
            width: 80.0,
            height: 24.0,
            line_count: 1,
        })
    }
}

#[test]
fn semantic_request_uses_the_canonical_operation_runner() {
    let output = Renderer::new()
        .render(RenderRequest::semantic(
            "flowchart TD\nA[Start] --> B[Done]",
            OperationControl::new(),
        ))
        .expect("semantic request should succeed");

    let RenderOutput::Semantic(Some(artifact)) = output else {
        panic!("expected a semantic artifact");
    };
    assert_eq!(artifact.diagram_type(), "flowchart-v2");
    assert_eq!(artifact.semantic_kind(), "flowchart");
}

#[test]
fn cancelled_semantic_request_returns_no_partial_artifact() {
    let control = OperationControl::new();
    control.cancel();

    let error = Renderer::new()
        .render(RenderRequest::semantic("flowchart TD\nA --> B", control))
        .expect_err("cancelled operation must not return an artifact");

    assert!(matches!(
        error,
        RenderError::Cancelled(cancelled)
            if cancelled.phase == OperationPhase::Admission
                && cancelled.reason == merman::CancelReason::Requested
    ));
}

#[test]
fn expired_deadline_is_reported_as_deadline_cancellation() {
    let control = OperationControl::new().with_deadline(Duration::ZERO);

    let error = Renderer::new()
        .render(RenderRequest::semantic("flowchart TD\nA --> B", control))
        .expect_err("expired operation must stop before parsing");

    assert!(matches!(
        error,
        RenderError::Cancelled(cancelled)
            if cancelled.reason == merman::CancelReason::DeadlineExceeded
    ));
}

#[test]
fn prepare_semantic_delegates_to_the_same_runner() {
    let artifact: Option<SemanticArtifact> = Renderer::new()
        .prepare_semantic("info", OperationControl::new())
        .expect("prepare should succeed");
    assert_eq!(
        artifact.expect("info should be detected").semantic_kind(),
        "info"
    );
}

#[test]
fn renderer_defaults_apply_when_the_request_does_not_override_them() {
    let renderer = Renderer::new().with_resource_policy(
        InputResourcePolicy::default()
            .with_limit(InputResourceLimitId::MaxSourceBytes, 4)
            .expect("valid source limit"),
    );

    let error = renderer
        .render(RenderRequest::semantic(
            "flowchart TD\nA --> B",
            OperationControl::new(),
        ))
        .expect_err("the renderer's source limit must apply to one-shot requests");

    assert!(matches!(
        error,
        RenderError::ResourceLimitExceeded(limit)
            if limit.id == "max_source_bytes" && limit.maximum == 4
    ));
}

#[test]
fn source_resource_terminal_replays_before_later_cancellation() {
    let renderer = Renderer::new().with_resource_policy(
        InputResourcePolicy::default()
            .with_limit(InputResourceLimitId::MaxSourceBytes, 4)
            .expect("valid source limit"),
    );
    let control = OperationControl::new();

    let first = renderer
        .render(RenderRequest::semantic(
            "flowchart TD\nA --> B",
            control.clone(),
        ))
        .expect_err("the source limit must reject the request");
    assert!(matches!(
        &first,
        RenderError::ResourceLimitExceeded(limit)
            if limit.id == "max_source_bytes"
                && limit.phase == "source"
                && limit.actual == 20
                && limit.maximum == 4
    ));

    control.cancel();
    let replayed = renderer
        .render(RenderRequest::semantic("info", control))
        .expect_err("the first source terminal must remain sticky");
    assert!(matches!(
        replayed,
        RenderError::ResourceLimitExceeded(limit)
            if limit.id == "max_source_bytes"
                && limit.phase == "source"
                && limit.actual == 20
                && limit.maximum == 4
    ));
}

#[test]
fn request_overrides_take_precedence_over_renderer_defaults() {
    let renderer = Renderer::new()
        .with_parse_options(ParseOptions::strict())
        .with_resource_policy(
            InputResourcePolicy::default()
                .with_limit(InputResourceLimitId::MaxSourceBytes, 4)
                .expect("valid source limit"),
        );
    let request_resources = InputResourcePolicy::default()
        .with_limit(InputResourceLimitId::MaxSourceBytes, 4_096)
        .expect("valid source limit");

    let output = renderer
        .render(
            RenderRequest::semantic("flowchart TD\nA --> B", OperationControl::new())
                .with_parse_options(ParseOptions::lenient())
                .with_resource_policy(request_resources),
        )
        .expect("request overrides should replace renderer defaults");

    assert!(matches!(output, RenderOutput::Semantic(Some(_))));
}

#[cfg(feature = "svg")]
#[test]
fn typed_svg_targets_share_the_prepared_operation() {
    let renderer = Renderer::new();
    let source = "flowchart TD\nA[Start] --> B[Done]";

    let layout = renderer
        .render(RenderRequest::layout_json(
            source,
            OperationControl::new(),
            merman::SvgRequest::default(),
        ))
        .expect("layout target should succeed");
    let RenderOutput::LayoutJson(Some(layout)) = layout else {
        panic!("expected typed layout JSON");
    };
    assert!(layout.layout().get("layout").is_some());

    let plan = renderer
        .render(RenderRequest::svg_plan(
            source,
            OperationControl::new(),
            merman::SvgRequest::default(),
        ))
        .expect("SVG plan target should succeed");
    let RenderOutput::SvgPlan(Some(plan)) = plan else {
        panic!("expected typed SVG capability plan");
    };
    assert!(plan.is_ready());
}

#[cfg(feature = "svg")]
#[test]
fn svg_evidence_carries_preparation_owned_capability_requirements() {
    let request = merman::SvgRequest {
        environment: merman::SvgEnvironment::deterministic()
            .without_math_renderer()
            .with_math_renderer(Arc::new(EvidenceMathRenderer)),
        ..Default::default()
    };
    let output = Renderer::new()
        .render(RenderRequest::svg(
            "sequenceDiagram\nA->>B: $$x$$",
            OperationControl::new(),
            request,
        ))
        .expect("math SVG render should succeed");
    let RenderOutput::Svg(Some(output)) = output else {
        panic!("expected typed SVG output");
    };

    assert_eq!(
        output.evidence().required_capabilities(),
        &[merman::svg::RenderCapability::Math]
    );
}

#[cfg(feature = "svg")]
#[test]
fn semantic_artifact_exposes_compatibility_json_without_family_types() {
    let artifact = Renderer::new()
        .prepare_semantic("flowchart TD\nA --> B", OperationControl::new())
        .expect("parse should succeed")
        .expect("diagram should be detected");
    let json = artifact
        .compatibility_json()
        .expect("compatibility JSON should be projected");
    assert_eq!(json["type"], "flowchart-v2");
}

#[test]
fn compatibility_json_observes_the_artifact_operation_control() {
    let control = OperationControl::new();
    let artifact = Renderer::new()
        .prepare_semantic("flowchart TD\nA --> B", control.clone())
        .expect("parse should succeed")
        .expect("diagram should be detected");
    control.cancel();

    let error = artifact
        .compatibility_json()
        .expect_err("semantic projection must observe cancellation");
    assert!(matches!(
        error,
        RenderError::Cancelled(cancelled)
            if cancelled.phase == OperationPhase::Semantic
    ));
}

#[cfg(feature = "svg")]
#[test]
fn layout_json_observes_cancellation_after_layout_preparation() {
    let control = OperationControl::new();
    let identity = merman::svg::TextMeasurementProfileIdentity::new(
        merman::svg::MeasurementProfileId::new("merman.test-cancelling-host")
            .expect("static profile id"),
        "render-operation-test@1",
    )
    .expect("static profile identity");
    let policy = merman::svg::TextMeasurementPolicy::host_display(
        identity,
        Arc::new(CancellingTextMeasurer {
            control: control.clone(),
        }),
        merman::svg::TextMeasurementPhase::ALL,
    );
    let request = merman::SvgRequest {
        environment: merman::SvgEnvironment::deterministic().with_text_measurement_policy(policy),
        ..Default::default()
    };

    let error = Renderer::new()
        .render(RenderRequest::layout_json(
            "flowchart TD\nA[Start] --> B[Done]",
            control,
            request,
        ))
        .expect_err("layout projection must not return after its control terminates");
    assert!(matches!(
        error,
        RenderError::Cancelled(cancelled)
            if cancelled.phase == OperationPhase::Layout
                && cancelled.reason == merman::CancelReason::Requested
    ));
}

#[cfg(feature = "svg")]
#[test]
fn svg_request_cancellation_is_not_reported_as_a_resource_limit() {
    let control = OperationControl::new();
    control.cancel();
    let error = Renderer::new()
        .render(RenderRequest::svg(
            "flowchart TD\nA --> B",
            control,
            merman::SvgRequest::default(),
        ))
        .expect_err("cancelled SVG request must stop");
    assert!(matches!(error, RenderError::Cancelled(_)));
}

#[cfg(feature = "svg")]
#[test]
fn svg_backend_errors_use_canonical_render_error_classification() {
    let cancelled = merman::OperationCancelled {
        phase: OperationPhase::Emit,
        reason: merman::CancelReason::Requested,
    };
    let error = RenderError::from(merman::svg::RenderError::Cancelled(cancelled));
    assert!(matches!(error, RenderError::Cancelled(actual) if actual == cancelled));

    let policy = merman::svg::RenderResourcePolicy::interactive()
        .with_limit(merman::svg::ResourceLimitId::MaxSvgBytes, 1)
        .expect("max SVG bytes is overridable");
    let resource = policy
        .check_svg_bytes("<svg/>", merman::svg::ResourceLimitPhase::SvgOutput)
        .expect_err("the backend policy should reject the oversized SVG");
    let error = RenderError::from(merman::svg::RenderError::ResourceLimitExceeded(resource));
    assert!(matches!(
        error,
        RenderError::ResourceLimitExceeded(limit)
            if limit.id == "max_svg_bytes"
                && limit.phase == "svg_output"
                && limit.actual == 6
                && limit.maximum == 1
                && limit.cause == merman::ResourceLimitCause::Ceiling
    ));

    let error = RenderError::from(merman::svg::RenderError::SvgPostprocess {
        pass: "test".to_string(),
        message: "backend failure".to_string(),
    });
    assert!(matches!(
        error,
        RenderError::Svg(merman::svg::RenderError::SvgPostprocess { .. })
    ));
}

#[cfg(any(feature = "png", feature = "jpeg", feature = "pdf"))]
#[test]
fn export_backend_errors_use_canonical_render_error_classification() {
    let cancelled = merman::OperationCancelled {
        phase: OperationPhase::Export,
        reason: merman::CancelReason::Requested,
    };
    let error = RenderError::from(merman::svg::export::ExportError::Cancelled(cancelled));
    assert!(matches!(error, RenderError::Cancelled(actual) if actual == cancelled));

    let error = RenderError::from(merman::svg::export::ExportError::EmbeddedImageLimit {
        limit_name: "max_bytes_per_image",
        actual: 2,
        max: 1,
    });
    assert!(matches!(
        error,
        RenderError::ResourceLimitExceeded(limit)
            if limit.id
                == merman::svg::export::MAX_EMBEDDED_IMAGE_BYTES_RESOURCE_LIMIT_ID
                && limit.phase == "embedded_image_decode"
                && limit.actual == 2
                && limit.maximum == 1
                && limit.cause == merman::ResourceLimitCause::Ceiling
                && limit.provenance.as_ref().is_some_and(|provenance| {
                    provenance.domain == merman::OperationResourceDomain::Export
                        && provenance.profile.is_none()
                        && provenance.explicit_overrides.is_empty()
                })
    ));

    let error = RenderError::from(merman::svg::export::ExportError::SvgParse);
    assert!(matches!(
        error,
        RenderError::Export(merman::svg::export::ExportError::SvgParse)
    ));

    let error = RenderError::from(merman::svg::export::ExportError::SvgConversionLimit {
        limit_name: "svg_backend_tree_depth",
        actual: 2,
        max: 1,
    });
    assert!(matches!(
        error,
        RenderError::ResourceLimitExceeded(limit)
            if limit.id == "svg_backend_tree_depth"
                && limit.phase == "svg_postprocess"
                && limit.actual == 2
                && limit.maximum == 1
                && limit.provenance.as_ref().is_some_and(|provenance| {
                    provenance.domain == merman::OperationResourceDomain::Render
                        && provenance.profile.is_none()
                        && provenance.explicit_overrides.is_empty()
                })
    ));

    let error = RenderError::from(
        merman::svg::export::ExportError::ResourceArithmeticOverflow {
            limit_id: "max_export_bytes",
            phase: "export",
            actual: u64::MAX,
            max: 1024,
        },
    );
    assert!(matches!(
        error,
        RenderError::ResourceLimitExceeded(limit)
            if limit.id == "max_export_bytes"
                && limit.phase == "export"
                && limit.actual == u64::MAX
                && limit.maximum == 1024
                && limit.cause == merman::ResourceLimitCause::ArithmeticOverflow
                && limit.provenance.as_ref().is_some_and(|provenance| {
                    provenance.domain == merman::OperationResourceDomain::Export
                        && provenance.profile.is_none()
                        && provenance.explicit_overrides.is_empty()
                })
    ));

    let provenance = merman::OperationResourceProvenance::new(
        merman::OperationResourceDomain::Render,
        Some(merman::resources::ResourceProfile::Constrained),
        [merman::OperationResourceOverride {
            id: "max_svg_bytes",
            value: 17,
        }],
    );
    let terminal = merman::OperationLedgerError::ArithmeticOverflow {
        id: "max_svg_bytes",
        phase: OperationPhase::Postprocess,
        resource_phase: "svg_postprocess",
        actual: u64::MAX,
        maximum: 17,
        provenance: provenance.clone(),
    };
    let error = RenderError::from(merman::svg::export::ExportError::OperationResourceTerminal(
        terminal,
    ));
    assert!(matches!(
        error,
        RenderError::ResourceLimitExceeded(limit)
            if limit.id == "max_svg_bytes"
                && limit.phase == "svg_postprocess"
                && limit.actual == u64::MAX
                && limit.maximum == 17
                && limit.cause == merman::ResourceLimitCause::ArithmeticOverflow
                && limit.provenance == Some(provenance)
    ));
}

#[cfg(feature = "ascii")]
#[test]
fn ascii_backend_errors_use_canonical_render_error_classification() {
    let cancelled = merman::OperationCancelled {
        phase: OperationPhase::Layout,
        reason: merman::CancelReason::Requested,
    };
    let error = RenderError::from(merman::ascii::AsciiError::Cancelled(cancelled));
    assert!(matches!(error, RenderError::Cancelled(actual) if actual == cancelled));

    let error = RenderError::from(merman::ascii::AsciiError::UnsupportedDiagram {
        diagram_type: "test".to_string(),
    });
    assert!(matches!(
        error,
        RenderError::Ascii(merman::ascii::AsciiError::UnsupportedDiagram { .. })
    ));
}

#[cfg(feature = "ascii")]
#[test]
fn ascii_request_honors_target_local_grid_policy_at_exact_and_minus_one() {
    const EXACT_GRID_CELLS: usize = 75;
    let source = "flowchart TD\nA --> B";
    let exact_resources = AsciiResourcePolicy::default()
        .with_limit(AsciiResourceLimitId::MaxGridCells, EXACT_GRID_CELLS)
        .expect("the exact grid limit must be valid");

    let output = Renderer::new()
        .render(RenderRequest::ascii(
            source,
            OperationControl::new(),
            AsciiRequest {
                resources: exact_resources,
                ..Default::default()
            },
        ))
        .expect("the exact target-local grid policy must admit the request");
    assert!(matches!(output, RenderOutput::Ascii(Some(_))));

    let below_resources = AsciiResourcePolicy::default()
        .with_limit(AsciiResourceLimitId::MaxGridCells, EXACT_GRID_CELLS - 1)
        .expect("the max-minus-one grid limit must be valid");
    let error = Renderer::new()
        .render(RenderRequest::ascii(
            source,
            OperationControl::new(),
            AsciiRequest {
                resources: below_resources,
                ..Default::default()
            },
        ))
        .expect_err("the max-minus-one target-local grid policy must reject the request");
    assert!(matches!(
        error,
        RenderError::ResourceLimitExceeded(limit)
            if limit.id == "max_ascii_grid_cells"
                && limit.phase == "ascii_layout"
                && limit.actual == EXACT_GRID_CELLS as u64
                && limit.maximum == (EXACT_GRID_CELLS - 1) as u64
                && limit.provenance.as_ref().is_some_and(|provenance| {
                    provenance.domain == OperationResourceDomain::Ascii
                        && provenance.profile
                            == Some(merman::resources::ResourceProfile::Interactive)
                        && provenance.explicit_overrides.as_ref()
                            == [OperationResourceOverride {
                                id: "max_ascii_grid_cells",
                                value: (EXACT_GRID_CELLS - 1) as u64,
                            }]
                })
    ));
}

#[cfg(feature = "ascii")]
#[test]
fn ascii_request_uses_common_cancellation() {
    let control = OperationControl::new();
    control.cancel();
    let error = Renderer::new()
        .render(RenderRequest::ascii(
            "flowchart TD\nA --> B",
            control,
            AsciiRequest::default(),
        ))
        .expect_err("cancelled ASCII request must stop");
    assert!(matches!(error, RenderError::Cancelled(_)));
}

#[cfg(feature = "ascii")]
#[test]
fn ascii_target_cancellation_precedes_option_validation() {
    let control = OperationControl::new();
    let artifact = Renderer::new()
        .prepare_semantic("flowchart TD\nA --> B", control.clone())
        .expect("semantic preparation should succeed")
        .expect("flowchart should produce a semantic artifact");
    control.cancel();

    let mut request = AsciiRequest::default();
    request.options.flowchart_node_label_wrap_width = 0;
    let error = artifact
        .render(RenderTarget::Ascii(request))
        .expect_err("target admission cancellation must win over invalid options");

    assert!(matches!(
        error,
        RenderError::Cancelled(cancelled)
            if cancelled.phase == OperationPhase::Admission
                && cancelled.reason == merman::CancelReason::Requested
    ));
}

#[cfg(feature = "ascii")]
#[test]
fn ascii_target_deadline_precedes_option_validation() {
    let control = OperationControl::new();
    let artifact = Renderer::new()
        .prepare_semantic("flowchart TD\nA --> B", control.clone())
        .expect("semantic preparation should succeed")
        .expect("flowchart should produce a semantic artifact");
    assert!(control.set_deadline(Duration::ZERO));

    let mut request = AsciiRequest::default();
    request.options.flowchart_node_label_wrap_width = 0;
    let error = artifact
        .render(RenderTarget::Ascii(request))
        .expect_err("target admission deadline must win over invalid options");

    assert!(matches!(
        error,
        RenderError::Cancelled(cancelled)
            if cancelled.phase == OperationPhase::Admission
                && cancelled.reason == merman::CancelReason::DeadlineExceeded
    ));
}

#[cfg(feature = "ascii")]
#[test]
fn ascii_target_resource_terminal_precedes_option_validation_without_rewriting_provenance() {
    let control = OperationControl::new();
    let artifact = Renderer::new()
        .prepare_semantic("flowchart TD\nA --> B", control.clone())
        .expect("semantic preparation should succeed")
        .expect("flowchart should produce a semantic artifact");
    let provenance = OperationResourceProvenance::new(
        OperationResourceDomain::Render,
        Some(merman::resources::ResourceProfile::Constrained),
        [OperationResourceOverride {
            id: "max_svg_bytes",
            value: 17,
        }],
    );
    control.terminate_resource_limit(OperationResourceLimitExceeded {
        id: "max_svg_bytes",
        phase: OperationPhase::Emit,
        resource_phase: "svg_output",
        limit: 17,
        consumed: 17,
        requested: 1,
        provenance: provenance.clone(),
    });

    let mut request = AsciiRequest::default();
    request.options.flowchart_node_label_wrap_width = 0;
    let error = artifact
        .render(RenderTarget::Ascii(request))
        .expect_err("the sticky render resource terminal must win before ASCII option validation");

    assert!(matches!(
        error,
        RenderError::ResourceLimitExceeded(details)
            if details.id == "max_svg_bytes"
                && details.phase == "svg_output"
                && details.actual == 18
                && details.maximum == 17
                && details.provenance == Some(provenance)
    ));
}
