mod support;

use merman_ascii::{AsciiError, AsciiRenderOptions, AsciiResourceLimitId, AsciiResourcePolicy};
use merman_core::diagrams::gantt::{GanttDiagramRenderModel, GanttRenderTask};
use merman_core::diagrams::git_graph::{GitGraphCommitRenderModel, GitGraphRenderModel};
use merman_core::diagrams::journey::JourneyDiagramRenderModel;
use merman_core::diagrams::kanban::{KanbanDiagramRenderModel, KanbanRenderNode};
use merman_core::diagrams::mindmap::{MindmapDiagramRenderModel, MindmapDiagramRenderNode};
use merman_core::diagrams::packet::{PacketDiagramRenderModel, PacketRenderBlock};
use merman_core::diagrams::timeline::TimelineDiagramRenderModel;
use merman_core::diagrams::tree_view::TreeViewDiagramRenderModel;
use merman_core::{Engine, OperationControl, OperationPhase, ParseOptions, RenderSemanticModel};
use support::{render_controlled_model, render_model};

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

fn structured_text_models() -> Vec<(&'static str, RenderSemanticModel)> {
    let mut gantt = GanttDiagramRenderModel::default();
    gantt.tasks = vec![
        GanttRenderTask {
            id: "first".to_string(),
            ..GanttRenderTask::default()
        },
        GanttRenderTask {
            id: "second".to_string(),
            ..GanttRenderTask::default()
        },
    ];

    let git_commit = |seq, id: &str| GitGraphCommitRenderModel {
        id: id.to_string(),
        message: String::new(),
        seq,
        commit_type: 0,
        tags: Vec::new(),
        parents: Vec::new(),
        branch: "main".to_string(),
        custom_type: None,
        custom_id: None,
    };
    let git_graph = GitGraphRenderModel {
        diagram_type: "gitGraph".to_string(),
        commits: vec![git_commit(0, "first"), git_commit(1, "second")],
        branches: Vec::new(),
        current_branch: "main".to_string(),
        direction: "TB".to_string(),
        title: None,
        acc_title: None,
        acc_descr: None,
        warning_facts: Vec::new(),
    };

    let mut journey = JourneyDiagramRenderModel::default();
    journey.sections.push("Planning".to_string());

    let kanban = KanbanDiagramRenderModel {
        nodes: vec![
            KanbanRenderNode::new("first", "First"),
            KanbanRenderNode::new("second", "Second"),
        ],
    };

    let mindmap_node = |id: &str| MindmapDiagramRenderNode {
        id: id.to_string(),
        dom_id: format!("node-{id}"),
        label: id.to_string(),
        label_type: String::new(),
        is_group: false,
        shape: "defaultMindmapNode".to_string(),
        width: 0.0,
        height: 0.0,
        padding: 0.0,
        css_classes: String::new(),
        css_styles: Vec::new(),
        look: "classic".to_string(),
        icon: None,
        x: None,
        y: None,
        level: 0,
        node_id: id.to_string(),
        node_type: 0,
        section: None,
    };
    let mindmap = MindmapDiagramRenderModel {
        nodes: vec![mindmap_node("first"), mindmap_node("second")],
        edges: Vec::new(),
    };

    let mut packet = PacketDiagramRenderModel::default();
    packet.packet = vec![vec![
        PacketRenderBlock {
            start: 0,
            end: 0,
            bits: 1,
            label: "first".to_string(),
        },
        PacketRenderBlock {
            start: 1,
            end: 1,
            bits: 1,
            label: "second".to_string(),
        },
    ]];

    let mut timeline = TimelineDiagramRenderModel::default();
    timeline.sections.push("Planning".to_string());

    vec![
        ("gantt", RenderSemanticModel::Gantt(gantt)),
        ("gitGraph", RenderSemanticModel::GitGraph(git_graph)),
        ("journey", RenderSemanticModel::Journey(journey)),
        ("kanban", RenderSemanticModel::Kanban(kanban)),
        ("mindmap", RenderSemanticModel::Mindmap(mindmap)),
        ("packet", RenderSemanticModel::Packet(packet)),
        ("timeline", RenderSemanticModel::Timeline(timeline)),
        (
            "treeView",
            RenderSemanticModel::TreeView(TreeViewDiagramRenderModel::default()),
        ),
    ]
}

