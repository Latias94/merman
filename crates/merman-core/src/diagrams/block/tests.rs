use super::*;
#[cfg(feature = "editor-facts")]
use crate::EditorSemanticCompleteness;
use crate::{MermaidEngine, ParseDiagnosticSpanKind, ParseOptions, RenderSemanticModel};
use futures::executor::block_on;

fn parse(text: &str) -> crate::ManagedSemanticJson {
    let engine = MermaidEngine::new();
    block_on(engine.parse_diagram(text, ParseOptions::default()))
        .unwrap()
        .unwrap()
        .model
}

#[test]
fn block_ordinary_projections_do_not_collect_editor_facts() {
    let source = "block\nA\n";
    reset_block_syntax_construction_count();
    #[cfg(feature = "editor-facts")]
    crate::editor::test_work::reset();
    parse_block_model_for_render_controlled(source, &meta(), &crate::OperationControl::new())
        .unwrap()
        .unwrap();
    assert_eq!(block_syntax_construction_count(), 1);
    #[cfg(feature = "editor-facts")]
    assert_eq!(crate::editor::test_work::read(), Default::default());
    reset_block_syntax_construction_count();
    #[cfg(feature = "editor-facts")]
    crate::editor::test_work::reset();
    let projected = crate::ManagedSemanticJson::from(parse_block(source, &meta()).unwrap());
    assert!(projected.as_value().is_object());
    assert_eq!(block_syntax_construction_count(), 1);
    #[cfg(feature = "editor-facts")]
    assert_eq!(crate::editor::test_work::read(), Default::default());
}

#[test]
fn block_composite_color_indices_follow_first_insertion_preorder() {
    let model = parse("block-beta\nblock:outer\nblock:inner\nA\nend\nend\nblock:sibling\nB\nend\n");
    let blocks = model["blocksFlat"].as_array().unwrap();
    for (id, index) in [("outer", 0), ("inner", 1), ("sibling", 2)] {
        let block = blocks.iter().find(|block| block["id"] == id).unwrap();
        assert_eq!(block["colorIndex"], json!(index));
    }
    for id in ["root", "A", "B"] {
        let block = blocks.iter().find(|block| block["id"] == id).unwrap();
        assert!(block.get("colorIndex").is_none());
    }
    assert_eq!(model["blocks"][0]["colorIndex"], json!(0));
    assert_eq!(model["blocks"][0]["children"][0]["colorIndex"], json!(1));
}

fn meta() -> SourcePresentationMetadata {
    SourcePresentationMetadata {
        diagram_type: "block".to_string(),
        config: MermaidConfig::default(),
        effective_config: crate::presentation::SourcePresentationConfig::default(),
        title: None,
    }
}

#[test]
fn block_space_width_is_bounded_before_materialization() {
    let width = MAX_BLOCK_SPACE_EXPANSION_ITEMS + 1;
    for text in [
        format!("block\nspace:{width}\n"),
        format!("block\nA space:{width} B\n"),
    ] {
        let error = parse_block(&text, &meta()).expect_err("oversized space must be rejected");
        let Error::DiagramParse { diagnostic, .. } = error else {
            panic!("expected structured Block parse error");
        };
        let start = text.find(&width.to_string()).expect("width in fixture");
        assert_eq!(
            diagnostic.span(),
            Some(SourceSpan::new(start, start + width.to_string().len()))
        );
        assert!(
            diagnostic
                .message()
                .contains("exceeds the materialization limit")
        );
    }
}

fn deep_block_chain(depth: usize) -> String {
    let mut input = String::from("block\n");
    for level in 0..depth {
        input.push_str(&format!("block:n{level}[\"n{level}\"]\n"));
    }
    input.push_str("leaf[\"leaf\"]\n");
    for _ in 0..depth {
        input.push_str("end\n");
    }
    input
}

#[test]
fn block_conversion_observes_control_after_partial_record_transfer() {
    let input = deep_block_chain(500);
    let source = construct_block_semantic_source(
        &input,
        meta().effective_config.as_config(),
        #[cfg(feature = "editor-facts")]
        EditorFactsDemand::Omit,
        true,
        &OperationControl::new(),
    )
    .unwrap()
    .unwrap_or_else(|failure| panic!("unexpected parse error: {}", failure.error));
    let control = OperationControl::new().for_phase(crate::OperationPhase::Semantic);
    control.cancel_after_checkpoints(1);
    let cancelled = source.parsed.into_render_model(&control).unwrap_err();
    assert_eq!(cancelled.phase, crate::OperationPhase::Semantic);
    assert!(parse_block_model_for_render("block\nA\n", &meta()).is_ok());
}

#[test]
fn block_cancellation_releases_managed_partial_projection() {
    let model = parse_block_model_for_render(&deep_block_chain(500), &meta()).unwrap();
    let control = OperationControl::new().for_phase(crate::OperationPhase::Export);
    // The first subtree has finished its leaf and assembled deep ancestors at checkpoint seven.
    control.cancel_after_checkpoints(6);
    let cancelled = render_model_to_compat_json_controlled(&model, &meta(), &control).unwrap_err();
    assert_eq!(cancelled.phase, crate::OperationPhase::Export);
    assert!(parse_block_model_for_render("block\nA\n", &meta()).is_ok());
}

