use super::*;
use crate::flowchart::{FlowchartLexer, parse_flowchart};
use merman_core_preprocess::grid_coordinate::{GridCoordinateField, GridCoordinateValue};
use merman_model::OperationCancelled;
use serde_json::json;

pub(super) fn parse_ast(source: &str) -> FlowchartAst {
    let control = OperationControl::new();
    let lexer = FlowchartLexer::new(source).with_control(&control);
    parse_flowchart(lexer, &control, 500, source.len())
        .expect("a private parse control cannot be cancelled")
        .expect("valid Flowchart test source")
}

#[test]
fn inline_grid_uses_db_order_and_the_last_writer_of_each_field() {
    for (header, diagram_type) in [("flowchart", "flowchart-v2"), ("swimlane-beta", "swimlane")] {
        let input = format!(
            "{header} TD\nZ\nA@{{ row: 0.5 }}\nZ@{{ column: null, row: 7 }}\nZ@{{ row: 0.5 }}\n"
        );
        let config = MermaidConfig::from_value(json!({"layout": "grid"}));
        let control = OperationControl::new();
        #[cfg(feature = "presentation")]
        let (_, _, grid) = parse_ast(&input)
            .into_model_with_render_context_and_grid_coordinates(
                diagram_type,
                &config,
                None,
                None,
                &control,
            )
            .unwrap()
            .unwrap();
        #[cfg(not(feature = "presentation"))]
        let (_, grid) = parse_ast(&input)
            .into_model_with_grid_coordinates(diagram_type, &config, None, None, &control)
            .unwrap()
            .unwrap();
        let fields: Vec<_> = grid.iter().collect();
        assert_eq!(fields.len(), 3);
        let row = fields[0];
        assert_eq!(
            (row.id, row.field, row.value),
            (
                "Z",
                GridCoordinateField::Row,
                GridCoordinateValue::Number(0.5)
            )
        );
        assert_eq!(row.span.start, input.rfind("@{ row: 0.5 }").unwrap());
        let column = fields[1];
        assert_eq!(
            (column.id, column.field, column.value),
            ("Z", GridCoordinateField::Column, GridCoordinateValue::Null)
        );
        assert_eq!(
            &input[column.span.start..column.span.end],
            "@{ column: null, row: 7 }"
        );
        assert_eq!(fields[2].id, "A");

        let (error, span) = grid.validate_controlled(&control).unwrap().unwrap_err();
        assert_eq!(span, row.span);
        assert_eq!(
            error.to_string(),
            "GRID_INVALID_COORDINATE: Invalid row for placement \"Z\": expected a safe integer; received 0.5 (number)."
        );
        control.cancel_after_checkpoints(0);
        assert!(matches!(
            grid.validate_controlled(&control),
            Err(OperationCancelled { .. })
        ));
    }
}

#[test]
fn inline_grid_follows_vertex_edge_and_completed_subgraph_dispatch() {
    let input = concat!(
        "flowchart TD\n",
        "G@{ row: null } & G@{ row: -1 }\n",
        "subgraph G\n  A\nend\n",
        "A G@--> B\n",
        "B onlyEdge@--> C\n",
        "onlyEdge@{ row: false }\n",
        "G@{ column: true }\n",
    );
    let control = OperationControl::new();
    let config = MermaidConfig::from_value(json!({"layout": "grid"}));
    let (model, grid) = parse_ast(input)
        .into_model_with_grid_coordinates("flowchart-v2", &config, None, None, &control)
        .unwrap()
        .unwrap();
    assert_eq!(model.edges[0].id, "G");
    let fields: Vec<_> = grid.iter().collect();
    assert_eq!(fields.len(), 2);
    let vertex = fields[0];
    assert_eq!(
        (vertex.id, vertex.field, vertex.value),
        (
            "G",
            GridCoordinateField::Row,
            GridCoordinateValue::Number(-1.0)
        )
    );
    assert_eq!(&input[vertex.span.start..vertex.span.end], "@{ row: -1 }");
    let (error, span) = grid.validate_controlled(&control).unwrap().unwrap_err();
    assert_eq!(
        (error.placement_id, error.field, error.received),
        (
            "G",
            GridCoordinateField::Column,
            GridCoordinateValue::Boolean(true)
        )
    );
    assert_eq!(&input[span.start..span.end], "@{ column: true }");
}

