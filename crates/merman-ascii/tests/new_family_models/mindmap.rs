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
            "Root [1]\n",
            "|-- Child 1 [2]\n",
            "|   \\-- Leaf [4]\n",
            "\\-- Child 2 [3]\n",
            "\nDetails:\n",
            "  [1] id(bytes=4)=\"root\"\n",
            "  [2] id(bytes=6)=\"child1\"\n",
            "  [3] id(bytes=6)=\"child2\"\n",
            "  [4] id(bytes=4)=\"leaf\"",
        )
    );
}

#[test]
fn mindmap_ordinary_parsed_tree_has_only_authored_labels_and_connectors() {
    let rendered = render_parsed("mindmap\n root\n  left\n   leaf\n  right\n");

    assert_eq!(rendered, "root\n|-- left\n|   \\-- leaf\n\\-- right");
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
            "Root [1]\n",
            "\\-- Child [2]\n",
            "\n",
            "A [3]\n",
            "\\-- B [4]\n",
            "    \\-- A [3] (cycle)\n",
            "\nDetails:\n",
            "  [1] id(bytes=4)=\"root\"\n",
            "  [2] id(bytes=5)=\"child\"\n",
            "  [3] id(bytes=1)=\"a\"\n",
            "  [4] id(bytes=1)=\"b\"",
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
fn mindmap_details_preserve_identity_shape_icon_and_custom_section() {
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
            "Root [1]\n",
            "\nDetails:\n",
            "  [1] id(bytes=4)=\"root\" shape(bytes=6)=\"circle\" ",
            "icon(bytes=4)=\"home\" section=2",
        )
    );
}

#[test]
fn mindmap_details_framing_distinguishes_labels_from_authored_ids() {
    let mut embedded_id = mindmap_node("0", "Root [id=x]", 0);
    embedded_id.node_id = "y".to_string();

    let mut explicit_id = mindmap_node("0", "Root", 0);
    explicit_id.node_id = "x] [id=y".to_string();

    let embedded = render(RenderSemanticModel::Mindmap(MindmapDiagramRenderModel {
        nodes: vec![embedded_id],
        edges: Vec::new(),
    }));
    let explicit = render(RenderSemanticModel::Mindmap(MindmapDiagramRenderModel {
        nodes: vec![explicit_id],
        edges: Vec::new(),
    }));

    assert_eq!(
        embedded,
        "\"Root [id=x]\" [1]\n\nDetails:\n  [1] id(bytes=1)=\"y\""
    );
    assert_eq!(
        explicit,
        "Root [1]\n\nDetails:\n  [1] id(bytes=8)=\"x] [id=y\""
    );
    assert_ne!(
        embedded, explicit,
        "authored labels must not be able to forge authored node-id disclosure",
    );
}