#[test]
fn block_compatibility_projection_handles_deep_nested_wire() {
    let supported = parse_block_model_for_render(&deep_block_chain(61), &meta()).unwrap();
    let compatibility =
        ManagedSemanticJson::from(render_model_to_compat_json(&supported, &meta()).unwrap());
    let mut bytes = Vec::new();
    compatibility.write_json(&mut bytes).unwrap();
    assert!(!bytes.is_empty());

    let deep = parse_block_model_for_render(&deep_block_chain(62), &meta()).unwrap();
    let compatibility =
        ManagedSemanticJson::from(render_model_to_compat_json(&deep, &meta()).unwrap());
    let mut bytes = Vec::new();
    compatibility.write_json(&mut bytes).unwrap();
    assert!(!bytes.is_empty());
}

#[test]
fn block_canonical_growth_and_legacy_output_cost_are_separate() {
    let mut measurements = Vec::new();
    for depth in [16, 32, 64] {
        let source = deep_block_chain(depth);
        let model = parse_block_model_for_render(&source, &meta()).unwrap();
        let records = model.blocks_flat.len();
        let child_ids = model
            .blocks_flat
            .iter()
            .map(|block| block.children.len())
            .sum::<usize>();
        assert_eq!(records, depth + 2);
        assert_eq!(child_ids, depth + 1);
        let compatibility =
            ManagedSemanticJson::from(render_model_to_compat_json(&model, &meta()).unwrap());
        let mut bytes = Vec::new();
        compatibility.write_json(&mut bytes).unwrap();
        println!(
            "depth={depth} source_bytes={} records={records} child_ids={child_ids} legacy_bytes={}",
            source.len(),
            bytes.len()
        );
        measurements.push(bytes.len());
    }
    assert!(measurements[2] > 3 * measurements[1]);
}

fn blocks(model: &Value) -> Vec<Value> {
    model["blocks"].as_array().cloned().unwrap_or_default()
}

fn edges(model: &Value) -> Vec<Value> {
    model["edges"].as_array().cloned().unwrap_or_default()
}

fn columns_for_id(model: &Value, id: &str) -> Option<i64> {
    for b in model["blocksFlat"].as_array()? {
        if b["id"].as_str()? == id {
            return b.get("columns").and_then(|v| v.as_i64());
        }
    }
    None
}

#[test]
fn block_diagram_with_node() {
    let model = parse("block-beta\n  id\n");
    let blocks = blocks(&model);
    assert_eq!(blocks.len(), 1);
    assert_eq!(blocks[0]["id"].as_str().unwrap(), "id");
    assert_eq!(blocks[0]["label"].as_str().unwrap(), "id");
}

#[test]
fn node_with_square_shape_and_label() {
    let model = parse("block\n  id[\"A label\"]\n");
    let blocks = blocks(&model);
    assert_eq!(blocks.len(), 1);
    assert_eq!(blocks[0]["id"].as_str().unwrap(), "id");
    assert_eq!(blocks[0]["label"].as_str().unwrap(), "A label");
    assert_eq!(blocks[0]["type"].as_str().unwrap(), "square");
}

#[test]
fn multiple_nodes() {
    let model = parse("block\n  id1\n  id2\n  id3\n");
    let blocks = blocks(&model);
    assert_eq!(blocks.len(), 3);
    assert_eq!(blocks[0]["id"].as_str().unwrap(), "id1");
    assert_eq!(blocks[1]["id"].as_str().unwrap(), "id2");
    assert_eq!(blocks[2]["id"].as_str().unwrap(), "id3");
}

#[test]
fn nodes_with_edge_basic() {
    let model = parse("block\n  id1[\"first\"]  -->   id2[\"second\"]\n");
    let blocks = blocks(&model);
    let edges = edges(&model);
    assert_eq!(blocks.len(), 2);
    assert_eq!(edges.len(), 1);
    assert_eq!(edges[0]["start"].as_str().unwrap(), "id1");
    assert_eq!(edges[0]["end"].as_str().unwrap(), "id2");
    assert_eq!(edges[0]["arrowTypeEnd"].as_str().unwrap(), "arrow_point");
}

#[test]
fn block_compact_links_preserve_both_markers_and_line_semantics() {
    for (token, start, end, thickness, pattern) in [
        ("-->", "arrow_open", "arrow_point", "normal", "solid"),
        ("<-->", "arrow_point", "arrow_point", "normal", "solid"),
        ("<==>", "arrow_point", "arrow_point", "thick", "solid"),
        ("==>", "arrow_open", "arrow_point", "thick", "solid"),
        ("-.->", "arrow_open", "arrow_point", "normal", "dotted"),
        ("x--o", "arrow_cross", "arrow_circle", "normal", "solid"),
        ("===", "arrow_open", "", "thick", "solid"),
        ("-.-", "arrow_open", "", "normal", "dotted"),
    ] {
        let separator = if token.starts_with(['x', 'o']) {
            " "
        } else {
            ""
        };
        let model = parse(&format!("block\nA{separator}{token}B\n"));
        let edge = &model["edges"][0];
        assert_eq!(edge["start"], "A", "{token}");
        assert_eq!(edge["end"], "B", "{token}");
        assert_eq!(edge["arrowTypeStart"], start, "{token}");
        assert_eq!(edge["arrowTypeEnd"], end, "{token}");
        assert_eq!(edge["thickness"], thickness, "{token}");
        assert_eq!(edge["pattern"], pattern, "{token}");
    }
}

