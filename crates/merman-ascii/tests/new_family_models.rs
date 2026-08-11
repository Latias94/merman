use merman_ascii::{AsciiError, AsciiRenderOptions, render_model};
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
            "title(bytes=7)=\"Project\"\n",
            "accTitle(bytes=10)=\"Tree title\"\n",
            "accDescr(bytes=16)=\"Tree description\"\n",
            "[directory] name(bytes=1)=\"/\" [id=0, level=-1]\n",
            "|-- [directory] name(bytes=4)=\"Root\"/ [id=1, level=0]\n",
            "|   |-- [file] name(bytes=7)=\"Child 1\" [id=2, level=1]\n",
            "|   \\-- [file] name(bytes=7)=\"Child 2\" [id=3, level=1]\n",
            "\\-- [file] name(bytes=7)=\"Sibling\" [id=4, level=0]",
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
            "[directory] name(bytes=1)=\"/\" [id=0, level=-1]\n",
            "\\-- [directory] name(bytes=3)=\"src\"/ [id=1, level=0, icon(bytes=6)=\"folder\",\n",
            "    class(bytes=9)=\"highlight\"] description(bytes=16)=\"source directory\"\n",
            "    \\-- [file] name(bytes=7)=\"App.tsx\" [id=2, level=1, icon(bytes=5)=\"react\"]\n",
            "        description(bytes=14)=\"main component\"",
        )
    );
    assert_eq!(
        unicode,
        concat!(
            "[directory] name(bytes=1)=\"/\" [id=0, level=-1]\n",
            "└── [directory] name(bytes=3)=\"src\"/ [id=1, level=0, icon(bytes=6)=\"folder\",\n",
            "    class(bytes=9)=\"highlight\"] description(bytes=16)=\"source directory\"\n",
            "    └── [file] name(bytes=7)=\"App.tsx\" [id=2, level=1, icon(bytes=5)=\"react\"]\n",
            "        description(bytes=14)=\"main component\"",
        )
    );
}

#[test]
fn tree_view_distinguishes_trailing_slash_files_from_directories() {
    let file = TreeViewNodeRenderModel {
        id: 1,
        level: 0,
        name: "src/".to_string(),
        node_type: "file".to_string(),
        ..Default::default()
    };
    let directory = TreeViewNodeRenderModel {
        node_type: "directory".to_string(),
        ..file.clone()
    };

    let file_model = TreeViewDiagramRenderModel {
        root: file,
        ..Default::default()
    };
    let directory_model = TreeViewDiagramRenderModel {
        root: directory,
        ..Default::default()
    };

    let file_rendered = render(RenderSemanticModel::TreeView(file_model));
    let directory_rendered = render(RenderSemanticModel::TreeView(directory_model));

    assert!(file_rendered.starts_with("[file] name(bytes=4)=\"src/\" "));
    assert!(directory_rendered.starts_with("[directory] name(bytes=4)=\"src/\" "));
    assert_ne!(file_rendered, directory_rendered);
}

#[test]
fn tree_view_structured_text_framing_distinguishes_icon_from_class() {
    let mut embedded_class = TreeViewDiagramRenderModel::default();
    embedded_class.root.icon = Some("folder, class=highlight".to_string());

    let mut explicit_class = TreeViewDiagramRenderModel::default();
    explicit_class.root.icon = Some("folder".to_string());
    explicit_class.root.css_class = Some("highlight".to_string());

    assert_ne!(
        render(RenderSemanticModel::TreeView(embedded_class)),
        render(RenderSemanticModel::TreeView(explicit_class)),
        "authored icon text must not be able to forge a CSS class field",
    );
}

