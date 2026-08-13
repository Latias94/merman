use merman_ascii::{
    AsciiError, AsciiRenderOptions, AsciiResourceLimitId, AsciiResourcePolicy, render_model,
    render_model_with_operation,
};
use merman_core::{Engine, OperationControl, OperationPhase, ParseOptions, RenderSemanticModel};

fn parse_model(source: &str) -> RenderSemanticModel {
    Engine::new()
        .parse_diagram_for_render_model_sync(source, ParseOptions::strict())
        .expect("diagram should parse")
        .expect("diagram should be detected")
        .into_parts()
        .1
}

fn operation_context() -> merman_core::runtime::OperationContext {
    Engine::new()
        .begin_operation()
        .expect("deterministic operation context should be available")
}

#[test]
fn cancelled_operation_is_distinct_from_resource_exhaustion() {
    let model = parse_model("flowchart LR\n  A --> B\n  B --> C\n");
    let control = OperationControl::new();
    control.cancel();

    let error = render_model_with_operation(
        &model,
        &AsciiRenderOptions::ascii(),
        &control,
        &operation_context(),
        AsciiResourcePolicy::default(),
    )
    .expect_err("cancelled operation must not produce output");

    assert!(matches!(
        error,
        AsciiError::Cancelled(cancelled)
            if cancelled.phase == OperationPhase::Admission
                && cancelled.reason == merman_core::CancelReason::Requested
    ));
}

#[test]
fn flowchart_grid_admission_returns_structured_resource_error() {
    let model = parse_model("flowchart LR\n  A --> B\n");
    let error = render_model_with_operation(
        &model,
        &AsciiRenderOptions::ascii(),
        &OperationControl::new(),
        &operation_context(),
        AsciiResourcePolicy::default()
            .with_limit(AsciiResourceLimitId::MaxGridCells, 1)
            .expect("one cell is a valid ASCII resource limit"),
    )
    .expect_err("the flowchart canvas should exceed one cell");

    assert!(matches!(
        error,
        AsciiError::ResourceLimitExceeded(resource)
            if resource.limit == AsciiResourceLimitId::MaxGridCells
                && resource.phase() == merman_ascii::AsciiResourceLimitPhase::Layout
                && resource.max == 1
                && resource.actual > resource.max
    ));
}

#[test]
fn sequence_event_traversal_observes_cancellation_at_layout_checkpoint() {
    let model =
        parse_model("sequenceDiagram\nparticipant A\nparticipant B\nA->>B: one\nB-->>A: two\n");
    let control = OperationControl::new();
    control.cancel_after_checkpoints(4);

    let error = render_model_with_operation(
        &model,
        &AsciiRenderOptions::ascii(),
        &control,
        &operation_context(),
        AsciiResourcePolicy::default(),
    )
    .expect_err("sequence traversal should observe scheduled cancellation");

    assert!(matches!(
        error,
        AsciiError::Cancelled(cancelled)
            if cancelled.phase == OperationPhase::Layout
                && cancelled.reason == merman_core::CancelReason::Requested
    ));
}

#[test]
fn controlled_success_matches_uncontrolled_flowchart_output() {
    let model = parse_model("flowchart LR\n  A[Start] --> B[Finish]\n");
    let options = AsciiRenderOptions::ascii();
    let expected = render_model(&model, &options).expect("uncontrolled render should succeed");
    let actual = render_model_with_operation(
        &model,
        &options,
        &OperationControl::new(),
        &operation_context(),
        AsciiResourcePolicy::default(),
    )
    .expect("controlled render should succeed");

    assert_eq!(actual, expected);
}