#[test]
fn block_spaced_class_ids_rebind_without_mutating_attached_objects() {
    let value = parse("block\nA[\"Alpha\"] B[\"Beta\"]\nclass A, B hot\n");
    assert_eq!(value["blocks"][0]["classes"], json!(["hot"]));
    assert_eq!(value["blocks"][1]["classes"], Value::Null);
    assert_eq!(value["blocks"][1]["label"], "Beta");
    let binding = value["blocksFlat"]
        .as_array()
        .unwrap()
        .iter()
        .find(|node| node["id"] == "B")
        .unwrap();
    assert_eq!(binding["classes"], json!(["hot"]));
    assert_eq!(binding["type"], "na");
    let control = parse("block\nA B\nclass A,B hot\n");
    assert_eq!(control["blocks"][0]["classes"], json!(["hot"]));
    assert_eq!(control["blocks"][1]["classes"], json!(["hot"]));
}

#[test]
fn block_render_model_uses_typed_variant_without_changing_json_parse() {
    let engine = MermaidEngine::new();
    let input = "block-beta\n  A[\"first\"] --> B[\"second\"]\n";

    let parsed = engine
        .parse_diagram_for_render_model_sync(input, ParseOptions::strict())
        .unwrap()
        .unwrap();

    assert_eq!(parsed.metadata().diagram_type, "block");
    match parsed.model() {
        RenderSemanticModel::Block(model) => {
            let a = model
                .blocks_flat
                .iter()
                .find(|block| block.id == "A")
                .unwrap();
            assert_eq!(a.label, "first");
            assert_eq!(model.edges.len(), 1);
            assert_eq!(model.edges[0].start, "A");
            assert_eq!(model.edges[0].end, "B");
            assert_eq!(
                model.edges[0].arrow_type_end.as_deref(),
                Some("arrow_point")
            );
        }
        other => panic!("block render parse should return typed model, got {other:?}"),
    }

    let parsed_json = engine
        .parse_diagram_sync(input, ParseOptions::strict())
        .unwrap()
        .unwrap();
    assert_eq!(parsed_json.model["type"], json!("block"));
    assert_eq!(parsed_json.model["blocks"][0]["id"], json!("A"));
    assert_eq!(parsed_json.model["edges"][0]["start"], json!("A"));
    assert!(parsed_json.model.get("config").is_some());
}

#[test]
#[cfg(feature = "editor-facts")]
fn block_combined_parse_constructs_once_and_preserves_projections() {
    let text = r#"block-beta
columns 2
A["Alpha"] --"calls"--> B["Beta"]
classDef important fill:#f96,stroke:#333
class A,B important
style B stroke-width:3px
C<["Route"]>(left,down)
"#;
    let meta = meta();

    reset_block_syntax_construction_count();
    let (combined_json, combined_facts) = crate::family::test_support::into_projected_result(
        parse_block_json_and_editor_facts(text, &meta, &OperationControl::new()),
        &meta,
    )
    .unwrap();
    assert_eq!(
        block_syntax_construction_count(),
        1,
        "one combined request must construct Block syntax once"
    );

    assert_eq!(combined_json, parse_block(text, &meta).unwrap());
    assert!(!combined_facts.symbols.is_empty());
    let typed = parse_block_model_for_render(text, &meta).unwrap();
    assert_eq!(
        render_model_to_compat_json(&typed, &meta).unwrap(),
        combined_json
    );
    assert_eq!(combined_json["type"], json!("block"));
    assert!(combined_json["config"].is_object());
    assert!(combined_json["warningFacts"].is_array());
    assert!(combined_json["warnings"].is_array());
}

#[test]
fn block_typed_and_json_projections_preserve_semantic_order() {
    let text = "block\ncolumns 1\nA[\"Alpha\"] --> B[\"Beta\"]\n";
    let meta = meta();
    let compat = parse_block(text, &meta).unwrap();
    let typed = parse_block_model_for_render(text, &meta).unwrap();

    let compat_ids = compat["blocksFlat"]
        .as_array()
        .unwrap()
        .iter()
        .filter_map(|block| block["id"].as_str())
        .collect::<Vec<_>>();
    let typed_ids = typed
        .blocks_flat
        .iter()
        .map(|block| block.id.as_str())
        .collect::<Vec<_>>();
    assert_eq!(compat_ids, typed_ids);
    assert_eq!(compat["warningFacts"], json!(typed.warning_facts));
    assert_eq!(compat["edges"].as_array().unwrap().len(), typed.edges.len());
    assert_eq!(compat["edges"][0]["start"], json!(typed.edges[0].start));
    assert_eq!(compat["edges"][0]["end"], json!(typed.edges[0].end));
}

