use std::time::Duration;

use merman::{
    OperationControl, OperationPhase, RenderError, RenderOutput, RenderRequest, Renderer,
    SemanticArtifact,
};

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