#[test]
fn tree_view_rejects_unknown_direct_model_node_types() {
    let mut model = TreeViewDiagramRenderModel::default();
    model.root.node_type = "mystery".to_string();

    let error = render_model(
        &RenderSemanticModel::TreeView(model),
        &AsciiRenderOptions::ascii(),
    )
    .expect_err("unknown TreeView node types must not be projected as authored syntax");
    assert_eq!(
        error,
        AsciiError::UnsupportedFeature {
            diagram_type: "treeView",
            feature: "unknown node types",
        }
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
            "node(bytes=4)=\"Root\" id(bytes=4)=\"root\"\n",
            "|-- node(bytes=7)=\"Child 1\" id(bytes=6)=\"child1\"\n",
            "|   \\-- node(bytes=4)=\"Leaf\" id(bytes=4)=\"leaf\"\n",
            "\\-- node(bytes=7)=\"Child 2\" id(bytes=6)=\"child2\"",
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
            "node(bytes=4)=\"Root\" id(bytes=4)=\"root\"\n",
            "\\-- node(bytes=5)=\"Child\" id(bytes=5)=\"child\"\n",
            "\n",
            "node(bytes=1)=\"A\" id(bytes=1)=\"a\"\n",
            "\\-- node(bytes=1)=\"B\" id(bytes=1)=\"b\"\n",
            "    \\-- node(bytes=1)=\"A\" id(bytes=1)=\"a\" (cycle)",
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

    let mut first = mindmap_node("0", "First", 0);
    first.node_id = "authored".to_string();
    let mut second = mindmap_node("1", "Second", 0);
    second.node_id = "authored".to_string();
    let duplicate_authored = MindmapDiagramRenderModel {
        nodes: vec![first, second],
        edges: Vec::new(),
    };
    let error = render_model(
        &RenderSemanticModel::Mindmap(duplicate_authored),
        &AsciiRenderOptions::ascii(),
    )
    .expect_err("duplicate authored node ids must be rejected");
    assert!(matches!(
        error,
        AsciiError::UnsupportedFeature {
            diagram_type: "mindmap",
            feature: "duplicate authored node ids",
        }
    ));

    let mut missing_authored = mindmap_node("0", "Missing", 0);
    missing_authored.node_id.clear();
    let error = render_model(
        &RenderSemanticModel::Mindmap(MindmapDiagramRenderModel {
            nodes: vec![missing_authored],
            edges: Vec::new(),
        }),
        &AsciiRenderOptions::ascii(),
    )
    .expect_err("missing authored node ids must be rejected");
    assert!(matches!(
        error,
        AsciiError::UnsupportedFeature {
            diagram_type: "mindmap",
            feature: "missing authored node ids",
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
        concat!(
            "node(bytes=4)=\"Root\" id(bytes=4)=\"root\" shape(bytes=6)=\"circle\"\n",
            "icon(bytes=4)=\"home\" section=2",
        )
    );
}

#[test]
fn mindmap_structured_text_framing_distinguishes_labels_from_authored_ids() {
    let mut embedded_id = mindmap_node("0", "Root [id=x]", 0);
    embedded_id.node_id = "y".to_string();

    let mut explicit_id = mindmap_node("0", "Root", 0);
    explicit_id.node_id = "x] [id=y".to_string();

    assert_ne!(
        render(RenderSemanticModel::Mindmap(MindmapDiagramRenderModel {
            nodes: vec![embedded_id],
            edges: Vec::new(),
        })),
        render(RenderSemanticModel::Mindmap(MindmapDiagramRenderModel {
            nodes: vec![explicit_id],
            edges: Vec::new(),
        })),
        "authored labels must not be able to forge authored node-id disclosure",
    );
}

#[test]
fn mindmap_honors_ascii_and_unicode_structural_charsets() {
    let model = MindmapDiagramRenderModel {
        nodes: vec![
            mindmap_node("root", "Root", 0),
            mindmap_node("child", "Child", 1),
        ],
        edges: vec![mindmap_edge("edge", "root", "child")],
    };

    let ascii = render_with_options(
        RenderSemanticModel::Mindmap(model.clone()),
        &AsciiRenderOptions::ascii(),
    );
    let unicode = render_with_options(
        RenderSemanticModel::Mindmap(model),
        &AsciiRenderOptions::unicode(),
    );

    assert!(ascii.contains("\\-- "), "missing ASCII connector:\n{ascii}");
    assert!(
        unicode.contains("└── "),
        "missing Unicode connector:\n{unicode}"
    );
    assert_ne!(ascii, unicode);
}

#[test]
fn mindmap_uses_internal_ids_for_topology_and_authored_ids_for_disclosure() {
    let mut root = mindmap_node("0", "Root", 0);
    root.node_id = "authored-root".to_string();
    let mut child = mindmap_node("1", "Child", 1);
    child.node_id = "authored-child".to_string();
    let model = MindmapDiagramRenderModel {
        nodes: vec![root, child],
        edges: vec![mindmap_edge("edge_0_1", "0", "1")],
    };

    assert_eq!(
        render(RenderSemanticModel::Mindmap(model)),
        "node(bytes=4)=\"Root\" id(bytes=13)=\"authored-root\"\n\\-- node(bytes=5)=\"Child\" id(bytes=14)=\"authored-child\""
    );
}

#[test]
fn mindmap_parser_preserves_authored_ids_in_structured_text() {
    let rendered = render_parsed("mindmap\nroot[The root]\n  theId(child1)\n    leaf1\n  child2\n");

    for authored_id in ["root", "theId", "leaf1", "child2"] {
        assert!(
            rendered.contains(&format!(
                "id(bytes={})=\"{authored_id}\"",
                authored_id.len()
            )),
            "missing authored id {authored_id}:\n{rendered}"
        );
    }
    for internal_id in ["0", "1", "2", "3"] {
        assert!(
            !rendered.contains(&format!("id(bytes=1)=\"{internal_id}\"")),
            "internal id {internal_id} leaked:\n{rendered}"
        );
    }
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
            "title(bytes=8)=\"Timeline\"\n",
            "accTitle(bytes=14)=\"Timeline title\"\n",
            "accDescr(bytes=20)=\"Timeline description\"\n",
            "direction: TD\n",
            "section(bytes=8)=\"Planning\"\n",
            "  - task(bytes=6)=\"Design\"\n",
            "    * event(bytes=7)=\"Kickoff\"\n",
            "  - task(bytes=9)=\"Implement\"\n",
            "    * event(bytes=10)=\"Build spec\"\n",
            "    * event(bytes=6)=\"Review\"",
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
            "section(bytes=8)=\"Planning\"\n",
            "  - task(bytes=107)=\"Design a very long integration event stream normalization w\n",
            "    orkflow that still fits readable terminal output\"\n",
            "    * event(bytes=87)=\"Capture every upstream payload variant without losing the\n",
            "       important operational context\"",
        )
    );
}

#[test]
fn timeline_structured_text_framing_distinguishes_task_text_from_events() {
    let mut embedded_event = TimelineDiagramRenderModel::default();
    embedded_event.tasks = vec![TimelineRenderTask {
        id: 1,
        section: String::new(),
        section_index: None,
        task_type: String::new(),
        task: "Task\n* Event".to_string(),
        score: 0,
        events: Vec::new(),
    }];

    let mut explicit_event = embedded_event.clone();
    explicit_event.tasks[0].task = "Task".to_string();
    explicit_event.tasks[0].events = vec!["Event".to_string()];

    assert_ne!(
        render(RenderSemanticModel::Timeline(embedded_event)),
        render(RenderSemanticModel::Timeline(explicit_event)),
        "authored task text must not be able to forge an event row",
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
            "title(bytes=5)=\"Gantt\"\n",
            "accTitle(bytes=11)=\"Gantt title\"\n",
            "accDescr(bytes=17)=\"Gantt description\"\n",
            "dateFormat(bytes=10)=\"YYYY-MM-DD\"\n",
            "axisFormat(bytes=2)=\"%d\"\n",
            "section(bytes=5)=\"Empty\"\n",
            "section(bytes=5)=\"Empty\"\n",
            "section(bytes=5)=\"Build\"\n",
            "  - task(bytes=9)=\"Implement\" [id(bytes=6)=\"task-1\", order=0,\n",
            "    range=+292278994-08-17T07:12:55.000 -> +292278994-08-17T07:12:55.001,\n",
            "    renderEnd=+292278994-08-17T07:12:55.002, flags=milestone, active, done,\n",
            "    crit, vert]",
        )
    );
}