#[test]
fn block_completed_parent_observes_source_backed_live_metadata_updates() {
    // Mermaid 12.1.0 blockDB registers and attaches the same first-declaration object.
    // Later declarations merge only type/label; style/class actions update that object.
    let source = "block\nblock:G\nA<[\"old\"]>(down):1\nend\nstyle A fill:red,stroke:blue\nclass A hot\nA<[\"middle\"]>(up):3\nA((\"new\")):4\n";
    let typed = parse_block_model_for_render(source, &meta()).unwrap();
    let value = ManagedSemanticJson::from(render_model_to_compat_json(&typed, &meta()).unwrap());
    let nested = &value["blocks"][0]["children"][0];
    let flat = value["blocksFlat"]
        .as_array()
        .unwrap()
        .iter()
        .find(|block| block["id"] == "A")
        .unwrap();
    assert_eq!(nested, flat);
    assert_eq!(nested["label"], json!("new"));
    assert_eq!(nested["type"], json!("circle"));
    assert_eq!(nested["classes"], json!(["hot"]));
    assert_eq!(nested["styles"], json!(["fill:red", "stroke:blue"]));
    assert_eq!(nested["widthInColumns"], json!(1));
    assert_eq!(nested["directions"], json!(["down"]));
    let group = typed
        .blocks_flat
        .iter()
        .find(|block| block.id == "G")
        .unwrap();
    let child = typed.block(group.children[0]).unwrap();
    assert_eq!(child.label, "new");
    assert_eq!(child.block_type, "circle");
    assert_eq!(child.width_in_columns, Some(1.0));
    assert_eq!(child.directions, ["down"]);
}

#[test]
fn block_repeated_composite_preserves_first_children_and_columns() {
    // blockDB replays a repeated group's document into the later declaration object,
    // rather than replacing the first object's hierarchy in the canonical database.
    let source = "block\nblock:G\ncolumns 2\nA\nend\nblock:G\ncolumns 3\nB\nend\n";
    let typed = parse_block_model_for_render(source, &meta()).unwrap();
    let value = ManagedSemanticJson::from(render_model_to_compat_json(&typed, &meta()).unwrap());
    let ids = typed
        .blocks_flat
        .iter()
        .map(|block| block.id.as_str())
        .collect::<Vec<_>>();
    assert_eq!(ids, ["root", "G", "A", "B"]);
    let group = typed.block(typed.root().unwrap().children[0]).unwrap();
    assert_eq!(group.id, "G");
    assert_eq!(group.columns, Some(2.0));
    assert_eq!(group.children.len(), 1);
    assert_eq!(typed.block(group.children[0]).unwrap().id, "A");
    assert_eq!(value["blocks"].as_array().unwrap().len(), 1);
    assert_eq!(value["blocks"][0]["columns"], json!(2));
    assert_eq!(value["blocks"][0]["children"][0]["id"], json!("A"));
    assert!(
        value["blocksFlat"]
            .as_array()
            .unwrap()
            .iter()
            .any(|block| block["id"] == "B")
    );
}

#[test]
#[cfg(feature = "editor-facts")]
fn block_editor_projection_uses_parser_token_spans() {
    let text = r#"block
A["Alpha"] --"calls"--> B["Beta"]
classDef important fill:red
class A,B important
C<["Route"]>(left,down)
"#;
    let facts =
        crate::family::test_support::editor_facts(parse_block_json_and_editor_facts, text, &meta());

    for (name, detail) in [
        ("Alpha", "block label"),
        ("calls", "block edge label"),
        ("important", "block class definition"),
        ("left", "block arrow direction"),
        ("down", "block arrow direction"),
    ] {
        let start = text.find(name).unwrap();
        let symbol = facts
            .symbols
            .iter()
            .find(|symbol| symbol.name == name && symbol.detail.as_deref() == Some(detail))
            .unwrap_or_else(|| panic!("missing {detail} fact for {name}"));
        assert_eq!(symbol.span, SourceSpan::new(start, start + name.len()));
        assert_eq!(symbol.selection, symbol.span);
    }

    let class_definition = facts
        .symbols
        .iter()
        .find(|symbol| {
            symbol.name == "important" && symbol.detail.as_deref() == Some("block class definition")
        })
        .expect("block class definition fact");
    assert_eq!(
        class_definition.role,
        crate::EditorSemanticRole::ClassDefinition
    );

    for target in ["A", "B"] {
        let class_target = facts
            .symbols
            .iter()
            .find(|symbol| {
                symbol.name == target && symbol.detail.as_deref() == Some("block class target")
            })
            .unwrap_or_else(|| panic!("missing block class target {target}"));
        assert_eq!(class_target.role, crate::EditorSemanticRole::Reference);
    }

    let class_ids_start = text.find("A,B important").unwrap();
    assert!(facts.expected_syntax.iter().any(|expected| {
        expected.kind == EditorExpectedSyntaxKind::IdList
            && expected.span == SourceSpan::new(class_ids_start, class_ids_start + 3)
    }));
}

#[test]
#[cfg(feature = "editor-facts")]
fn block_style_targets_are_references_without_creating_definitions() {
    let text = "block\nstyle Future fill:#f00\nFuture[\"Defined later\"]\n";
    let facts =
        crate::family::test_support::editor_facts(parse_block_json_and_editor_facts, text, &meta());

    let future: Vec<_> = facts
        .symbols
        .iter()
        .filter(|symbol| symbol.name == "Future")
        .collect();
    assert_eq!(future.len(), 2);
    assert_eq!(future[0].role, crate::EditorSemanticRole::Reference);
    assert_eq!(future[1].role, crate::EditorSemanticRole::Entity);
}

