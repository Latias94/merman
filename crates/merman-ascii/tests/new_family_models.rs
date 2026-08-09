use merman_ascii::{AsciiError, AsciiRenderOptions, render_model};
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
use merman_core::diagrams::timeline::{
    TimelineDiagramRenderModel, TimelineDirection, TimelineRenderTask,
};
use merman_core::diagrams::tree_view::{TreeViewDiagramRenderModel, TreeViewNodeRenderModel};
use merman_core::{DiagramWarningFact, GIT_GRAPH_DUPLICATE_COMMIT_WARNING_RULE_ID};

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
            "/ [id=0, level=-1]\n",
            "|-- Root/ [id=1, level=0]\n",
            "|   |-- Child 1 [id=2, level=1]\n",
            "|   \\-- Child 2 [id=3, level=1]\n",
            "\\-- Sibling [id=4, level=0]",
        )
    );
}

#[test]
fn tree_view_discloses_typed_fields_and_honors_unicode_connectors() {
    let file = TreeViewNodeRenderModel {
        id: 2,
        level: 1,
        name: "App.tsx".to_string(),
        node_type: "file".to_string(),
        icon: Some("react".to_string()),
        description: Some("main component".to_string()),
        ..Default::default()
    };
    let directory = TreeViewNodeRenderModel {
        id: 1,
        level: 0,
        name: "src".to_string(),
        node_type: "directory".to_string(),
        css_class: Some("highlight".to_string()),
        icon: Some("folder".to_string()),
        description: Some("source directory".to_string()),
        children: vec![file],
    };
    let model = TreeViewDiagramRenderModel {
        root: TreeViewNodeRenderModel {
            children: vec![directory],
            ..Default::default()
        },
        ..Default::default()
    };

    let ascii = render_with_options(
        RenderSemanticModel::TreeView(model.clone()),
        &AsciiRenderOptions::ascii(),
    );
    let unicode = render_with_options(
        RenderSemanticModel::TreeView(model),
        &AsciiRenderOptions::unicode(),
    );

    assert_eq!(
        ascii,
        concat!(
            "/ [id=0, level=-1]\n",
            "\\-- src/ [id=1, level=0, icon=folder, class=highlight] -- source directory\n",
            "    \\-- App.tsx [id=2, level=1, icon=react] -- main component",
        )
    );
    assert_eq!(
        unicode,
        concat!(
            "/ [id=0, level=-1]\n",
            "└── src/ [id=1, level=0, icon=folder, class=highlight] -- source directory\n",
            "    └── App.tsx [id=2, level=1, icon=react] -- main component",
        )
    );
}

#[test]
fn tree_view_rejects_duplicate_public_node_ids() {
    let model = TreeViewDiagramRenderModel {
        root: tree_node(
            0,
            -1,
            "workspace",
            vec![
                tree_node(1, 0, "first", Vec::new()),
                tree_node(1, 0, "second", Vec::new()),
            ],
        ),
        ..Default::default()
    };

    let error = render_model(
        &RenderSemanticModel::TreeView(model),
        &AsciiRenderOptions::ascii(),
    )
    .expect_err("duplicate TreeView identities must be rejected before projection");

    assert_eq!(
        error,
        AsciiError::UnsupportedFeature {
            diagram_type: "treeView",
            feature: "duplicate node ids",
        }
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
        concat!(
            "Root [id=root]\n",
            "|-- Child 1 [id=child1]\n",
            "|   \\-- Leaf [id=leaf]\n",
            "\\-- Child 2 [id=child2]",
        )
    );
}

