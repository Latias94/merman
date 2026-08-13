#[cfg(feature = "svg")]
use std::sync::Arc;
use std::time::Duration;

use merman::{
    OperationControl, OperationPhase, ParseOptions, RenderError, RenderOutput, RenderRequest,
    Renderer, SemanticArtifact,
    resources::{InputResourceLimitId, InputResourcePolicy},
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

#[cfg(feature = "ascii")]
#[test]
fn ascii_request_uses_target_local_grid_policy_and_common_cancellation() {
    let control = OperationControl::new();
    control.cancel();
    let error = Renderer::new()
        .render(RenderRequest::ascii(
            "flowchart TD\nA --> B",
            control,
            merman::AsciiRequest::default(),
        ))
        .expect_err("cancelled ASCII request must stop");
    assert!(matches!(error, RenderError::Cancelled(_)));
}