#[test]
#[cfg(feature = "editor-facts")]
fn block_node_occurrences_only_define_the_first_implicit_block() {
    let text = "block\nA --> B\nA --> C\n";
    let facts =
        crate::family::test_support::editor_facts(parse_block_json_and_editor_facts, text, &meta());

    let a_roles: Vec<_> = facts
        .symbols
        .iter()
        .filter(|symbol| symbol.name == "A")
        .map(|symbol| symbol.role)
        .collect();
    assert_eq!(
        a_roles,
        [
            crate::EditorSemanticRole::Entity,
            crate::EditorSemanticRole::Reference,
        ]
    );
    for name in ["B", "C"] {
        assert!(facts.symbols.iter().any(|symbol| {
            symbol.name == name && symbol.role == crate::EditorSemanticRole::Entity
        }));
    }

    let explicit = "block\nA --> B\nA[\"Defined later\"]\n";
    let explicit_facts = crate::family::test_support::editor_facts(
        parse_block_json_and_editor_facts,
        explicit,
        &meta(),
    );
    let explicit_a_roles: Vec<_> = explicit_facts
        .symbols
        .iter()
        .filter(|symbol| symbol.name == "A")
        .map(|symbol| symbol.role)
        .collect();
    assert_eq!(
        explicit_a_roles,
        [
            crate::EditorSemanticRole::Entity,
            crate::EditorSemanticRole::Entity,
        ]
    );
}

#[test]
#[cfg(feature = "editor-facts")]
fn block_editor_projection_publishes_typed_directive_slots() {
    let text = "block\nA\nclassDef hot \nclass A hot\nstyle A \n";
    let facts =
        crate::family::test_support::editor_facts(parse_block_json_and_editor_facts, text, &meta());

    let class_definition = text.find("classDef hot").unwrap() + "classDef ".len();
    assert!(facts.expected_syntax.iter().any(|expected| {
        expected.kind == EditorExpectedSyntaxKind::ClassName
            && expected.span == SourceSpan::new(class_definition, class_definition + "hot".len())
    }));

    let class_reference = text.find("class A hot").unwrap() + "class A ".len();
    assert!(facts.expected_syntax.iter().any(|expected| {
        expected.kind == EditorExpectedSyntaxKind::ClassName
            && expected.span == SourceSpan::new(class_reference, class_reference + "hot".len())
    }));

    let style_start = text.find("style A ").unwrap();
    let style_slot = style_start + "style A ".len();
    assert!(facts.expected_syntax.iter().any(|expected| {
        expected.kind == EditorExpectedSyntaxKind::Directive
            && expected.span == SourceSpan::new(style_start, style_slot)
    }));
    assert!(facts.expected_syntax.iter().any(|expected| {
        expected.kind == EditorExpectedSyntaxKind::StyleValue
            && expected.span == SourceSpan::new(style_slot, style_slot)
    }));
}

#[test]
#[cfg(feature = "editor-facts")]
fn block_classdef_does_not_consume_the_next_physical_line() {
    for (name, line_ending) in [("lf", "\n"), ("crlf", "\r\n"), ("cr", "\r")] {
        let text = ["block", "classDef", "A", ""].join(line_ending);
        let facts = crate::family::test_support::editor_facts(
            parse_block_json_and_editor_facts,
            &text,
            &meta(),
        );

        assert!(
            facts.symbols.iter().any(|symbol| {
                symbol.name == "A" && symbol.detail.as_deref() == Some("block node")
            }),
            "{name} must retain the next-line node"
        );
        assert!(facts.symbols.iter().all(|symbol| {
            symbol.name != "A" || symbol.detail.as_deref() != Some("block class definition")
        }));
    }
}

#[test]
fn block_recovery_keeps_confirmed_prefix_and_later_semantics() {
    let text = concat!(
        "block-beta\r\n",
        "  A<[\"鏂瑰悜\"]>(right, sideways)\r\n",
        "  鍚庣画[\"瀹屾垚\"]\r\n",
    );
    let invalid_start = text.find("sideways").unwrap();
    let invalid_span = SourceSpan::new(invalid_start, invalid_start + "sideways".len());

    let error = parse_block(text, &meta()).expect_err("strict parsing must reject bad direction");
    let Error::DiagramParse { diagnostic, .. } = error else {
        panic!("expected structured Block parse error");
    };
    assert_eq!(diagnostic.span(), Some(invalid_span));

    #[cfg(feature = "editor-facts")]
    {
        let facts = crate::family::test_support::editor_facts(
            parse_block_json_and_editor_facts,
            text,
            &meta(),
        );
        assert_eq!(facts.completeness, EditorSemanticCompleteness::Recovered);
        assert!(facts.symbols.iter().any(|symbol| symbol.name == "A"));
        assert!(facts.symbols.iter().any(|symbol| symbol.name == "鍚庣画"));
        assert_eq!(facts.diagnostics[0].span, Some(invalid_span));
    }
}