#[test]
fn mindmap_render_model_rejects_parallel_edges() {
    let model = MindmapDiagramRenderModel {
        nodes: vec![
            mindmap_node("root", "Root", 0),
            mindmap_node("child", "Child", 1),
        ],
        edges: vec![
            mindmap_edge("e1", "root", "child"),
            mindmap_edge("e2", "root", "child"),
            mindmap_edge("e3", "child", "root"),
        ],
    };

    let error = render_model(
        &RenderSemanticModel::Mindmap(model),
        &AsciiRenderOptions::ascii(),
    )
    .expect_err("parallel mindmap edges must not be silently deduplicated");

    assert!(matches!(
        error,
        AsciiError::UnsupportedFeature {
            diagram_type: "mindmap",
            feature: "parallel edges",
        }
    ));
}

#[test]
fn mindmap_render_model_keeps_disconnected_cycle_components() {
    let model = MindmapDiagramRenderModel {
        nodes: vec![
            mindmap_node("root", "Root", 0),
            mindmap_node("child", "Child", 1),
            mindmap_node("a", "A", 0),
            mindmap_node("b", "B", 1),
        ],
        edges: vec![
            mindmap_edge("e1", "root", "child"),
            mindmap_edge("e2", "a", "b"),
            mindmap_edge("e3", "b", "a"),
        ],
    };

    let rendered = render(RenderSemanticModel::Mindmap(model));

    assert_eq!(
        rendered,
        concat!(
            "Root [id=root]\n",
            "\\-- Child [id=child]\n",
            "\n",
            "A [id=a]\n",
            "\\-- B [id=b]\n",
            "    \\-- A [id=a] (cycle)",
        )
    );
}

#[test]
fn mindmap_direct_model_rejects_duplicate_ids_and_missing_edges() {
    let mut duplicate = MindmapDiagramRenderModel {
        nodes: vec![mindmap_node("same", "A", 0), mindmap_node("same", "B", 0)],
        edges: Vec::new(),
    };
    let error = render_model(
        &RenderSemanticModel::Mindmap(duplicate.clone()),
        &AsciiRenderOptions::ascii(),
    )
    .expect_err("duplicate node ids must be rejected");
    assert!(matches!(
        error,
        AsciiError::UnsupportedFeature {
            diagram_type: "mindmap",
            feature: "duplicate node ids",
        }
    ));

    duplicate.nodes = vec![mindmap_node("root", "Root", 0)];
    duplicate.edges = vec![mindmap_edge("edge", "root", "missing")];
    let error = render_model(
        &RenderSemanticModel::Mindmap(duplicate),
        &AsciiRenderOptions::ascii(),
    )
    .expect_err("missing edge endpoints must be rejected");
    assert!(matches!(
        error,
        AsciiError::UnsupportedFeature {
            diagram_type: "mindmap",
            feature: "edge with missing end node",
        }
    ));
}

#[test]
fn mindmap_structured_text_discloses_identity_shape_icon_and_section() {
    let mut root = mindmap_node("root", "Root", 0);
    root.shape = "circle".to_string();
    root.icon = Some("home".to_string());
    root.section = Some(2);

    let rendered = render(RenderSemanticModel::Mindmap(MindmapDiagramRenderModel {
        nodes: vec![root],
        edges: Vec::new(),
    }));

    assert_eq!(
        rendered,
        "Root [id=root] [shape=circle] [icon=home] [section=2]"
    );
}

#[test]
fn timeline_render_model_renders_sections_tasks_and_events() {
    let mut model = TimelineDiagramRenderModel::default();
    model.title = Some("Timeline".to_string());
    model.acc_title = Some("Timeline title".to_string());
    model.acc_descr = Some("Timeline description".to_string());
    model.direction = TimelineDirection::TopDown;
    model.sections = vec!["Planning".to_string()];
    model.tasks = vec![
        TimelineRenderTask {
            id: 0,
            section: "Planning".to_string(),
            section_index: Some(0),
            task_type: "Planning".to_string(),
            task: "Design".to_string(),
            score: 0,
            events: vec!["Kickoff".to_string()],
        },
        TimelineRenderTask {
            id: 1,
            section: "Planning".to_string(),
            section_index: Some(0),
            task_type: "Planning".to_string(),
            task: "Implement".to_string(),
            score: 3,
            events: vec!["Build spec".to_string(), "Review".to_string()],
        },
    ];

    let rendered = render(RenderSemanticModel::Timeline(model));

    assert_eq!(
        rendered,
        concat!(
            "Timeline\n",
            "accTitle: Timeline title\n",
            "accDescr: Timeline description\n",
            "direction: TD\n",
            "section: Planning\n",
            "  - Design\n",
            "    * Kickoff\n",
            "  - Implement\n",
            "    * Build spec\n",
            "    * Review",
        )
    );
}