#[test]
fn inline_grid_waits_for_replay_and_leaves_other_layout_metadata_uninterpreted() {
    let control = OperationControl::new();
    let grid = MermaidConfig::from_value(json!({"layout": "grid"}));
    let result = parse_ast("flowchart TD\nA@{ row: 0.5 }\nB@{ shape: Bad }\n")
        .into_model_with_grid_coordinates("flowchart-v2", &grid, None, None, &control)
        .unwrap();
    assert_eq!(
        result.unwrap_err(),
        "No such shape: Bad. Shape names should be lowercase."
    );

    let dagre = MermaidConfig::from_value(json!({"layout": "dagre"}));
    let (_, grid) = parse_ast("flowchart TD\nA@{ row: 0.5 }\n")
        .into_model_with_grid_coordinates("flowchart-v2", &dagre, None, None, &control)
        .unwrap()
        .unwrap();
    assert_eq!(grid.iter().count(), 0);
}

#[test]
fn semantic_flowchart_source_does_not_collect_render_only_sidecars() {
    let config = MermaidConfig::empty_object();
    let control = OperationControl::new();
    let ast =
        parse_ast("flowchart TD\nsubgraph G\nA[\"&nbsp;label&nbsp;\"]\nend\nG@{ view: collapsed }");
    let source = build_flowchart_source(ast, None, None, &config, false, &control)
        .expect("a private parse control cannot be cancelled")
        .expect("semantic source");

    #[cfg(feature = "presentation")]
    {
        assert!(source.collapsed_subgraphs.is_none());
    }
    let model = source
        .into_model_parts_controlled("flowchart-v2", &config, false, &control)
        .expect("a private parse control cannot be cancelled")
        .expect("semantic projection")
        .model;
    assert_eq!(model.subgraphs[0].id, "G");
    assert!(model.contains_subgraph_vertex("G"));
}

#[test]
fn same_id_shape_data_routes_to_subgraph_metadata_without_mutating_node() {
    let ast = parse_ast(concat!(
        "flowchart TD\n",
        "subgraph G\n",
        "  A\n",
        "end\n",
        "G --> A\n",
        "G@{ shape: stadium, label: \"Authored G\" }\n",
    ));
    let source = build_flowchart_source(
        ast,
        None,
        None,
        &MermaidConfig::empty_object(),
        false,
        &OperationControl::new(),
    )
    .expect("a private parse control cannot be cancelled")
    .expect("same-id shapeData should parse");

    let subgraph = source
        .subgraphs
        .iter()
        .find(|subgraph| subgraph.id == "G")
        .expect("subgraph metadata target");
    assert_eq!(
        subgraph.metadata.as_ref().unwrap()["shape"],
        json!("stadium")
    );
    assert_eq!(
        subgraph.metadata.as_ref().unwrap()["label"],
        json!("Authored G")
    );

    let node = source
        .nodes
        .iter()
        .find(|node| node.id == "G")
        .expect("authored G node");
    assert_eq!(node.provenance, FlowNodeProvenance::Authored);
    assert_eq!(node.shape, None);
    assert_eq!(node.label, None);
}

#[test]
fn flowchart_source_strips_prototype_keys_from_subgraph_metadata() {
    let ast = parse_ast(concat!(
        "flowchart TD\n",
        "subgraph __group\n",
        "  A\n",
        "end\n",
        "__group@{\n",
        "  algorithm: elk.box\n",
        "  __proto__: ignored\n",
        "  details:\n",
        "    - constructor: ignored\n",
        "      prototype: ignored\n",
        "      note: kept\n",
        "}\n",
    ));
    let source = build_flowchart_source(
        ast,
        None,
        None,
        &MermaidConfig::empty_object(),
        false,
        &OperationControl::new(),
    )
    .expect("a private parse control cannot be cancelled")
    .expect("subgraph metadata should parse");

    let group = &source.subgraphs[0];
    assert_eq!(group.id, "__group");
    assert_eq!(
        group.metadata.as_ref(),
        Some(&json!({"algorithm": "elk.box", "details": [{"note": "kept"}]}))
    );
}