#[test]
fn block_recovery_keeps_an_incomplete_labeled_edge_prefix() {
    let text = "block\nA o-- \"label\"\nB[\"later\"]\n";
    parse_block(text, &meta()).expect_err("strict parsing must reject incomplete labeled edge");
    #[cfg(feature = "editor-facts")]
    {
        let facts = crate::family::test_support::editor_facts(
            parse_block_json_and_editor_facts,
            text,
            &meta(),
        );

        assert_eq!(facts.completeness, EditorSemanticCompleteness::Recovered);
        assert!(facts.symbols.iter().any(|symbol| symbol.name == "B"));
        assert!(facts.symbols.iter().any(|symbol| symbol.name == "later"));
    }
}

#[test]
fn block_malformed_statement_recovers_with_exact_parser_span() {
    let text = "block\nA<[\"Move\"]>(sideways)\nB[\"Later\"]\n";
    let invalid_start = text.find("sideways").unwrap();
    let invalid_span = SourceSpan::new(invalid_start, invalid_start + "sideways".len());

    let error = parse_block(text, &meta()).expect_err("strict parse must reject direction");
    let Error::DiagramParse { diagnostic, .. } = error else {
        panic!("expected structured Block parse error");
    };
    assert_eq!(diagnostic.span(), Some(invalid_span));
    assert_eq!(diagnostic.span_kind(), ParseDiagnosticSpanKind::Exact);

    #[cfg(feature = "editor-facts")]
    {
        reset_block_syntax_construction_count();
        let facts = crate::family::test_support::editor_facts(
            parse_block_json_and_editor_facts,
            text,
            &meta(),
        );
        assert_eq!(block_syntax_construction_count(), 1);
        assert_eq!(facts.completeness, EditorSemanticCompleteness::Recovered);
        assert!(facts.diagnostics.iter().any(|diagnostic| {
            diagnostic.span == Some(invalid_span)
                && diagnostic.message.contains("invalid direction")
        }));
        assert!(facts.symbols.iter().any(|symbol| {
            symbol.name == "B" && symbol.detail.as_deref() == Some("block node")
        }));
        assert!(facts.symbols.iter().any(|symbol| {
            symbol.name == "Later" && symbol.detail.as_deref() == Some("block label")
        }));
    }
}

#[test]
fn block_unclosed_nested_block_reports_eof_insertion_and_partial_facts() {
    let text = "block\nblock:group[\"Group\"]\nA[\"Inside\"]\n";
    let eof = SourceSpan::new(text.len(), text.len());

    let error = parse_block(text, &meta()).expect_err("nested block requires end");
    let Error::DiagramParse { diagnostic, .. } = error else {
        panic!("expected structured Block parse error");
    };
    assert_eq!(diagnostic.span(), Some(eof));
    assert_eq!(
        diagnostic.span_kind(),
        ParseDiagnosticSpanKind::InsertionPoint
    );

    #[cfg(feature = "editor-facts")]
    {
        let facts = crate::family::test_support::editor_facts(
            parse_block_json_and_editor_facts,
            text,
            &meta(),
        );
        assert_eq!(facts.completeness, EditorSemanticCompleteness::Recovered);
        assert!(facts.diagnostics.iter().any(|diagnostic| {
            diagnostic.span == Some(eof) && diagnostic.message.contains("expected end")
        }));
        assert!(facts.symbols.iter().any(|symbol| {
            symbol.name == "group"
                && symbol.detail.as_deref() == Some("block composite")
                && symbol.kind == EditorSemanticKind::Namespace
        }));
        assert!(facts.symbols.iter().any(|symbol| {
            symbol.name == "Inside" && symbol.detail.as_deref() == Some("block label")
        }));
    }
}

#[test]
fn block_deep_chain_semantic_and_render_model_use_heap_traversal() {
    const DEPTH: usize = 1200;
    let input = deep_block_chain(DEPTH);

    // Legacy blocksFlat repeats descendants; its output size is a separate bounded probe.
    const COMPAT_DEPTH: usize = 64;
    let model = parse(&deep_block_chain(COMPAT_DEPTH));
    let blocks_flat = model["blocksFlat"].as_array().expect("blocksFlat array");
    assert_eq!(blocks_flat.len(), COMPAT_DEPTH + 2);
    assert_eq!(blocks_flat[0]["id"].as_str(), Some("root"));
    assert_eq!(
        blocks_flat
            .last()
            .and_then(|block| block.get("id"))
            .and_then(Value::as_str),
        Some("leaf")
    );

    let parsed = MermaidEngine::new()
        .parse_diagram_for_render_model_sync(&input, ParseOptions::strict())
        .unwrap()
        .unwrap();
    match parsed.model() {
        RenderSemanticModel::Block(model) => {
            assert_eq!(model.blocks_flat.len(), DEPTH + 2);
            assert_eq!(model.blocks_flat[0].id, "root");
            assert_eq!(
                model.blocks_flat.last().map(|block| block.id.as_str()),
                Some("leaf")
            );
        }
        other => panic!("block render parse should return typed model, got {other:?}"),
    }
}

#[test]
fn nodes_with_edge_label() {
    let model = parse("block\n  id1[\"first\"]  -- \"a label\" -->   id2[\"second\"]\n");
    let edges = edges(&model);
    assert_eq!(edges[0]["label"].as_str().unwrap(), "a label");
}