#[test]
fn mindmap_tree_quotes_labels_that_would_lose_text_or_forge_annotations() {
    for (label, expected) in [
        ("", "\"\""),
        ("  label  ", "\"  label  \""),
        ("two  spaces", "\"two  spaces\""),
        ("line\nnext", "\"line\\nnext\""),
        (r"line\nnext", r#""line\\nnext""#),
        ("tab\tstop", r#""tab\tstop""#),
        ("Root [1]", "\"Root [1]\""),
        ("Root (cycle)", "\"Root (cycle)\""),
        ("Details:", "\"Details:\""),
        ("|-- fake child", "\"|-- fake child\""),
        ("before\u{202e}after", r#""before\u{202E}after""#),
    ] {
        let mut root = mindmap_node("root", label, 0);
        if !label.is_empty() {
            root.node_id = label.to_string();
        }
        let output = render(RenderSemanticModel::Mindmap(MindmapDiagramRenderModel {
            nodes: vec![root],
            edges: Vec::new(),
        }));
        let tree = output.split("\n\nDetails:").next().unwrap();
        if label.is_empty() {
            assert_eq!(tree, format!("{expected} [1]"), "{output}");
            assert!(output.contains("id(bytes=4)=\"root\""));
        } else {
            assert_eq!(tree, expected, "{output}");
        }
    }
}

#[test]
fn mindmap_wrapped_label_cannot_forge_a_child() {
    let root_label = "x".repeat(80);
    let mut root = mindmap_node("root", &root_label, 0);
    root.node_id = root.label.clone();
    let mut child = mindmap_node("child", "forged", 1);
    child.node_id = child.label.clone();
    let tree = RenderSemanticModel::Mindmap(MindmapDiagramRenderModel {
        nodes: vec![root, child],
        edges: vec![mindmap_edge("edge", "root", "child")],
    });
    let mut forged = mindmap_node("forged", &format!("{root_label} └── forged"), 0);
    forged.node_id = forged.label.clone();
    let single = RenderSemanticModel::Mindmap(MindmapDiagramRenderModel {
        nodes: vec![forged],
        edges: Vec::new(),
    });
    let options = AsciiRenderOptions::unicode();

    let tree = render_with_options(tree, &options);
    let single = render_with_options(single, &options);
    assert_ne!(tree, single, "wrapped label must not impersonate a child");
    assert!(single.starts_with('"'), "{single}");
}

#[test]
fn mindmap_wrapped_labels_preserve_word_boundary_spaces_at_each_depth() {
    for options in [AsciiRenderOptions::ascii(), AsciiRenderOptions::unicode()] {
        for profile in [
            merman_ascii::TerminalWidthProfile::Unicode,
            merman_ascii::TerminalWidthProfile::Cjk,
        ] {
            let options = options.with_terminal_width_profile(profile);
            for (prefix, nested) in [
                ("x".repeat(80), false),
                ("界".repeat(40), false),
                ("x".repeat(76), true),
                ("界".repeat(38), true),
            ] {
                let render_label = |separator: &str| {
                    let mut node = mindmap_node(
                        "label",
                        &format!("{prefix}{separator}word"),
                        i64::from(nested),
                    );
                    node.node_id = node.label.clone();
                    let mut model = MindmapDiagramRenderModel {
                        nodes: Vec::new(),
                        edges: Vec::new(),
                    };
                    if nested {
                        model.nodes.push(mindmap_node("root", "root", 0));
                        model.edges.push(mindmap_edge("edge", "root", "label"));
                    }
                    model.nodes.push(node);
                    render_with_options(RenderSemanticModel::Mindmap(model), &options)
                };
                assert_ne!(
                    render_label(""),
                    render_label(" "),
                    "wrapped label whitespace is semantic: {profile:?}, nested={nested}"
                );
            }
        }
    }
}

#[test]
fn mindmap_keeps_duplicate_labels_attached_to_distinct_identities() {
    let model = MindmapDiagramRenderModel {
        nodes: vec![
            mindmap_node("root", "root", 0),
            mindmap_node("left", "Same", 1),
            mindmap_node("right", "Same", 1),
        ],
        edges: vec![
            mindmap_edge("left-edge", "root", "left"),
            mindmap_edge("right-edge", "root", "right"),
        ],
    };

    let output = render(RenderSemanticModel::Mindmap(model));
    assert_eq!(
        output,
        concat!(
            "root\n|-- Same [2]\n\\-- Same [3]\n",
            "\nDetails:\n",
            "  [2] id(bytes=4)=\"left\"\n",
            "  [3] id(bytes=5)=\"right\"",
        )
    );
}

#[test]
fn mindmap_custom_sections_retain_even_values_matching_other_subtrees() {
    let mut left = mindmap_node("left", "left", 1);
    left.section = Some(0);
    let mut right = mindmap_node("right", "right", 1);
    right.section = Some(0);
    let mut model = MindmapDiagramRenderModel {
        nodes: vec![mindmap_node("root", "root", 0), left, right],
        edges: vec![
            mindmap_edge("left-edge", "root", "left"),
            mindmap_edge("right-edge", "root", "right"),
        ],
    };

    assert_eq!(
        render(RenderSemanticModel::Mindmap(model.clone())),
        concat!(
            "root\n|-- left [2]\n\\-- right [3]\n",
            "\nDetails:\n",
            "  [2] section=0\n",
            "  [3] section=0",
        )
    );

    model.nodes[2].section = Some(1);
    assert_eq!(
        render(RenderSemanticModel::Mindmap(model)),
        "root\n|-- left\n\\-- right",
        "a complete standard palette is already expressed by the tree's branch order"
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
        concat!(
            "Root [1]\n\\-- Child [2]\n",
            "\nDetails:\n",
            "  [1] id(bytes=13)=\"authored-root\"\n",
            "  [2] id(bytes=14)=\"authored-child\"",
        )
    );
}

#[test]
fn mindmap_parser_preserves_distinct_authored_ids_in_details() {
    let rendered = render_parsed("mindmap\nroot[The root]\n  theId(child1)\n    leaf1\n  child2\n");

    let (tree, details) = rendered.split_once("\n\nDetails:").unwrap();
    assert_eq!(
        tree,
        "The root [1]\n|-- child1 [2]\n|   \\-- leaf1\n\\-- child2"
    );
    for authored_id in ["root", "theId"] {
        assert!(
            details.contains(&format!(
                "id(bytes={})=\"{authored_id}\"",
                authored_id.len()
            )),
            "missing authored id {authored_id}:\n{rendered}"
        );
    }
    assert!(details.contains("shape(bytes=4)=\"rect\""));
    assert!(details.contains("shape(bytes=7)=\"rounded\""));
    assert!(!details.contains("section="));
    for internal_id in ["0", "1", "2", "3"] {
        assert!(
            !rendered.contains(&format!("id(bytes=1)=\"{internal_id}\"")),
            "internal id {internal_id} leaked:\n{rendered}"
        );
    }
}
