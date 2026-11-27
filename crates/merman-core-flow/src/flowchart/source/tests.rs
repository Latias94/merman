use super::*;
use crate::flowchart::{FlowchartLexer, parse_flowchart};
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