#[test]
fn cancelled_operation_is_distinct_from_resource_exhaustion() {
    let model = parse_model("flowchart LR\n  A --> B\n  B --> C\n");
    let control = OperationControl::new();
    control.cancel();

    let error = render_controlled_model(
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
    let error = render_controlled_model(
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
fn flowchart_projection_observes_cancellation_during_semantic_work() {
    let model = parse_model("flowchart LR\n  A --> B\n  B --> C\n");
    let control = OperationControl::new();
    control.cancel_after_checkpoints(2);

    let error = render_controlled_model(
        &model,
        &AsciiRenderOptions::ascii(),
        &control,
        &operation_context(),
        AsciiResourcePolicy::default(),
    )
    .expect_err("flowchart projection should observe scheduled cancellation");

    assert!(matches!(
        error,
        AsciiError::Cancelled(cancelled)
            if cancelled.phase == OperationPhase::Semantic
                && cancelled.reason == merman_core::CancelReason::Requested
    ));
}

#[test]
fn state_projection_observes_cancellation_during_semantic_work() {
    let model = parse_model("stateDiagram-v2\n  [*] --> Active\n  Active --> [*]\n");
    let control = OperationControl::new();
    control.cancel_after_checkpoints(2);

    let error = render_controlled_model(
        &model,
        &AsciiRenderOptions::ascii(),
        &control,
        &operation_context(),
        AsciiResourcePolicy::default(),
    )
    .expect_err("state projection should observe scheduled cancellation");

    assert!(matches!(
        error,
        AsciiError::Cancelled(cancelled)
            if cancelled.phase == OperationPhase::Semantic
                && cancelled.reason == merman_core::CancelReason::Requested
    ));
}

#[test]
fn flowchart_resource_admission_observes_cancellation_before_work_failure() {
    let model = parse_model("flowchart LR\n  A --> B\n");
    let control = OperationControl::new();
    control.cancel_after_checkpoints(3);

    let error = render_controlled_model(
        &model,
        &AsciiRenderOptions::ascii(),
        &control,
        &operation_context(),
        AsciiResourcePolicy::default()
            .with_limit(AsciiResourceLimitId::MaxLayoutWorkUnits, 1)
            .expect("one work unit is a valid ASCII resource limit"),
    )
    .expect_err("scheduled cancellation must precede flowchart work exhaustion");

    assert!(matches!(
        error,
        AsciiError::Cancelled(cancelled)
            if cancelled.phase == OperationPhase::Semantic
                && cancelled.reason == merman_core::CancelReason::Requested
    ));
}

#[test]
fn state_resource_admission_observes_cancellation_before_work_failure() {
    let model = parse_model("stateDiagram-v2\n  [*] --> Active\n  Active --> [*]\n");
    let control = OperationControl::new();
    control.cancel_after_checkpoints(3);

    let error = render_controlled_model(
        &model,
        &AsciiRenderOptions::ascii(),
        &control,
        &operation_context(),
        AsciiResourcePolicy::default()
            .with_limit(AsciiResourceLimitId::MaxLayoutWorkUnits, 1)
            .expect("one work unit is a valid ASCII resource limit"),
    )
    .expect_err("scheduled cancellation must precede state work exhaustion");

    assert!(matches!(
        error,
        AsciiError::Cancelled(cancelled)
            if cancelled.phase == OperationPhase::Semantic
                && cancelled.reason == merman_core::CancelReason::Requested
    ));
}

#[test]
fn sequence_projection_observes_cancellation_at_semantic_checkpoint() {
    let model =
        parse_model("sequenceDiagram\nparticipant A\nparticipant B\nA->>B: one\nB-->>A: two\n");
    let control = OperationControl::new();
    control.cancel_after_checkpoints(2);

    let error = render_controlled_model(
        &model,
        &AsciiRenderOptions::ascii(),
        &control,
        &operation_context(),
        AsciiResourcePolicy::default(),
    )
    .expect_err("sequence projection should observe scheduled cancellation");

    assert!(matches!(
        error,
        AsciiError::Cancelled(cancelled)
            if cancelled.phase == OperationPhase::Semantic
                && cancelled.reason == merman_core::CancelReason::Requested
    ));
}

#[test]
fn class_resource_admission_observes_cancellation_before_layout_work_failure() {
    let model = parse_model("classDiagram\nclass A\n");
    let control = OperationControl::new();
    control.cancel_after_checkpoints(3);

    let error = render_controlled_model(
        &model,
        &AsciiRenderOptions::ascii(),
        &control,
        &operation_context(),
        AsciiResourcePolicy::default()
            .with_limit(AsciiResourceLimitId::MaxLayoutWorkUnits, 1)
            .expect("one work unit is a valid ASCII resource limit"),
    )
    .expect_err("scheduled cancellation must precede class work exhaustion");

    assert!(matches!(
        error,
        AsciiError::Cancelled(cancelled)
            if cancelled.phase == OperationPhase::Semantic
                && cancelled.reason == merman_core::CancelReason::Requested
    ));
}

#[test]
fn er_resource_admission_observes_cancellation_before_layout_work_failure() {
    let model = parse_model("erDiagram\nA {\n  string id\n}\n");
    let control = OperationControl::new();
    control.cancel_after_checkpoints(3);

    let error = render_controlled_model(
        &model,
        &AsciiRenderOptions::ascii(),
        &control,
        &operation_context(),
        AsciiResourcePolicy::default()
            .with_limit(AsciiResourceLimitId::MaxLayoutWorkUnits, 1)
            .expect("one work unit is a valid ASCII resource limit"),
    )
    .expect_err("scheduled cancellation must precede ER work exhaustion");

    assert!(matches!(
        error,
        AsciiError::Cancelled(cancelled)
            if cancelled.phase == OperationPhase::Semantic
                && cancelled.reason == merman_core::CancelReason::Requested
    ));
}

#[test]
fn xychart_resource_admission_observes_cancellation_before_layout_work_failure() {
    let model = parse_model("xychart\nx-axis [A]\ny-axis 0 --> 1\nbar [1]\n");
    let control = OperationControl::new();
    control.cancel_after_checkpoints(3);

    let error = render_controlled_model(
        &model,
        &AsciiRenderOptions::ascii(),
        &control,
        &operation_context(),
        AsciiResourcePolicy::default()
            .with_limit(AsciiResourceLimitId::MaxLayoutWorkUnits, 1)
            .expect("one work unit is a valid ASCII resource limit"),
    )
    .expect_err("scheduled cancellation must precede XYChart work exhaustion");

    assert!(matches!(
        error,
        AsciiError::Cancelled(cancelled)
            if cancelled.phase == OperationPhase::Layout
                && cancelled.reason == merman_core::CancelReason::Requested
    ));
}

#[test]
fn structured_text_resource_admission_observes_cancellation_before_layout_work_failure() {
    let resources = AsciiResourcePolicy::default()
        .with_limit(AsciiResourceLimitId::MaxLayoutWorkUnits, 1)
        .expect("one work unit is a valid ASCII resource limit");

    for (diagram_type, model) in structured_text_models() {
        let resource_error = render_controlled_model(
            &model,
            &AsciiRenderOptions::ascii(),
            &OperationControl::new(),
            &operation_context(),
            resources,
        )
        .expect_err("the constrained family model should exceed one layout work unit");
        assert!(
            matches!(
                resource_error,
                AsciiError::ResourceLimitExceeded(details)
                    if details.limit == AsciiResourceLimitId::MaxLayoutWorkUnits
            ),
            "{diagram_type} should reach the expected layout-work ceiling without cancellation: {resource_error:?}",
        );

        let control = OperationControl::new();
        control.cancel_after_checkpoints(2);
        let cancelled = render_controlled_model(
            &model,
            &AsciiRenderOptions::ascii(),
            &control,
            &operation_context(),
            resources,
        )
        .expect_err("scheduled cancellation must precede structured-text work exhaustion");
        assert!(
            matches!(
                cancelled,
                AsciiError::Cancelled(details)
                    if details.phase == OperationPhase::Layout
                        && details.reason == merman_core::CancelReason::Requested
            ),
            "{diagram_type} should report layout cancellation before the resource ceiling: {cancelled:?}",
        );
    }
}

#[test]
fn explicit_control_success_matches_default_test_operation_output() {
    let model = parse_model("flowchart LR\n  A[Start] --> B[Finish]\n");
    let options = AsciiRenderOptions::ascii();
    let expected = render_model(&model, &options).expect("default test operation should succeed");
    let actual = render_controlled_model(
        &model,
        &options,
        &OperationControl::new(),
        &operation_context(),
        AsciiResourcePolicy::default(),
    )
    .expect("controlled render should succeed");

    assert_eq!(actual, expected);
}