#[test]
fn diagram_with_column_statements() {
    let model = parse("block\n  columns 2\n  block1[\"Block 1\"]\n");
    assert_eq!(columns_for_id(&model, "root").unwrap(), 2);
    assert_eq!(blocks(&model).len(), 1);
}

#[test]
fn diagram_without_column_statements() {
    let model = parse("block\n  block1[\"Block 1\"]\n");
    assert_eq!(columns_for_id(&model, "root").unwrap(), -1);
    assert_eq!(blocks(&model).len(), 1);
}

#[test]
fn diagram_with_auto_column_statements() {
    let model = parse("block\n  columns auto\n  block1[\"Block 1\"]\n");
    assert_eq!(columns_for_id(&model, "root").unwrap(), -1);
    assert_eq!(blocks(&model).len(), 1);
}

#[test]
fn blocks_next_to_each_other() {
    let model = parse("block\n  columns 2\n  block1[\"Block 1\"]\n  block2[\"Block 2\"]\n");
    assert_eq!(columns_for_id(&model, "root").unwrap(), 2);
    assert_eq!(blocks(&model).len(), 2);
}

#[test]
fn blocks_on_top_of_each_other() {
    let model = parse("block\n  columns 1\n  block1[\"Block 1\"]\n  block2[\"Block 2\"]\n");
    assert_eq!(columns_for_id(&model, "root").unwrap(), 1);
    assert_eq!(blocks(&model).len(), 2);
}

#[test]
fn compound_blocks() {
    let model = parse("block\n  block\n    aBlock[\"ABlock\"]\n    bBlock[\"BBlock\"]\n  end\n");
    let blocks = blocks(&model);
    assert_eq!(blocks.len(), 1);
    assert_eq!(blocks[0]["type"].as_str().unwrap(), "composite");
    assert_eq!(blocks[0]["children"].as_array().unwrap().len(), 2);
}

#[test]
fn compound_blocks_of_compound_blocks() {
    let model = parse(
        "block\n  block\n    aBlock[\"ABlock\"]\n    block\n      bBlock[\"BBlock\"]\n    end\n  end\n",
    );
    let blocks = blocks(&model);
    assert_eq!(blocks.len(), 1);
    let first = &blocks[0];
    assert_eq!(first["children"].as_array().unwrap().len(), 2);
    let a_block = &first["children"][0];
    assert_eq!(a_block["label"].as_str().unwrap(), "ABlock");
    let second_composite = &first["children"][1];
    assert_eq!(second_composite["type"].as_str().unwrap(), "composite");
    assert_eq!(second_composite["children"].as_array().unwrap().len(), 1);
    let b_block = &second_composite["children"][0];
    assert_eq!(b_block["label"].as_str().unwrap(), "BBlock");
}

#[test]
fn compound_blocks_with_title() {
    let model = parse(
        "block\n  block:compoundBlock[\"Compound block\"]\n    columns 1\n    block2[\"Block 2\"]\n  end\n",
    );
    let blocks = blocks(&model);
    assert_eq!(blocks.len(), 1);
    let compound = &blocks[0];
    assert_eq!(compound["id"].as_str().unwrap(), "compoundBlock");
    assert_eq!(compound["label"].as_str().unwrap(), "Compound block");
    assert_eq!(compound["type"].as_str().unwrap(), "composite");
    assert_eq!(compound["children"].as_array().unwrap().len(), 1);
    assert_eq!(compound["children"][0]["id"].as_str().unwrap(), "block2");
}

#[test]
fn blocks_mixed_with_compound_blocks() {
    let model = parse(
        "block\n  columns 1\n  block1[\"Block 1\"]\n\n  block\n    columns 2\n    block2[\"Block 2\"]\n    block3[\"Block 3\"]\n  end\n",
    );
    let blocks = blocks(&model);
    assert_eq!(blocks.len(), 2);
    let compound = &blocks[1];
    assert_eq!(compound["type"].as_str().unwrap(), "composite");
    assert_eq!(compound["children"].as_array().unwrap().len(), 2);
    assert_eq!(compound["children"][0]["id"].as_str().unwrap(), "block2");
}

#[test]
fn arrow_blocks() {
    let model = parse(
        "block\n  columns 3\n  block1[\"Block 1\"]\n  blockArrow<[\"&nbsp;&nbsp;&nbsp;\"]>(right)\n  block2[\"Block 2\"]\n",
    );
    let blocks = blocks(&model);
    assert_eq!(blocks.len(), 3);
    assert_eq!(blocks[1]["type"].as_str().unwrap(), "block_arrow");
    assert!(
        blocks[1]["directions"]
            .as_array()
            .unwrap()
            .iter()
            .any(|v| v.as_str() == Some("right"))
    );
}