#[test]
fn gantt_direct_model_discloses_task_order() {
    let mut model = GanttDiagramRenderModel::default();
    model.tasks = vec![
        GanttRenderTask {
            id: "first".to_string(),
            task: "First".to_string(),
            order: 7,
            ..Default::default()
        },
        GanttRenderTask {
            id: "second".to_string(),
            task: "Second".to_string(),
            order: 3,
            ..Default::default()
        },
    ];

    let rendered = render(RenderSemanticModel::Gantt(model.clone()));
    assert!(rendered.contains("task(bytes=5)=\"First\" [id(bytes=5)=\"first\", order=7,"));
    assert!(rendered.contains("task(bytes=6)=\"Second\" [id(bytes=6)=\"second\", order=3,"));

    model.tasks[0].order = 3;
    model.tasks[1].order = 7;
    let reordered = render(RenderSemanticModel::Gantt(model));
    assert_ne!(rendered, reordered, "task order must remain recoverable");
}

#[test]
fn gantt_structured_text_framing_distinguishes_title_from_axis_format() {
    let mut embedded_axis = GanttDiagramRenderModel::default();
    embedded_axis.title = Some("Gantt\naxisFormat: %d".to_string());

    let mut explicit_axis = GanttDiagramRenderModel::default();
    explicit_axis.title = Some("Gantt".to_string());
    explicit_axis.axis_format = "%d".to_string();

    assert_ne!(
        render(RenderSemanticModel::Gantt(embedded_axis)),
        render(RenderSemanticModel::Gantt(explicit_axis)),
        "authored title text must not be able to forge an axisFormat field",
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
        "Release: release, 2026-01-05, 2026-01-06\n",
    ));

    assert!(
        rendered.contains("after=[bytes=6 \"design\", bytes=6 \"review\"]"),
        "structured Gantt output should disclose the dependency source expression:\n{rendered}"
    );
    assert!(rendered.contains("until=[bytes=6 \"design\", bytes=6 \"review\"]"));
    assert!(rendered.contains("start(bytes=10)=\"2026-01-01\""));
    assert!(rendered.contains("duration(bytes=2)=\"1d\""));
    assert!(rendered.contains("end(bytes=10)=\"2026-01-06\""));
    assert!(rendered.contains("id(bytes=6)=\"design\""));
    assert!(rendered.contains("id(bytes=6)=\"review\""));
    assert!(rendered.contains("id(bytes=14)=\"implementation\""));
    assert!(rendered.contains("id(bytes=7)=\"release\""));
}

