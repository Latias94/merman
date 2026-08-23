mod support;

use merman_ascii::{AsciiError, AsciiRenderOptions, AsciiResourcePolicy};
use merman_core::diagram::RenderSemanticModel;
use merman_core::diagrams::gantt::{
    GanttDiagramRenderModel, GanttRenderTask, GanttRenderTaskEnd, GanttRenderTaskRaw,
    GanttRenderTaskStart, GanttTaskEndConstraint, GanttTaskStartConstraint,
};
use merman_core::diagrams::git_graph::{
    GitGraphBranchRenderModel, GitGraphCommitRenderModel, GitGraphRenderModel,
};
use merman_core::diagrams::journey::{JourneyDiagramRenderModel, JourneyRenderTask};
use merman_core::diagrams::kanban::{KanbanDiagramRenderModel, KanbanRenderNode};
use merman_core::diagrams::mindmap::{
    MindmapDiagramRenderEdge, MindmapDiagramRenderModel, MindmapDiagramRenderNode,
};
use merman_core::diagrams::packet::{PacketDiagramRenderModel, PacketRenderBlock};
use merman_core::diagrams::timeline::{
    TimelineDiagramRenderModel, TimelineDirection, TimelineRenderTask,
};
use merman_core::diagrams::tree_view::{TreeViewDiagramRenderModel, TreeViewNodeRenderModel};
use merman_core::{DiagramWarningFact, GIT_GRAPH_DUPLICATE_COMMIT_WARNING_RULE_ID};
use support::{render_controlled_model, render_model};

fn render(model: RenderSemanticModel) -> String {
    render_model(&model, &AsciiRenderOptions::ascii()).unwrap()
}

fn render_with_options(model: RenderSemanticModel, options: &AsciiRenderOptions) -> String {
    render_model(&model, options).unwrap()
}

fn render_parsed(input: &str) -> String {
    let engine = merman_core::Engine::new();
    let parsed = engine
        .parse_diagram_for_render_model_sync(input, merman_core::ParseOptions::strict())
        .unwrap()
        .unwrap();
    render_model(parsed.model(), &AsciiRenderOptions::ascii()).unwrap()
}

fn render_with_scheduled_cancellation(
    model: RenderSemanticModel,
    successful_checkpoints: usize,
) -> AsciiError {
    let engine = merman_core::Engine::new();
    let context = engine
        .begin_operation()
        .expect("deterministic operation context should be available");
    let control = merman_core::OperationControl::new();
    control.cancel_after_checkpoints(successful_checkpoints);
    render_controlled_model(
        &model,
        &AsciiRenderOptions::ascii(),
        &control,
        &context,
        AsciiResourcePolicy::default(),
    )
    .expect_err("scheduled cancellation must prevent summary output")
}

fn tree_node(
    id: i64,
    level: i64,
    name: &str,
    children: Vec<TreeViewNodeRenderModel>,
) -> TreeViewNodeRenderModel {
    let node_type = if children.is_empty() {
        "file"
    } else {
        "directory"
    };
    TreeViewNodeRenderModel {
        id,
        level,
        name: name.to_string(),
        node_type: node_type.to_string(),
        children,
        ..Default::default()
    }
}

fn mindmap_node(id: &str, label: &str, level: i64) -> MindmapDiagramRenderNode {
    MindmapDiagramRenderNode {
        id: id.to_string(),
        dom_id: format!("node_{id}"),
        label: label.to_string(),
        label_type: String::new(),
        is_group: false,
        shape: "defaultMindmapNode".to_string(),
        width: 40.0,
        height: 24.0,
        padding: 10.0,
        css_classes: String::new(),
        css_styles: Vec::new(),
        look: "classic".to_string(),
        icon: None,
        x: None,
        y: None,
        level,
        node_id: id.to_string(),
        node_type: 0,
        section: None,
    }
}

fn mindmap_edge(id: &str, start: &str, end: &str) -> MindmapDiagramRenderEdge {
    MindmapDiagramRenderEdge {
        id: id.to_string(),
        start: start.to_string(),
        end: end.to_string(),
        edge_type: String::new(),
        curve: String::new(),
        thickness: String::new(),
        look: String::new(),
        classes: String::new(),
        depth: 0,
        section: None,
    }
}

#[derive(Default)]
struct KanbanNodeMetadata<'a> {
    parent_id: Option<&'a str>,
    ticket: Option<&'a str>,
    priority: Option<&'a str>,
    assigned: Option<&'a str>,
    icon: Option<&'a str>,
}

fn kanban_node(
    id: &str,
    label: &str,
    is_group: bool,
    metadata: KanbanNodeMetadata<'_>,
) -> KanbanRenderNode {
    let mut node = KanbanRenderNode::new(id, label);
    node.is_group = is_group;
    node.parent_id = metadata.parent_id.map(str::to_string);
    node.ticket = metadata.ticket.map(str::to_string);
    node.priority = metadata.priority.map(str::to_string);
    node.assigned = metadata.assigned.map(str::to_string);
    node.icon = metadata.icon.map(str::to_string);
    node
}

#[path = "new_family_models/gantt.rs"]
mod gantt;
#[path = "new_family_models/git_graph.rs"]
mod git_graph;
#[path = "new_family_models/journey.rs"]
mod journey;
#[path = "new_family_models/kanban.rs"]
mod kanban;
#[path = "new_family_models/mindmap.rs"]
mod mindmap;
#[path = "new_family_models/packet.rs"]
mod packet;
#[path = "new_family_models/timeline.rs"]
mod timeline;
#[path = "new_family_models/tree_view.rs"]
mod tree_view;
