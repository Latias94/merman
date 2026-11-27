use super::*;

fn metadata() -> crate::SemanticMetadata {
    crate::SemanticMetadata {
        diagram_type: "block".to_string(),
        config: MermaidConfig::empty_object(),
        semantic_config: MermaidConfig::empty_object(),
        title: None,
    }
}

#[test]
fn block_semantics_preserve_records_and_warnings_without_editor_or_palette_work() {
    reset_block_syntax_construction_count();
    #[cfg(feature = "editor-facts")]
    crate::editor::test_work::reset();
    let source = r#"block
columns 1
A["Alpha"]:2
block:outer
turn<["Turn"]>(right,down)
end
classDef hot fill:red
class A hot
A --> turn
"#;
    let metadata = metadata();
    let construction = construct_block_semantic_source(
        source,
        &metadata.semantic_config,
        #[cfg(feature = "editor-facts")]
        EditorFactsDemand::Omit,
        false,
        &OperationControl::new(),
    )
    .unwrap()
    .unwrap_or_else(|failure| panic!("unexpected parse error: {}", failure.error));
    let model = construction
        .parsed
        .into_semantic_model(&OperationControl::new())
        .unwrap();
    let root = model.root().unwrap();
    assert_eq!(root.columns, Some(1.0));
    let alpha = model
        .blocks_flat
        .iter()
        .find(|node| node.id == "A")
        .unwrap();
    assert_eq!(alpha.label, "Alpha");
    assert_eq!(alpha.width_in_columns, Some(2.0));
    assert_eq!(alpha.classes, ["hot"]);
    let turn = model
        .blocks_flat
        .iter()
        .find(|node| node.id == "turn")
        .unwrap();
    assert_eq!(turn.directions, ["right", "down"]);
    assert_eq!(model.edges.len(), 1);
    assert_eq!(model.edges[0].start, "A");
    assert_eq!(model.edges[0].end, "turn");
    assert_eq!(model.warning_facts.len(), 1);
    assert!(model.class_defs.contains_key("hot"));
    assert_eq!(block_syntax_construction_count(), 1);
    #[cfg(feature = "editor-facts")]
    assert_eq!(crate::editor::test_work::read(), Default::default());
    let direct = parse_block_semantic_model_controlled(
        source,
        &metadata,
        #[cfg(feature = "editor-facts")]
        EditorFactsDemand::Omit,
        &OperationControl::new(),
    )
    .unwrap()
    .model
    .unwrap();
    assert_eq!(direct.blocks_flat.len(), model.blocks_flat.len());
}

#[test]
fn block_semantics_preserve_width_admission_spans_and_cancellation() {
    let width = MAX_BLOCK_SPACE_EXPANSION_ITEMS + 1;
    let source = format!(
        "block
space:{width}
"
    );
    #[cfg(feature = "editor-facts")]
    crate::editor::test_work::reset();
    let error = parse_block_semantic_model_controlled(
        &source,
        &metadata(),
        #[cfg(feature = "editor-facts")]
        EditorFactsDemand::Omit,
        &OperationControl::new(),
    )
    .unwrap()
    .model
    .unwrap_err();
    let Error::DiagramParse { diagnostic, .. } = error else {
        panic!("structured error");
    };
    let start = source.find(&width.to_string()).unwrap();
    assert_eq!(
        diagnostic.span(),
        Some(SourceSpan::new(start, start + width.to_string().len()))
    );
    assert_eq!(
        diagnostic.span_kind(),
        crate::ParseDiagnosticSpanKind::Exact
    );
    #[cfg(feature = "editor-facts")]
    assert_eq!(crate::editor::test_work::read(), Default::default());
    let control = OperationControl::new();
    control.cancel_after_checkpoints(2);
    assert!(
        parse_block_semantic_model_controlled(
            "block
A
B
",
            &metadata(),
            #[cfg(feature = "editor-facts")]
            EditorFactsDemand::Omit,
            &control,
        )
        .is_err()
    );
}