#[test]
fn gantt_direct_model_renders_only_typed_constraints() {
    let mut model = GanttDiagramRenderModel::default();
    model.sections = vec!["Build".to_string()];
    model.tasks = vec![GanttRenderTask {
        id: "implementation".to_string(),
        task: "Implement".to_string(),
        section: "Build".to_string(),
        start_constraint: GanttTaskStartConstraint::After {
            dependency_ids: vec!["design".to_string(), "review".to_string()],
        },
        end_constraint: GanttTaskEndConstraint::Until {
            dependency_ids: vec!["release".to_string()],
        },
        prev_task_id: Some("legacy-previous".to_string()),
        raw: GanttRenderTaskRaw {
            data: "implementation,after raw-start,until raw-end".to_string(),
            start_time: GanttRenderTaskStart::GetStartDate {
                start_data: "after raw-start".to_string(),
            },
            end_time: GanttRenderTaskEnd {
                data: "until raw-end".to_string(),
            },
        },
        ..Default::default()
    }];

    let rendered = render(RenderSemanticModel::Gantt(model));
    assert!(rendered.contains("after=[bytes=6 \"design\", bytes=6 \"review\"]"));
    assert!(rendered.contains("until=[bytes=7 \"release\"]"));
    for legacy in ["raw-start", "raw-end", "legacy-previous"] {
        assert!(
            !rendered.contains(legacy),
            "ASCII must not recover constraints from legacy raw fields:\n{rendered}"
        );
    }
}

#[test]
fn gantt_direct_model_distinguishes_fixed_and_relative_end_constraints() {
    let mut model = GanttDiagramRenderModel::default();
    model.tasks = vec![
        GanttRenderTask {
            id: "fixed".to_string(),
            task: "Fixed".to_string(),
            start_constraint: GanttTaskStartConstraint::Fixed {
                value: "2026-01-01 08:30".to_string(),
            },
            end_constraint: GanttTaskEndConstraint::Fixed {
                value: "2026-01-01 10:45".to_string(),
            },
            ..Default::default()
        },
        GanttRenderTask {
            id: "relative".to_string(),
            task: "Relative".to_string(),
            start_constraint: GanttTaskStartConstraint::PreviousTaskEnd {
                dependency_id: Some("fixed".to_string()),
            },
            end_constraint: GanttTaskEndConstraint::Duration {
                value: "2.5h".to_string(),
            },
            ..Default::default()
        },
    ];

    let rendered = render(RenderSemanticModel::Gantt(model));
    assert!(rendered.contains("start(bytes=16)=\"2026-01-01 08:30\""));
    assert!(rendered.contains("end(bytes=16)=\"2026-01-01 10:45\""));
    assert!(rendered.contains("after(bytes=5)=\"fixed\""));
    assert!(rendered.contains("duration(bytes=4)=\"2.5h\""));
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

    assert!(rendered.contains("id(bytes=5)=\"timed\""));
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
            "title(bytes=7)=\"Journey\"\n",
            "accTitle(bytes=13)=\"Journey title\"\n",
            "accDescr(bytes=19)=\"Journey description\"\n",
            "actors=[bytes=5 \"Alice\", bytes=3 \"Bob\"]\n",
            "section(bytes=9)=\"Discovery\"\n",
            "  - task(bytes=8)=\"Research\" score=5 people=[bytes=5 \"Alice\", bytes=3 \"Bob\"]\n",
            "  - task(bytes=4)=\"Ship\" score=3 people=[bytes=3 \"Bob\"]",
        )
    );
}

