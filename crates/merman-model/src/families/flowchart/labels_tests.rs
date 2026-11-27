use super::*;
use crate::resources::{
    FlowchartComplexity, InputResourceLimitId, InputResourcePolicy, ResourceProfile,
};

fn model() -> FlowchartModel {
    serde_json::from_value(serde_json::json!({
        "nodes": [{"id": "A", "label": "canonical node", "layoutShape": "squareRect"}],
        "edges": [
            {"id": "same", "from": "A", "to": "A", "length": 1, "label": "first"},
            {"id": "same", "from": "A", "to": "A", "length": 1, "label": "second"}
        ],
        "subgraphs": [
            {"id": "G", "title": "first group", "dir": null, "nodes": []},
            {"id": "G", "title": "second group", "dir": null, "nodes": []}
        ]
    }))
    .unwrap()
}

#[test]
fn sparse_sources_keep_occurrence_identity_and_stay_out_of_serialization() {
    let mut model = model();
    let canonical_json = serde_json::to_vec(&model).unwrap();
    model
        .label_sources
        .insert_node("A".to_owned(), "&lt;Node&gt;".to_owned());
    model
        .label_sources
        .insert_edge(1, "second &lt;T&gt;".to_owned());
    model.label_sources.insert_edge(0, String::new());
    model
        .label_sources
        .insert_subgraph("G".to_owned(), 1, Some("&lt;Group&gt;".to_owned()));
    let edge_owner = FlowchartLabelSourceOwner::Edge(1);
    model
        .label_sources
        .insert_svg_text(edge_owner, "&lt;edge&gt;".to_owned());
    let group_owner = FlowchartLabelSourceOwner::Subgraph(1, "G");
    model
        .label_sources
        .insert_svg_text(group_owner, "&lt;title&gt;".to_owned());
    assert_eq!(
        model.label_sources.svg_text_source(edge_owner),
        Some("&lt;edge&gt;")
    );
    assert_eq!(
        model
            .label_sources
            .svg_text_source(FlowchartLabelSourceOwner::Edge(0)),
        None
    );
    assert_eq!(
        model.label_sources.svg_text_source(group_owner),
        Some("&lt;title&gt;")
    );
    assert_eq!(
        model
            .label_sources
            .svg_text_source(FlowchartLabelSourceOwner::Subgraph(1, "other")),
        None
    );

    assert_eq!(
        model.node_label_source(&model.nodes[0]),
        Some("&lt;Node&gt;")
    );
    assert_eq!(model.label_sources.node_source("missing"), None);
    assert_eq!(model.edge_label_source(0, &model.edges[0]), Some(""));
    assert_eq!(
        model.edge_label_source(1, &model.edges[1]),
        Some("second &lt;T&gt;")
    );
    assert_eq!(model.label_sources.edge_source(2), None);
    assert_eq!(
        model.subgraph_title_source(0, &model.subgraphs[0]),
        "first group"
    );
    assert_eq!(
        model.subgraph_title_source(1, &model.subgraphs[1]),
        "&lt;Group&gt;"
    );
    assert_eq!(model.label_sources.subgraph_source(1, "other"), None);
    assert_eq!(
        model
            .label_sources
            .validate_for_model_controlled(&model, &OperationControl::new())
            .unwrap(),
        Ok(())
    );
    assert_eq!(serde_json::to_vec(&model).unwrap(), canonical_json);
    assert!(!model.label_sources.is_empty());
    assert_eq!(
        serde_json::to_value(&model.label_sources).unwrap(),
        serde_json::json!({
            "nodes": {"A": "&lt;Node&gt;"},
            "edges": [[0, ""], [1, "second &lt;T&gt;"]],
            "subgraphs": [null, {"id": "G", "source": "&lt;Group&gt;"}]
        })
    );

    let roundtrip: FlowchartModel = serde_json::from_slice(&canonical_json).unwrap();
    assert_eq!(roundtrip.label_sources.node_source("A"), None);
    assert!(roundtrip.label_sources.is_empty());
    assert_eq!(
        roundtrip.node_label_source(&roundtrip.nodes[0]),
        Some("canonical node")
    );
    assert_eq!(
        roundtrip.edge_label_source(1, &roundtrip.edges[1]),
        Some("second")
    );
}

#[test]
fn source_clones_preserve_ownership_and_charge_utf8_bytes_to_the_model() {
    let mut model = model();
    let baseline = FlowchartComplexity::from_model(&model).label_bytes;
    model
        .label_sources
        .insert_node("A".to_owned(), "节点 &lt;T&gt;".to_owned());
    let mut cloned = model.clone();
    assert!(
        model
            .label_sources
            .shares_storage_with(&cloned.label_sources)
    );
    cloned.label_sources.insert_svg_text(
        FlowchartLabelSourceOwner::Node("A"),
        "节点 &lt;tag&gt;".to_owned(),
    );
    assert!(
        !model
            .label_sources
            .shares_storage_with(&cloned.label_sources)
    );
    assert_eq!(model.label_sources.node_source("A"), Some("节点 &lt;T&gt;"));
    assert_eq!(
        cloned.label_sources.node_source("A"),
        Some("节点 &lt;T&gt;")
    );
    assert_eq!(
        model
            .label_sources
            .svg_text_source(FlowchartLabelSourceOwner::Node("A")),
        None
    );

    let expected = baseline + "A节点 &lt;T&gt;".len() + "A节点 &lt;tag&gt;".len();
    let policy = InputResourcePolicy::for_profile(ResourceProfile::UnboundedForTrustedInput)
        .with_limit(InputResourceLimitId::MaxModelTextBytes, expected)
        .unwrap();
    assert_eq!(
        policy
            .check_flowchart_complexity(&cloned)
            .unwrap()
            .label_bytes,
        expected
    );
    let error = policy
        .with_limit(InputResourceLimitId::MaxModelTextBytes, expected - 1)
        .unwrap()
        .check_flowchart_complexity(&cloned)
        .unwrap_err();
    assert_eq!(error.actual, expected);
}

#[test]
fn validation_rejects_corrupted_edge_order_before_binary_search_consumers() {
    let model = model();
    let mut sources = FlowchartRenderLabelSources::default();
    sources.insert_edge(0, "first".to_owned());
    sources.insert_edge(1, "second".to_owned());
    FlowchartRenderLabelSources::table_mut(&mut sources.sources)
        .edges
        .swap(0, 1);
    assert_eq!(
        sources
            .validate_for_model_controlled(&model, &OperationControl::new())
            .unwrap(),
        Err("flowchart edge label semantic indices are not strictly increasing")
    );
}