#[test]
fn timeline_render_model_wraps_long_task_and_event_text() {
    let mut model = TimelineDiagramRenderModel::default();
    model.sections = vec!["Planning".to_string()];
    model.tasks = vec![TimelineRenderTask {
            id: 0,
            section: "Planning".to_string(),
            section_index: Some(0),
            task_type: "Planning".to_string(),
            task: "Design a very long integration event stream normalization workflow that still fits readable terminal output".to_string(),
            score: 0,
            events: vec![
                "Capture every upstream payload variant without losing the important operational context".to_string(),
            ],
        }];

    let rendered = render(RenderSemanticModel::Timeline(model));

    assert_eq!(
        rendered,
        concat!(
            "direction: LR\n",
            "section: Planning\n",
            "  - Design a very long integration event stream normalization workflow that\n",
            "    still fits readable terminal output\n",
            "    * Capture every upstream payload variant without losing the important\n",
            "      operational context",
        )
    );
}

#[test]
fn gantt_render_model_renders_sections_tasks_and_flags() {
    let mut model = GanttDiagramRenderModel::default();
    model.title = Some("Gantt".to_string());
    model.acc_title = Some("Gantt title".to_string());
    model.acc_descr = Some("Gantt description".to_string());
    model.date_format = "YYYY-MM-DD".to_string();
    model.axis_format = "%d".to_string();
    model.sections = vec![
        "Empty".to_string(),
        "Empty".to_string(),
        "Build".to_string(),
    ];
    model.tasks = vec![GanttRenderTask {
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
        ..Default::default()
    }];

    let rendered = render(RenderSemanticModel::Gantt(model));

    assert_eq!(
        rendered,
        concat!(
            "Gantt\n",
            "accTitle: Gantt title\n",
            "accDescr: Gantt description\n",
            "dateFormat: YYYY-MM-DD\n",
            "axisFormat: %d\n",
            "section: Empty\n",
            "section: Empty\n",
            "section: Build\n",
            "  - Implement [id=task-1, range=+292278994-08-17T07:12:55.000 ->\n",
            "    +292278994-08-17T07:12:55.001, renderEnd=+292278994-08-17T07:12:55.002,\n",
            "    flags=milestone, active, done, crit, vert]",
        )
    );
}

#[test]
fn gantt_structured_text_discloses_dependency_source_expressions() {
    let rendered = render_parsed(concat!(
        "gantt\n",
        "dateFormat YYYY-MM-DD\n",
        "section Build\n",
        "Design: design, 2026-01-01, 1d\n",
        "Review: review, 2026-01-02, 1d\n",
        "Implement: implementation, after design review, until design review\n",
    ));

    let normalized = rendered.split_whitespace().collect::<Vec<_>>().join(" ");
    assert!(
        normalized.contains("after=design review"),
        "structured Gantt output should disclose the dependency source expression:\n{rendered}"
    );
    assert!(normalized.contains("until=design review"));
    assert!(rendered.contains("id=design"));
    assert!(rendered.contains("id=review"));
    assert!(rendered.contains("id=implementation"));
}