#[test]
fn journey_structured_text_framing_distinguishes_actor_list_items() {
    let mut one_actor = JourneyDiagramRenderModel::default();
    one_actor.actors = vec!["Alice, Bob".to_string()];

    let mut two_actors = JourneyDiagramRenderModel::default();
    two_actors.actors = vec!["Alice".to_string(), "Bob".to_string()];

    assert_ne!(
        render(RenderSemanticModel::Journey(one_actor)),
        render(RenderSemanticModel::Journey(two_actors)),
        "one authored actor containing a comma must differ from two actor values",
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
            "group(bytes=7)=\"Backlog\" [id(bytes=7)=\"backlog\"]\n",
            "  - card(bytes=8)=\"Ticket A\" [id(bytes=6)=\"card-a\", ticket(bytes=3)=\"K-1\",\n",
            "    priority(bytes=4)=\"high\", assigned(bytes=5)=\"alice\", icon(bytes=3)=\"bug\"]\n",
            "  - card(bytes=8)=\"Ticket B\" [id(bytes=6)=\"card-b\", ticket(bytes=3)=\"K-2\"]\n",
            "group(bytes=5)=\"Doing\" [id(bytes=5)=\"doing\"]\n",
            "  - card(bytes=8)=\"Ticket C\" [id(bytes=6)=\"card-c\", ticket(bytes=3)=\"K-3\"]",
        )
    );
}

#[test]
fn kanban_structured_text_framing_distinguishes_ticket_from_priority() {
    let embedded_priority = KanbanDiagramRenderModel {
        nodes: vec![kanban_node(
            "card",
            "Card",
            false,
            KanbanNodeMetadata {
                ticket: Some("K-1, priority=high"),
                ..Default::default()
            },
        )],
    };
    let explicit_priority = KanbanDiagramRenderModel {
        nodes: vec![kanban_node(
            "card",
            "Card",
            false,
            KanbanNodeMetadata {
                ticket: Some("K-1"),
                priority: Some("high"),
                ..Default::default()
            },
        )],
    };

    assert_ne!(
        render(RenderSemanticModel::Kanban(embedded_priority)),
        render(RenderSemanticModel::Kanban(explicit_priority)),
        "authored ticket text must not be able to forge a priority field",
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
            "group(bytes=7)=\"Backlog\" [id(bytes=7)=\"backlog\"]\n",
            "  - card(bytes=5)=\"Known\" [id(bytes=5)=\"known\"]\n",
            "Unassigned\n",
            "  - card(bytes=5)=\"Loose\" [id(bytes=5)=\"loose\"]\n",
            "  - card(bytes=7)=\"Unknown\" [id(bytes=7)=\"unknown\", parent(bytes=7)=\"missing\",\n",
            "    ticket(bytes=5)=\"K-404\"]",
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
fn kanban_parser_projection_keeps_group_metadata() {
    let rendered =
        render_parsed("kanban\n  root@{ priority: high, assigned: alice, icon: star }\n");

    assert_eq!(
        rendered,
        concat!(
            "group(bytes=4)=\"root\" [id(bytes=4)=\"root\", priority(bytes=4)=\"high\",\n",
            "assigned(bytes=5)=\"alice\", icon(bytes=4)=\"star\"]",
        )
    );
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
            "title(bytes=6)=\"Packet\"\n",
            "accTitle(bytes=12)=\"Packet title\"\n",
            "accDescr(bytes=18)=\"Packet description\"\n",
            "row 1:\n",
            "  - range=[0..7] bits=8 label(bytes=6)=\"header\"\n",
            "  - range=[8..15] bits=8 label(bytes=7)=\"payload\"\n",
            "row 2:\n",
            "  - range=[16..31] bits=16 label(bytes=6)=\"footer\"",
        )
    );
}

