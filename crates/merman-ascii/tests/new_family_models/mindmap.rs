use super::*;

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
fn mindmap_direct_model_rejects_nodes_with_multiple_parents() {
    let model = MindmapDiagramRenderModel {
        nodes: vec![
            mindmap_node("root", "Root", 0),
            mindmap_node("left", "Left", 1),
            mindmap_node("right", "Right", 1),
            mindmap_node("shared", "Shared", 2),
        ],
        edges: vec![
            mindmap_edge("root-left", "root", "left"),
            mindmap_edge("root-right", "root", "right"),
            mindmap_edge("left-shared", "left", "shared"),
            mindmap_edge("right-shared", "right", "shared"),
        ],
    };

    let error = render_model(
        &RenderSemanticModel::Mindmap(model),
        &AsciiRenderOptions::ascii(),
    )
    .expect_err("a Mindmap child must not be expanded below multiple parents");

    assert!(matches!(
        error,
        AsciiError::UnsupportedFeature {
            diagram_type: "mindmap",
            feature: "nodes with multiple parents",
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