#[test]
fn gantt_structured_text_preserves_time_precision_and_render_end() {
    let mut model = GanttDiagramRenderModel::default();
    model.sections = vec!["Build".to_string()];
    model.tasks = vec![GanttRenderTask {
        id: "timed".to_string(),
        task: "Timed".to_string(),
        section: "Build".to_string(),
        start_ms: 1,
        end_ms: 2,
        render_end_ms: Some(3),
        ..Default::default()
    }];

    let rendered = render(RenderSemanticModel::Gantt(model));

    assert!(rendered.contains("id=timed"));
    assert!(
        rendered.contains("T"),
        "time-of-day precision must be visible:\n{rendered}"
    );
    assert!(rendered.contains("renderEnd="));
}

#[test]
fn gantt_direct_model_rejects_duplicate_task_ids() {
    let task = GanttRenderTask {
        id: "same".to_string(),
        task: "Task".to_string(),
        section: "Build".to_string(),
        ..Default::default()
    };
    let mut model = GanttDiagramRenderModel::default();
    model.tasks = vec![task.clone(), task];

    let error = render_model(
        &RenderSemanticModel::Gantt(model),
        &AsciiRenderOptions::ascii(),
    )
    .expect_err("duplicate task ids cannot provide stable identity");
    assert!(matches!(
        error,
        AsciiError::UnsupportedFeature {
            diagram_type: "gantt",
            feature: "duplicate task ids",
        }
    ));
}

#[test]
fn journey_render_model_renders_actors_sections_and_scores() {
    let mut model = JourneyDiagramRenderModel::default();
    model.title = Some("Journey".to_string());
    model.acc_title = Some("Journey title".to_string());
    model.acc_descr = Some("Journey description".to_string());
    model.sections = vec!["Discovery".to_string()];
    model.tasks = vec![
        JourneyRenderTask {
            score: 5,
            score_is_nan: false,
            people: vec!["Alice".to_string(), "Bob".to_string()],
            section: "Discovery".to_string(),
            section_index: Some(0),
            task_type: "Discovery".to_string(),
            task: "Research".to_string(),
        },
        JourneyRenderTask {
            score: 3,
            score_is_nan: false,
            people: vec!["Bob".to_string()],
            section: "Discovery".to_string(),
            section_index: Some(0),
            task_type: "Discovery".to_string(),
            task: "Ship".to_string(),
        },
    ];

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
            kanban_node("backlog", "Backlog", true, KanbanNodeMetadata::default()),
            kanban_node(
                "card-a",
                "Ticket A",
                false,
                KanbanNodeMetadata {
                    parent_id: Some("backlog"),
                    ticket: Some("K-1"),
                    priority: Some("high"),
                    assigned: Some("alice"),
                    icon: Some("bug"),
                },
            ),
            kanban_node(
                "card-b",
                "Ticket B",
                false,
                KanbanNodeMetadata {
                    parent_id: Some("backlog"),
                    ticket: Some("K-2"),
                    ..Default::default()
                },
            ),
            kanban_node("doing", "Doing", true, KanbanNodeMetadata::default()),
            kanban_node(
                "card-c",
                "Ticket C",
                false,
                KanbanNodeMetadata {
                    parent_id: Some("doing"),
                    ticket: Some("K-3"),
                    ..Default::default()
                },
            ),
        ],
    };

    let rendered = render(RenderSemanticModel::Kanban(model));

    assert_eq!(
        rendered,
        concat!(
            "Backlog [id=backlog]\n",
            "  - Ticket A [id=card-a, ticket=K-1, priority=high, assigned=alice, icon=bug]\n",
            "  - Ticket B [id=card-b, ticket=K-2]\n",
            "Doing [id=doing]\n",
            "  - Ticket C [id=card-c, ticket=K-3]",
        )
    );
}

