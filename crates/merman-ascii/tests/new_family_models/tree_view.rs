use super::*;

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
fn tree_view_recursive_emit_observes_operation_cancellation() {
    let mut node = tree_node(32, 31, "leaf", Vec::new());
    for depth in (0..32).rev() {
        node = tree_node(depth, depth - 1, &format!("node-{depth}"), vec![node]);
    }
    let model = TreeViewDiagramRenderModel {
        acc_title: None,
        acc_descr: None,
        title: None,
        root: tree_node(100, -1, "/", vec![node]),
    };

    // Schedule beyond the complete iterative validation pass so this remains an Emit traversal
    // regression rather than a validation-cancellation test.
    let error = render_with_scheduled_cancellation(RenderSemanticModel::TreeView(model), 256);
    assert!(
        matches!(
            error,
            AsciiError::Cancelled(cancelled)
                if cancelled.phase == merman_core::OperationPhase::Emit
        ),
        "expected Emit cancellation, got {error:?}"
    );
}