#[test]
fn packet_parser_split_blocks_render_inclusive_bit_counts() {
    let rendered = render_parsed(
        r#"packet
0-10: "test"
11-90: "multiple"
"#,
    );

    assert_eq!(
        rendered,
        concat!(
            "row 1:\n",
            "  - range=[0..10] bits=11 label(bytes=4)=\"test\"\n",
            "  - range=[11..31] bits=21 label(bytes=8)=\"multiple\"\n",
            "row 2:\n",
            "  - range=[32..63] bits=32 label(bytes=8)=\"multiple\"\n",
            "row 3:\n",
            "  - range=[64..90] bits=27 label(bytes=8)=\"multiple\"",
        )
    );
}

#[test]
fn packet_render_model_rejects_noninclusive_bit_counts() {
    let mut model = PacketDiagramRenderModel::default();
    model.packet = vec![vec![PacketRenderBlock {
        start: 11,
        end: 31,
        bits: 20,
        label: "multiple".to_string(),
    }]];

    let error = render_model(
        &RenderSemanticModel::Packet(model),
        &AsciiRenderOptions::ascii(),
    )
    .expect_err("packet range width must be validated before rendering");
    assert_eq!(
        error,
        AsciiError::UnsupportedFeature {
            diagram_type: "packet",
            feature: "packet block bit count does not match inclusive range",
        }
    );
}

#[test]
fn packet_labels_cannot_forge_following_block_boundaries() {
    let mut forged = PacketDiagramRenderModel::default();
    forged.packet = vec![vec![PacketRenderBlock {
        start: 0,
        end: 7,
        bits: 8,
        label: "header (8 bits) | [8..15] payload".to_string(),
    }]];
    let mut split = PacketDiagramRenderModel::default();
    split.packet = vec![vec![
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
    ]];

    assert_ne!(
        render(RenderSemanticModel::Packet(forged)),
        render(RenderSemanticModel::Packet(split)),
        "length-framed packet labels must distinguish one authored block from two blocks"
    );

    let mut leading = PacketDiagramRenderModel::default();
    leading.packet = vec![vec![PacketRenderBlock {
        start: 0,
        end: 7,
        bits: 8,
        label: " label".to_string(),
    }]];
    let mut trailing = PacketDiagramRenderModel::default();
    trailing.packet = vec![vec![PacketRenderBlock {
        start: 0,
        end: 7,
        bits: 8,
        label: "label ".to_string(),
    }]];
    assert_ne!(
        render(RenderSemanticModel::Packet(leading)),
        render(RenderSemanticModel::Packet(trailing)),
        "equal-length whitespace variants must remain distinguishable after wrapping"
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
            "gitGraph direction(bytes=2)=\"TB\" current(bytes=4)=\"main\"\n",
            "title(bytes=18)=\"Repository history\"\n",
            "accTitle(bytes=9)=\"Git title\"\n",
            "accDescr(bytes=15)=\"Git description\"\n",
            "branches=[bytes=4 \"main\", bytes=7 \"feature\"]\n",
            "  - seq=0 branch(bytes=4)=\"main\" id(bytes=2)=\"c0\" kind=highlight message(bytes=4)=\"init\" tags=[bytes=2 \"v1\"] parents=[bytes=4 \"seed\"] typeOverride=7 idSource=explicit\n",
            "warnings:\n",
            "  - message(bytes=14)=\"duplicate head\"",
        )
    );
}

