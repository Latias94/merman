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
fn an_unmigrated_family_fails_without_partial_drawing_list_bytes() {
    let output = Renderer::new()
        .render(RenderRequest::drawing_list(
            "info",
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
