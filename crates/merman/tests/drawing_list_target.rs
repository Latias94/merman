#![cfg(feature = "svg")]

use merman::{DrawingListRequest, OperationControl, RenderOutput, RenderRequest, Renderer};
use merman_display_list::DrawingListPolicy;

#[test]
fn error_diagram_has_a_complete_renderer_neutral_output() {
    let request = DrawingListRequest {
        policy: DrawingListPolicy::VectorOnly,
        ..DrawingListRequest::default()
    };
    let output = Renderer::new()
        .render(
            RenderRequest::drawing_list("flowchart TD\nA -->", OperationControl::new(), request)
                .with_parse_options(merman::ParseOptions::lenient()),
        )
        .expect("lenient error diagram should render as a DrawingList");
    let RenderOutput::DrawingList(Some(output)) = output else {
        panic!("expected a DrawingList output");
    };

    assert_eq!(
        output.media_type(),
        merman_display_list::DRAWING_LIST_MEDIA_TYPE
    );
    assert_eq!(
        output.document().version,
        merman_display_list::DRAWING_LIST_VERSION
    );
    assert!(!output.document().commands.is_empty());
    assert_eq!(
        output.json(),
        output.document().canonical_json_bytes().unwrap()
    );
}

#[test]
fn flowchart_emits_typed_routes_shapes_and_semantics() {
    let source = r#"flowchart TD
        classDef hot fill:#ffe4e6,stroke:#be123c,stroke-width:2px,color:#881337
        A[Parse]:::hot -->|next| B{Layout}
        B --> C([Done])
    "#;
    let output = Renderer::new()
        .render(RenderRequest::drawing_list(
            source,
            OperationControl::new(),
            DrawingListRequest::default(),
        ))
        .expect("portable Flowchart should render as a DrawingList");
    let RenderOutput::DrawingList(Some(output)) = output else {
        panic!("expected a DrawingList output");
    };

    assert_eq!(output.document().viewport.bounds.width > 0.0, true);
    assert!(
        output
            .document()
            .resources
            .iter()
            .any(|resource| matches!(resource, merman_display_list::DrawingResource::Path(_)))
    );
    assert!(output.document().commands.iter().any(|command| matches!(
        command,
        merman_display_list::DrawingCommand::DrawText { .. }
    )));
    assert!(
        output
            .document()
            .semantics
            .iter()
            .any(|semantic| semantic.role == merman_display_list::SemanticRole::Node)
    );
    assert!(
        output
            .document()
            .semantics
            .iter()
            .any(|semantic| semantic.role == merman_display_list::SemanticRole::Edge)
    );
}

#[test]
fn mindmap_emits_typed_shapes_routes_text_and_semantics() {
    let source = r#"mindmap
        root((Merman))
            Parser
            Renderer[Render]
    "#;
    let output = Renderer::new()
        .render(RenderRequest::drawing_list(
            source,
            OperationControl::new(),
            DrawingListRequest::default(),
        ))
        .expect("portable Mindmap should render as a DrawingList");
    let RenderOutput::DrawingList(Some(output)) = output else {
        panic!("expected a DrawingList output");
    };

    let document = output.document();
    assert!(document.viewport.bounds.width > 0.0);
    assert!(document.resources.iter().any(|resource| matches!(
        resource,
        merman_display_list::DrawingResource::Path(path)
            if path.id.as_str().contains("mindmap.node")
    )));
    assert!(document.commands.iter().any(|command| matches!(
        command,
        merman_display_list::DrawingCommand::DrawText { .. }
    )));
    assert!(document.semantics.iter().any(|semantic| {
        semantic.role == merman_display_list::SemanticRole::Node
            && semantic.id.starts_with("mindmap.node.")
    }));
    assert!(document.semantics.iter().any(|semantic| {
        semantic.role == merman_display_list::SemanticRole::Edge
            && semantic.id.starts_with("mindmap.edge.")
    }));
}

#[test]
fn mindmap_rejects_styled_markdown_instead_of_flattening_it_silently() {
    let error = Renderer::new()
        .render(RenderRequest::drawing_list(
            "mindmap\n  root[**bold root**]\n",
            OperationControl::new(),
            DrawingListRequest::default(),
        ))
        .expect_err("styled Mindmap labels must not be flattened silently");
    assert!(error.to_string().contains("cannot preserve"));
}

#[test]
fn info_emits_typed_version_text_and_semantics() {
    let output = Renderer::new()
        .render(RenderRequest::drawing_list(
            "info",
            OperationControl::new(),
            DrawingListRequest::default(),
        ))
        .expect("Info should render as a DrawingList");
    let RenderOutput::DrawingList(Some(output)) = output else {
        panic!("expected a DrawingList output");
    };

    assert_eq!(output.document().viewport.bounds.width, 400.0);
    assert_eq!(output.document().viewport.bounds.height, 100.0);
    assert!(output.document().commands.iter().any(|command| matches!(
        command,
        merman_display_list::DrawingCommand::DrawText { run }
            if run.text.starts_with('v') && run.origin.x == 100.0 && run.origin.y == 40.0
    )));
    assert!(output.document().semantics.iter().any(|semantic| {
        semantic.role == merman_display_list::SemanticRole::Label && semantic.id == "info.version"
    }));
}

#[test]
fn an_unmigrated_family_fails_without_partial_drawing_list_bytes() {
    let output = Renderer::new()
        .render(RenderRequest::drawing_list(
            "pie\n    \"A\": 1",
            OperationControl::new(),
            DrawingListRequest::default(),
        ))
        .expect_err("unmigrated families must fail closed");
    assert!(
        output
            .to_string()
            .contains("has not been migrated to the canonical document seam")
    );
}