#[test]
fn single_large_chain_observes_cancellation_in_post_parse_projections() {
    let nodes = (0..256)
        .map(|index| format!("n{index}"))
        .collect::<Vec<_>>()
        .join(" & ");
    let source = format!("flowchart TD\n{nodes}\n");
    let ast = parse_ast(&source);
    assert!(matches!(
        ast.walk().next(),
        Some(StatementEvent::Enter(Stmt::Chain { node_groups, .. }))
            if node_groups.iter().map(Vec::len).sum::<usize>() == 256
    ));

    let shape_data_control = OperationControl::new();
    shape_data_control.cancel_after_checkpoints(2);
    assert!(matches!(
        prepare_flowchart_shape_data(&ast, &shape_data_control),
        Err(OperationCancelled { .. })
    ));

    let config = MermaidConfig::empty_object();
    let semantic_source =
        build_flowchart_source(ast, None, None, &config, false, &OperationControl::new())
            .expect("a private parse control cannot be cancelled")
            .expect("large node group should build");
    let render_control = OperationControl::new();
    render_control.cancel_after_checkpoints(2);
    assert!(matches!(
        semantic_source.into_model_parts_controlled(
            "flowchart-v2",
            &config,
            false,
            &render_control
        ),
        Err(OperationCancelled { .. })
    ));
}

#[test]
fn single_large_subgraph_chain_observes_cancellation_in_membership_projection() {
    let nodes = (0..256)
        .map(|index| format!("n{index}"))
        .collect::<Vec<_>>()
        .join(" & ");
    let source = format!("flowchart TD\nsubgraph group\n{nodes}\nend\n");
    let ast = parse_ast(&source);
    let mut builder = SubgraphBuilder::new(false, ast.direction.clone());
    let control = OperationControl::new();
    control.cancel_after_checkpoints(2);

    assert!(matches!(
        builder.visit_statements(&ast, &control),
        Err(OperationCancelled { .. })
    ));
}

#[test]
fn flowchart_subgraphs_exist_matches_mermaid_flowdb_spec() {
    let subgraphs = vec![
        FlowSubGraph {
            id: "sg0".to_string(),
            nodes: vec![
                "a".to_string(),
                "b".to_string(),
                "c".to_string(),
                "e".to_string(),
            ],
            title: "".to_string(),
            classes: Vec::new(),
            styles: Vec::new(),
            dir: None,
            has_explicit_dir: false,
            label_type: "text".to_string(),
            metadata: None,
        },
        FlowSubGraph {
            id: "sg1".to_string(),
            nodes: vec!["f".to_string(), "g".to_string(), "h".to_string()],
            title: "".to_string(),
            classes: Vec::new(),
            styles: Vec::new(),
            dir: None,
            has_explicit_dir: false,
            label_type: "text".to_string(),
            metadata: None,
        },
        FlowSubGraph {
            id: "sg2".to_string(),
            nodes: vec!["i".to_string(), "j".to_string()],
            title: "".to_string(),
            classes: Vec::new(),
            styles: Vec::new(),
            dir: None,
            has_explicit_dir: false,
            label_type: "text".to_string(),
            metadata: None,
        },
        FlowSubGraph {
            id: "sg3".to_string(),
            nodes: vec!["k".to_string()],
            title: "".to_string(),
            classes: Vec::new(),
            styles: Vec::new(),
            dir: None,
            has_explicit_dir: false,
            label_type: "text".to_string(),
            metadata: None,
        },
    ];

    assert!(subgraph::subgraphs_exist(&subgraphs, "a"));
    assert!(subgraph::subgraphs_exist(&subgraphs, "h"));
    assert!(subgraph::subgraphs_exist(&subgraphs, "j"));
    assert!(subgraph::subgraphs_exist(&subgraphs, "k"));

    assert!(!subgraph::subgraphs_exist(&subgraphs, "a2"));
    assert!(!subgraph::subgraphs_exist(&subgraphs, "l"));
}

fn styles() -> FlowchartSubgraphStyles {
    let mut styles = FlowchartSubgraphStyles::default();
    styles.insert(
        "z".to_owned(),
        7,
        FlowSubgraphVertexStyle {
            classes: vec!["paint".to_owned(), "typography".to_owned()],
            styles: vec!["fill:red".to_owned(), "stroke:blue".to_owned()],
        },
    );
    styles.insert("a".to_owned(), 0, FlowSubgraphVertexStyle::default());
    styles
}

#[test]
fn subgraph_styles_remain_excluded_from_the_existing_typed_wire_and_complexity() {
    let mut model: FlowchartModel = serde_json::from_str(r#"{"nodes":[],"edges":[]}"#).unwrap();
    let wire = serde_json::to_string(&model).unwrap();
    let complexity = merman_model::resources::ModelComplexity::from_serializable(&model);
    model.subgraph_vertex_styles = styles();
    assert_eq!(serde_json::to_string(&model).unwrap(), wire);
    assert_eq!(
        merman_model::resources::ModelComplexity::from_serializable(&model),
        complexity
    );
}
