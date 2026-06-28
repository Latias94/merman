use merman_ascii::{AsciiRenderOptions, render_model};
use merman_core::diagram::RenderSemanticModel;
use merman_core::diagrams::gantt::{GanttDiagramRenderModel, GanttRenderTask};
use merman_core::diagrams::git_graph::{
    GitGraphBranchRenderModel, GitGraphCommitRenderModel, GitGraphRenderModel,
};
use merman_core::diagrams::journey::{JourneyDiagramRenderModel, JourneyRenderTask};
use merman_core::diagrams::kanban::{KanbanDiagramRenderModel, KanbanRenderNode};
use merman_core::diagrams::mindmap::{
    MindmapDiagramRenderEdge, MindmapDiagramRenderModel, MindmapDiagramRenderNode,
};
use merman_core::diagrams::packet::{PacketDiagramRenderModel, PacketRenderBlock};
use merman_core::diagrams::timeline::{TimelineDiagramRenderModel, TimelineRenderTask};
use merman_core::diagrams::tree_view::{TreeViewDiagramRenderModel, TreeViewNodeRenderModel};

fn render(model: RenderSemanticModel) -> String {
    render_model(&model, &AsciiRenderOptions::ascii()).unwrap()
}

fn tree_node(
    id: i64,
    level: i64,
    name: &str,
    children: Vec<TreeViewNodeRenderModel>,
) -> TreeViewNodeRenderModel {
    TreeViewNodeRenderModel {
        id,
        level,
        name: name.to_string(),
        children,
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

fn kanban_node(
    id: &str,
    label: &str,
    is_group: bool,
    parent_id: Option<&str>,
    ticket: Option<&str>,
    priority: Option<&str>,
    assigned: Option<&str>,
    icon: Option<&str>,
) -> KanbanRenderNode {
    KanbanRenderNode {
        id: id.to_string(),
        label: label.to_string(),
        is_group,
        parent_id: parent_id.map(str::to_string),
        ticket: ticket.map(str::to_string),
        priority: priority.map(str::to_string),
        assigned: assigned.map(str::to_string),
        icon: icon.map(str::to_string),
    }
}

#[test]
fn tree_view_render_model_renders_outline_summary() {
    let model = TreeViewDiagramRenderModel {
        acc_title: Some("Tree title".to_string()),
        acc_descr: Some("Tree description".to_string()),
        title: Some("Project".to_string()),
        root: tree_node(
            0,
            -1,
            "/",
            vec![
                tree_node(
                    1,
                    0,
                    "Root",
                    vec![
                        tree_node(2, 1, "Child 1", Vec::new()),
                        tree_node(3, 1, "Child 2", Vec::new()),
                    ],
                ),
                tree_node(4, 0, "Sibling", Vec::new()),
            ],
        ),
    };

    let rendered = render(RenderSemanticModel::TreeView(model));

    assert_eq!(
        rendered,
        concat!(
            "Project\n",
            "accTitle: Tree title\n",
            "accDescr: Tree description\n",
            "|-- Root\n",
            "|   |-- Child 1\n",
            "|   \\-- Child 2\n",
            "\\-- Sibling",
        )
    );
}

#[test]
fn mindmap_render_model_renders_rooted_outline() {
    let model = MindmapDiagramRenderModel {
        nodes: vec![
            mindmap_node("root", "Root", 0),
            mindmap_node("child1", "Child 1", 1),
            mindmap_node("child2", "Child 2", 1),
            mindmap_node("leaf", "Leaf", 2),
        ],
        edges: vec![
            mindmap_edge("e1", "root", "child1"),
            mindmap_edge("e2", "root", "child2"),
            mindmap_edge("e3", "child1", "leaf"),
        ],
    };

    let rendered = render(RenderSemanticModel::Mindmap(model));

    assert_eq!(
        rendered,
        concat!("Root\n", "|-- Child 1\n", "|   \\-- Leaf\n", "\\-- Child 2",)
    );
}

#[test]
fn timeline_render_model_renders_sections_tasks_and_events() {
    let model = TimelineDiagramRenderModel {
        title: Some("Timeline".to_string()),
        acc_title: Some("Timeline title".to_string()),
        acc_descr: Some("Timeline description".to_string()),
        sections: vec!["Planning".to_string()],
        tasks: vec![
            TimelineRenderTask {
                id: 0,
                section: "Planning".to_string(),
                task_type: "Planning".to_string(),
                task: "Design".to_string(),
                score: 0,
                events: vec!["Kickoff".to_string()],
            },
            TimelineRenderTask {
                id: 1,
                section: "Planning".to_string(),
                task_type: "Planning".to_string(),
                task: "Implement".to_string(),
                score: 3,
                events: vec!["Build spec".to_string(), "Review".to_string()],
            },
        ],
    };

    let rendered = render(RenderSemanticModel::Timeline(model));

    assert_eq!(
        rendered,
        concat!(
            "Timeline\n",
            "accTitle: Timeline title\n",
            "accDescr: Timeline description\n",
            "section: Planning\n",
            "  - Design\n",
            "    * Kickoff\n",
            "  - Implement (score 3)\n",
            "    * Build spec\n",
            "    * Review",
        )
    );
}

#[test]
fn gantt_render_model_renders_sections_tasks_and_flags() {
    let model = GanttDiagramRenderModel {
        title: Some("Gantt".to_string()),
        acc_title: Some("Gantt title".to_string()),
        acc_descr: Some("Gantt description".to_string()),
        date_format: "YYYY-MM-DD".to_string(),
        axis_format: "%d".to_string(),
        tasks: vec![GanttRenderTask {
            id: "task-1".to_string(),
            task: "Implement".to_string(),
            section: "Build".to_string(),
            task_type: "Build".to_string(),
            classes: Vec::new(),
            active: true,
            done: true,
            crit: true,
            milestone: true,
            vert: true,
            order: 0,
            start_ms: 9_223_372_036_854_775_000,
            end_ms: 9_223_372_036_854_775_001,
            render_end_ms: Some(9_223_372_036_854_775_002),
        }],
        ..Default::default()
    };

    let rendered = render(RenderSemanticModel::Gantt(model));

    assert_eq!(
        rendered,
        concat!(
            "Gantt\n",
            "accTitle: Gantt title\n",
            "accDescr: Gantt description\n",
            "dateFormat: YYYY-MM-DD\n",
            "axisFormat: %d\n",
            "section: Build\n",
            "  - Implement [9223372036854775000 -> 9223372036854775002] [milestone, active, done, crit, vert]",
        )
    );
}

#[test]
fn journey_render_model_renders_actors_sections_and_scores() {
    let model = JourneyDiagramRenderModel {
        title: Some("Journey".to_string()),
        acc_title: Some("Journey title".to_string()),
        acc_descr: Some("Journey description".to_string()),
        sections: vec!["Discovery".to_string()],
        tasks: vec![
            JourneyRenderTask {
                score: 5,
                score_is_nan: false,
                people: vec!["Alice".to_string(), "Bob".to_string()],
                section: "Discovery".to_string(),
                task_type: "Discovery".to_string(),
                task: "Research".to_string(),
            },
            JourneyRenderTask {
                score: 3,
                score_is_nan: false,
                people: vec!["Bob".to_string()],
                section: "Discovery".to_string(),
                task_type: "Discovery".to_string(),
                task: "Ship".to_string(),
            },
        ],
        actors: Vec::new(),
    };

    let rendered = render(RenderSemanticModel::Journey(model));

    assert_eq!(
        rendered,
        concat!(
            "Journey\n",
            "accTitle: Journey title\n",
            "accDescr: Journey description\n",
            "actors: Alice, Bob\n",
            "section: Discovery\n",
            "  - Research [score 5] (Alice, Bob)\n",
            "  - Ship [score 3] (Bob)",
        )
    );
}

#[test]
fn kanban_render_model_renders_groups_and_child_metadata() {
    let model = KanbanDiagramRenderModel {
        nodes: vec![
            kanban_node("backlog", "Backlog", true, None, None, None, None, None),
            kanban_node(
                "card-a",
                "Ticket A",
                false,
                Some("backlog"),
                Some("K-1"),
                Some("high"),
                Some("alice"),
                Some("bug"),
            ),
            kanban_node(
                "card-b",
                "Ticket B",
                false,
                Some("backlog"),
                Some("K-2"),
                None,
                None,
                None,
            ),
            kanban_node("doing", "Doing", true, None, None, None, None, None),
            kanban_node(
                "card-c",
                "Ticket C",
                false,
                Some("doing"),
                Some("K-3"),
                None,
                None,
                None,
            ),
        ],
    };

    let rendered = render(RenderSemanticModel::Kanban(model));

    assert_eq!(
        rendered,
        concat!(
            "Backlog\n",
            "  - Ticket A [ticket=K-1, priority=high, assigned=alice, icon=bug]\n",
            "  - Ticket B [ticket=K-2]\n",
            "Doing\n",
            "  - Ticket C [ticket=K-3]",
        )
    );
}

#[test]
fn packet_render_model_renders_rows_and_ranges() {
    let model = PacketDiagramRenderModel {
        title: Some("Packet".to_string()),
        acc_title: Some("Packet title".to_string()),
        acc_descr: Some("Packet description".to_string()),
        packet: vec![
            vec![
                PacketRenderBlock {
                    start: 0,
                    end: 7,
                    bits: 8,
                    label: "header".to_string(),
                },
                PacketRenderBlock {
                    start: 8,
                    end: 15,
                    bits: 8,
                    label: "payload".to_string(),
                },
            ],
            vec![PacketRenderBlock {
                start: 16,
                end: 31,
                bits: 16,
                label: "footer".to_string(),
            }],
        ],
    };

    let rendered = render(RenderSemanticModel::Packet(model));

    assert_eq!(
        rendered,
        concat!(
            "Packet\n",
            "accTitle: Packet title\n",
            "accDescr: Packet description\n",
            "row 1: [0..7] header | [8..15] payload\n",
            "row 2: [16..31] footer",
        )
    );
}

#[test]
fn git_graph_render_model_renders_branches_commits_and_warnings() {
    let model = GitGraphRenderModel {
        diagram_type: "gitGraph".to_string(),
        commits: vec![GitGraphCommitRenderModel {
            id: "c0".to_string(),
            message: "init".to_string(),
            seq: 0,
            commit_type: 2,
            tags: vec!["v1".to_string()],
            parents: vec!["seed".to_string()],
            branch: "main".to_string(),
            custom_type: Some(7),
            custom_id: Some(true),
        }],
        branches: vec![
            GitGraphBranchRenderModel {
                name: "main".to_string(),
            },
            GitGraphBranchRenderModel {
                name: "feature".to_string(),
            },
        ],
        current_branch: "main".to_string(),
        direction: "TB".to_string(),
        acc_title: Some("Git title".to_string()),
        acc_descr: Some("Git description".to_string()),
        warnings: vec!["duplicate head".to_string()],
    };

    let rendered = render(RenderSemanticModel::GitGraph(model));

    assert_eq!(
        rendered,
        concat!(
            "gitGraph direction=TB current=main\n",
            "accTitle: Git title\n",
            "accDescr: Git description\n",
            "branches: main, feature\n",
            "  - 0 main c0 [highlight] init tags=v1 parents=seed customType=7 customId=true\n",
            "warnings:\n",
            "  - duplicate head",
        )
    );
}