#[test]
fn arrow_blocks_with_multiple_points() {
    let model = parse(
        "block\n  columns 1\n  A\n  blockArrow<[\"&nbsp;&nbsp;&nbsp;\"]>(up, down)\n  block\n    columns 3\n    B\n    C\n    D\n  end\n",
    );
    let blocks = blocks(&model);
    assert_eq!(blocks.len(), 3);
    let arrow = &blocks[1];
    assert_eq!(arrow["type"].as_str().unwrap(), "block_arrow");
    let dirs: Vec<&str> = arrow["directions"]
        .as_array()
        .unwrap()
        .iter()
        .filter_map(|v| v.as_str())
        .collect();
    assert!(dirs.contains(&"up"));
    assert!(dirs.contains(&"down"));
    assert!(!dirs.contains(&"right"));
}

#[test]
fn blocks_with_different_widths() {
    let model = parse("block\n  columns 3\n  one[\"One Slot\"]\n  two[\"Two slots\"]:2\n");
    let blocks = blocks(&model);
    assert_eq!(blocks.len(), 2);
    assert_eq!(blocks[1]["widthInColumns"].as_i64().unwrap(), 2);
}

#[test]
fn empty_blocks_space() {
    let model = parse("block\n  columns 3\n  space\n  middle[\"In the middle\"]\n  space\n");
    let blocks = blocks(&model);
    assert_eq!(blocks.len(), 3);
    assert_eq!(blocks[0]["type"].as_str().unwrap(), "space");
    assert_eq!(blocks[2]["type"].as_str().unwrap(), "space");
    assert_eq!(blocks[1]["label"].as_str().unwrap(), "In the middle");
}

#[test]
fn generated_block_ids_are_deterministic_for_default_engine() {
    fn generated_ids(model: &Value) -> Vec<String> {
        model["blocksFlat"]
            .as_array()
            .into_iter()
            .flatten()
            .filter_map(|block| block["id"].as_str())
            .filter(|id| id.starts_with("id-"))
            .map(ToString::to_string)
            .collect()
    }

    let first = generated_ids(&parse("block\n  columns 2\n  space\n  space\n"));
    let second = generated_ids(&parse("block\n  columns 2\n  space\n  space\n"));

    assert_eq!(first, second);
    assert!(first.len() >= 2);
    let unique = first.iter().collect::<std::collections::BTreeSet<_>>();
    assert_eq!(unique.len(), first.len());
}

#[test]
fn classdef_and_apply_class() {
    let model = parse(
        "block\n  classDef black color:#ffffff, fill:#000000;\n  mc[\"Memcache\"]\n  class mc black\n",
    );
    let blocks = blocks(&model);
    assert_eq!(blocks.len(), 1);
    assert!(
        blocks[0]["classes"]
            .as_array()
            .unwrap()
            .iter()
            .any(|v| v.as_str() == Some("black"))
    );
    let classes = model["classes"].as_object().unwrap();
    let black = classes.get("black").unwrap();
    assert_eq!(black["id"].as_str().unwrap(), "black");
    assert_eq!(black["styles"][0].as_str().unwrap(), "color:#ffffff");
}

#[test]
fn classdef_ids_follow_mermaid_word_identifier_vocabulary() {
    let text = "block\nA\nclassDef foo.bar fill:#f00\nclass A foo.bar\n";
    let error = parse_block(text, &meta()).expect_err("dotted classDef id must be rejected");
    let Error::DiagramParse { diagnostic, .. } = error else {
        panic!("expected structured Block parse error");
    };
    let start = text.find("foo.bar").expect("fixture classDef id");
    assert_eq!(
        diagnostic.span(),
        Some(SourceSpan::new(start, start + "foo.bar".len()))
    );
    assert!(diagnostic.message().contains("invalid classDef identifier"));
}

#[test]
fn style_statement_applied() {
    let model = parse(
        "block\n  columns 1\n  B[\"A wide one in the middle\"]\n  style B fill:#f9F,stroke:#333,stroke-width:4px\n",
    );
    let blocks = blocks(&model);
    assert_eq!(blocks.len(), 1);
    let styles: Vec<&str> = blocks[0]["styles"]
        .as_array()
        .unwrap()
        .iter()
        .filter_map(|v| v.as_str())
        .collect();
    assert!(styles.contains(&"fill:#f9F"));
}

#[test]
fn warns_when_block_width_exceeds_column_width() {
    let text = "block-beta\n  columns 1\n  A:1\n  B:2\n  C:3\n";
    let meta = meta();
    let model = parse_block(text, &meta).unwrap();
    let typed = parse_block_model_for_render(text, &meta).unwrap();
    assert_eq!(render_model_to_compat_json(&typed, &meta).unwrap(), model);
    let warnings: Vec<&str> = model["warningFacts"]
        .as_array()
        .unwrap()
        .iter()
        .filter_map(|v| v.get("message").and_then(|message| message.as_str()))
        .collect();
    assert!(warnings.contains(&"Block B width 2 exceeds configured column width 1"));
    assert_eq!(model["warnings"], json!(warnings));
}

#[test]
fn prototype_property_ids_do_not_crash() {
    for prop in ["__proto__", "constructor"] {
        let text = format!("block\n{prop}\n");
        let _ = parse(&text);
        let text =
            format!("block\nA\nclassDef {prop} color:#ffffff,fill:#000000;\nclass A {prop}\n");
        let _ = parse(&text);
        let text = format!("block\nA; classDef {prop} color:#ffffff,fill:#000000; class A {prop}");
        let _ = parse(&text);
    }
}