#[test]
fn kanban_render_model_keeps_unassigned_and_unknown_parent_cards() {
    let model = KanbanDiagramRenderModel {
        nodes: vec![
            kanban_node("backlog", "Backlog", true, KanbanNodeMetadata::default()),
            kanban_node(
                "known",
                "Known",
                false,
                KanbanNodeMetadata {
                    parent_id: Some("backlog"),
                    ..Default::default()
                },
            ),
            kanban_node("loose", "Loose", false, KanbanNodeMetadata::default()),
            kanban_node(
                "unknown",
                "Unknown",
                false,
                KanbanNodeMetadata {
                    parent_id: Some("missing"),
                    ticket: Some("K-404"),
                    ..Default::default()
                },
            ),
        ],
    };

    let rendered = render(RenderSemanticModel::Kanban(model));

    assert_eq!(
        rendered,
        concat!(
            "Backlog [id=backlog]\n",
            "  - Known [id=known]\n",
            "Unassigned\n",
            "  - Loose [id=loose]\n",
            "  - Unknown [id=unknown, parent=missing, ticket=K-404]",
        )
    );
}

#[test]
fn kanban_render_model_rejects_duplicate_or_empty_ids() {
    let duplicate = KanbanDiagramRenderModel {
        nodes: vec![
            kanban_node("same", "A", true, KanbanNodeMetadata::default()),
            kanban_node("same", "B", false, KanbanNodeMetadata::default()),
        ],
    };
    let error = render_model(
        &RenderSemanticModel::Kanban(duplicate),
        &AsciiRenderOptions::ascii(),
    )
    .expect_err("group/card ids share one namespace");
    assert!(matches!(
        error,
        AsciiError::UnsupportedFeature {
            diagram_type: "kanban",
            feature: "duplicate node ids",
        }
    ));

    let empty = KanbanDiagramRenderModel {
        nodes: vec![kanban_node(
            "",
            "Empty",
            false,
            KanbanNodeMetadata::default(),
        )],
    };
    let error = render_model(
        &RenderSemanticModel::Kanban(empty),
        &AsciiRenderOptions::ascii(),
    )
    .expect_err("empty ids cannot provide stable card identity");
    assert!(matches!(
        error,
        AsciiError::UnsupportedFeature {
            diagram_type: "kanban",
            feature: "empty node ids",
        }
    ));
}

#[test]
fn packet_render_model_renders_rows_and_ranges() {
    let mut model = PacketDiagramRenderModel::default();
    model.title = Some("Packet".to_string());
    model.acc_title = Some("Packet title".to_string());
    model.acc_descr = Some("Packet description".to_string());
    model.packet = vec![
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
    ];

    let rendered = render(RenderSemanticModel::Packet(model));

    assert_eq!(
        rendered,
        concat!(
            "Packet\n",
            "accTitle: Packet title\n",
            "accDescr: Packet description\n",
            "row 1: [0..7] header (8 bits) | [8..15] payload (8 bits)\n",
            "row 2: [16..31] footer (16 bits)",
        )
    );
}

#[test]
fn packet_parser_split_blocks_render_upstream_split_bit_counts() {
    let rendered = render_parsed(
        r#"packet
0-10: "test"
11-90: "multiple"
"#,
    );

    assert_eq!(
        rendered,
        concat!(
            "row 1: [0..10] test (11 bits) | [11..31] multiple (20 bits)\n",
            "row 2: [32..63] multiple (31 bits)\n",
            "row 3: [64..90] multiple (26 bits)",
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
        title: Some("Repository history".to_string()),
        acc_title: Some("Git title".to_string()),
        acc_descr: Some("Git description".to_string()),
        warning_facts: vec![DiagramWarningFact::new(
            GIT_GRAPH_DUPLICATE_COMMIT_WARNING_RULE_ID,
            "duplicate head",
        )],
    };

    let rendered = render(RenderSemanticModel::GitGraph(model));

    assert_eq!(
        rendered,
        concat!(
            "gitGraph direction=TB current=main\n",
            "Repository history\n",
            "accTitle: Git title\n",
            "accDescr: Git description\n",
            "branches: main, feature\n",
            "  - 0 main c0 [highlight] init tags=v1 parents=seed typeOverride=7\n",
            "    idSource=explicit\n",
            "warnings:\n",
            "  - duplicate head",
        )
    );
}