#[test]
fn git_graph_commit_message_and_metadata_are_framed_without_collisions() {
    let base = GitGraphRenderModel {
        diagram_type: "gitGraph".to_string(),
        commits: Vec::new(),
        branches: Vec::new(),
        current_branch: String::new(),
        direction: "TB".to_string(),
        title: None,
        acc_title: None,
        acc_descr: None,
        warning_facts: Vec::new(),
    };
    let mut message = base.clone();
    message.commits.push(GitGraphCommitRenderModel {
        id: "c0".to_string(),
        message: "init tags=[v1]".to_string(),
        seq: 0,
        commit_type: 0,
        tags: Vec::new(),
        parents: Vec::new(),
        branch: "main".to_string(),
        custom_type: None,
        custom_id: None,
    });
    let mut metadata = base;
    metadata.commits.push(GitGraphCommitRenderModel {
        id: "c0".to_string(),
        message: "init".to_string(),
        seq: 0,
        commit_type: 0,
        tags: vec!["v1".to_string()],
        parents: Vec::new(),
        branch: "main".to_string(),
        custom_type: None,
        custom_id: None,
    });

    let message_output = render(RenderSemanticModel::GitGraph(message));
    let metadata_output = render(RenderSemanticModel::GitGraph(metadata));
    assert_ne!(message_output, metadata_output);
    assert!(
        message_output.contains("message(bytes=14)=")
            && message_output.contains("init")
            && message_output.contains("tags=[v1]")
    );
    assert!(
        metadata_output.contains("message(bytes=4)=\"init\"")
            && metadata_output.contains("tags=[bytes=2 \"v1\"]")
    );

    let mut comma_message = GitGraphRenderModel {
        diagram_type: "gitGraph".to_string(),
        commits: vec![GitGraphCommitRenderModel {
            id: "c0".to_string(),
            message: "x".to_string(),
            seq: 0,
            commit_type: 0,
            tags: vec!["a, b".to_string()],
            parents: Vec::new(),
            branch: "main".to_string(),
            custom_type: None,
            custom_id: None,
        }],
        branches: Vec::new(),
        current_branch: String::new(),
        direction: "TB".to_string(),
        title: None,
        acc_title: None,
        acc_descr: None,
        warning_facts: Vec::new(),
    };
    let mut comma_tags = comma_message.clone();
    comma_message.commits[0].tags.clear();
    comma_message.commits[0].message = "x tags=[a, b]".to_string();
    comma_tags.commits[0].tags = vec!["a".to_string(), "b".to_string()];
    assert_ne!(
        render(RenderSemanticModel::GitGraph(comma_message)),
        render(RenderSemanticModel::GitGraph(comma_tags.clone())),
        "length-framed fields must distinguish embedded delimiters from list structure"
    );

    let mut leading = comma_tags.clone();
    leading.commits[0].message = " init".to_string();
    leading.commits[0].tags.clear();
    let mut trailing = comma_tags;
    trailing.commits[0].message = "init ".to_string();
    trailing.commits[0].tags.clear();
    assert_ne!(
        render(RenderSemanticModel::GitGraph(leading)),
        render(RenderSemanticModel::GitGraph(trailing)),
        "quoted fields must preserve equal-length leading and trailing whitespace"
    );
}

#[test]
fn git_graph_branch_and_commit_identity_fields_are_length_framed() {
    let base = GitGraphRenderModel {
        diagram_type: "gitGraph".to_string(),
        commits: Vec::new(),
        branches: Vec::new(),
        current_branch: String::new(),
        direction: "TB".to_string(),
        title: None,
        acc_title: None,
        acc_descr: None,
        warning_facts: Vec::new(),
    };
    let mut joined_branches = base.clone();
    joined_branches.branches = vec![GitGraphBranchRenderModel {
        name: "main, feature".to_string(),
    }];
    let mut split_branches = base.clone();
    split_branches.branches = vec![
        GitGraphBranchRenderModel {
            name: "main".to_string(),
        },
        GitGraphBranchRenderModel {
            name: "feature".to_string(),
        },
    ];
    assert_ne!(
        render(RenderSemanticModel::GitGraph(joined_branches)),
        render(RenderSemanticModel::GitGraph(split_branches)),
        "branch-list delimiters must not be forgeable by authored branch names"
    );

    let mut joined_identity = base.clone();
    joined_identity.commits.push(GitGraphCommitRenderModel {
        id: "c0 x".to_string(),
        message: String::new(),
        seq: 0,
        commit_type: 0,
        tags: Vec::new(),
        parents: Vec::new(),
        branch: "main".to_string(),
        custom_type: None,
        custom_id: None,
    });
    let mut split_identity = base;
    split_identity.commits.push(GitGraphCommitRenderModel {
        id: "x".to_string(),
        message: String::new(),
        seq: 0,
        commit_type: 0,
        tags: Vec::new(),
        parents: Vec::new(),
        branch: "main c0".to_string(),
        custom_type: None,
        custom_id: None,
    });
    assert_ne!(
        render(RenderSemanticModel::GitGraph(joined_identity)),
        render(RenderSemanticModel::GitGraph(split_identity)),
        "commit branch and id ownership must remain distinguishable"
    );
}
