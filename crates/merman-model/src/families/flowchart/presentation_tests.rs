use super::*;
use crate::families::flowchart::{
    FlowEdgeMarker, FlowEdgeStroke, FlowEdgeVisibility, FlowNodeProvenance, FlowSubgraphVertexStyle,
};

fn node(id: &str) -> FlowNode {
    FlowNode {
        id: id.to_owned(),
        provenance: FlowNodeProvenance::Authored,
        label: Some(id.to_owned()),
        label_type: None,
        layout_shape: None,
        shape: None,
        icon: None,
        form: None,
        pos: None,
        img: None,
        constraint: None,
        asset_width: None,
        asset_height: None,
        classes: Vec::new(),
        styles: Vec::new(),
        link: None,
        link_target: None,
        have_callback: false,
    }
}

fn edge(label: &str) -> FlowEdge {
    FlowEdge {
        id: "duplicate-edge-id".to_owned(),
        from: "A".to_owned(),
        to: "B".to_owned(),
        label: Some(label.to_owned()),
        label_type: None,
        edge_type: None,
        arrow: "-->".to_owned(),
        start_marker: FlowEdgeMarker::None,
        end_marker: FlowEdgeMarker::Point,
        is_user_defined_id: true,
        stroke: None,
        stroke_kind: FlowEdgeStroke::Normal,
        visibility: FlowEdgeVisibility::Visible,
        interpolate: None,
        classes: Vec::new(),
        style: Vec::new(),
        animate: None,
        animation: None,
        length: 1,
    }
}

fn subgraph(id: &str, children: &[&str]) -> FlowSubgraph {
    FlowSubgraph {
        id: id.to_owned(),
        title: id.to_owned(),
        dir: None,
        has_explicit_dir: false,
        label_type: None,
        classes: Vec::new(),
        styles: Vec::new(),
        nodes: children.iter().map(|id| (*id).to_owned()).collect(),
        metadata: None,
    }
}

fn model() -> FlowchartModel {
    FlowchartModel {
        keyword: "flowchart".to_owned(),
        acc_descr: None,
        acc_title: None,
        class_defs: Default::default(),
        direction: Some("TB".to_owned()),
        edge_defaults: None,
        vertex_calls: Vec::new(),
        nodes: vec![node("A"), node("B")],
        edges: vec![edge("first & owner"), edge("second < owner")],
        subgraphs: vec![subgraph("G", &["H"]), subgraph("H", &["A", "B"])],
        subgraph_vertex_styles: Default::default(),
        tooltips: Default::default(),
        warning_facts: Vec::new(),
    }
}

fn context(model: &FlowchartModel) -> FlowchartRenderContext {
    let mut labels = FlowchartRenderLabelSources::default();
    labels.insert_node("A".to_owned(), "&nbsp;A".to_owned());
    // Edge identity belongs to its semantic ordinal even when its public ID repeats.
    labels.insert_edge(1, "second &lt; owner".to_owned());
    labels.insert_edge(0, "first &amp; owner".to_owned());
    labels.insert_subgraph("H".to_owned(), 1, Some("&nbsp;H".to_owned()));
    FlowchartRenderContext::new(
        labels,
        model.subgraph_vertex_styles.clone(),
        FxHashSet::from_iter(["G".to_owned(), "H".to_owned()]),
        &model.subgraphs,
    )
}

#[test]
fn ordinary_context_inherits_canonical_css_without_duplicate_text_accounting() {
    let mut model = model();
    model.subgraph_vertex_styles.insert(
        "G".to_owned(),
        0,
        FlowSubgraphVertexStyle {
            classes: vec!["canonical".to_owned()],
            styles: vec!["fill:red".to_owned()],
        },
    );
    let context = FlowchartRenderContext::from_model(&model);
    assert_eq!(
        context.effective_subgraph_css(0, &model.subgraphs[0]),
        model.effective_subgraph_css(0, &model.subgraphs[0])
    );
    assert!(context.contains_subgraph_vertex("G"));
    assert_eq!(context.retained_bytes(), 0);
    assert_eq!(
        context.node_label_for_render(&model.nodes[0]),
        model.nodes[0].label.as_deref()
    );
    assert_eq!(context.node_dom_index("G"), None);
    assert!(!context.is_subgraph_collapsed("G"));
    assert_eq!(
        context
            .validate_for_model_controlled(&model, &OperationControl::new())
            .unwrap(),
        Ok(())
    );

    let mut changed = model.subgraph_vertex_styles.clone();
    changed.insert("G".to_owned(), 0, FlowSubgraphVertexStyle::default());
    assert_ne!(
        context.effective_subgraph_css(0, &model.subgraphs[0]),
        changed.effective_subgraph_css(0, &model.subgraphs[0])
    );
}

#[test]
fn context_accepts_duplicate_ids_sparse_labels_and_hidden_collapsed_nodes() {
    let mut model = model();
    model.nodes.push(node("A"));
    model.subgraphs.push(subgraph("G", &[]));
    let mut context = context(&model);
    context
        .labels
        .insert_subgraph("G".to_owned(), 2, Some("latest G".to_owned()));
    context.set_node_dom_indices([
        ("A".to_owned(), 1_000_000),
        ("B".to_owned(), 2_000_000),
        ("G".to_owned(), 3_000_000),
    ]);

    assert_eq!(
        context
            .validate_for_model_controlled(&model, &OperationControl::new())
            .unwrap(),
        Ok(())
    );
    assert_eq!(
        context.edge_label_for_render(0, &model.edges[0]),
        Some("first &amp; owner")
    );
    assert_eq!(
        context.edge_label_for_render(1, &model.edges[1]),
        Some("second &lt; owner")
    );
    assert_eq!(
        context.subgraph_title_for_render(2, &model.subgraphs[2]),
        "latest G"
    );
    assert_eq!(context.collapsed_replacement("A"), Some("G"));
    assert_eq!(context.collapsed_replacement("H"), Some("G"));
    assert!(context.is_subgraph_collapsed("H"));
}

#[test]
fn css_validation_checks_only_the_retained_same_id_source_and_its_exact_ordinal() {
    let mut model = model();
    model.subgraphs.push(subgraph("G", &[]));
    let mut styles = FlowchartSubgraphStyles::default();
    styles.insert(
        "G".to_owned(),
        usize::MAX,
        FlowSubgraphVertexStyle::default(),
    );
    styles.insert(
        "G".to_owned(),
        2,
        FlowSubgraphVertexStyle {
            classes: vec!["later".to_owned()],
            styles: vec!["fill:red".to_owned()],
        },
    );
    model.subgraph_vertex_styles = styles.clone();
    let context = context(&model);
    let control = OperationControl::new();

    assert_eq!(
        styles
            .validate_for_model_controlled(&model, &control)
            .unwrap(),
        Ok(())
    );
    assert_eq!(
        context
            .validate_for_model_controlled(&model, &control)
            .unwrap(),
        Ok(())
    );
    assert!(
        styles
            .effective_subgraph_css(0, &model.subgraphs[0])
            .0
            .is_empty()
    );
    assert_eq!(
        styles.effective_subgraph_css(2, &model.subgraphs[2]).0,
        ["later"]
    );

    // The latest insertion can select an earlier canonical slot; it is not max(ordinal).
    styles.insert("G".to_owned(), 0, FlowSubgraphVertexStyle::default());
    assert_eq!(
        styles
            .validate_for_model_controlled(&model, &control)
            .unwrap(),
        Ok(())
    );
    styles.insert("G".to_owned(), 1, FlowSubgraphVertexStyle::default());
    assert!(
        styles
            .validate_for_model_controlled(&model, &control)
            .unwrap()
            .is_err()
    );
}

#[test]
fn context_rejects_foreign_labels_and_wrong_declaration_owners() {
    let model = model();
    let base = context(&model);
    let cases = [
        ("node identity", {
            let mut context = base.clone();
            context
                .labels
                .insert_node("missing".to_owned(), "label".to_owned());
            context
        }),
        ("edge range", {
            let mut context = base.clone();
            context
                .labels
                .insert_edge(model.edges.len(), "label".to_owned());
            context
        }),
        ("edge order", {
            let mut context = base.clone();
            context.labels.edges.swap(0, 1);
            context
        }),
        ("subgraph range", {
            let mut context = base.clone();
            context.labels.insert_subgraph(
                "G".to_owned(),
                model.subgraphs.len(),
                Some("title".to_owned()),
            );
            context
        }),
        ("subgraph identity", {
            let mut context = base.clone();
            context
                .labels
                .insert_subgraph("G".to_owned(), 1, Some("title".to_owned()));
            context
        }),
        ("CSS owner", {
            let mut context = base.clone();
            context
                .styles
                .insert("G".to_owned(), 1, FlowSubgraphVertexStyle::default());
            context
        }),
    ];
    for (label, context) in cases {
        assert!(
            context
                .validate_for_model_controlled(&model, &OperationControl::new())
                .unwrap()
                .is_err(),
            "{label}"
        );
    }
}

#[test]
fn context_also_validates_the_canonical_css_sidecar() {
    let mut model = model();
    let context = context(&model);
    model.subgraph_vertex_styles.insert(
        "G".to_owned(),
        model.subgraphs.len(),
        FlowSubgraphVertexStyle::default(),
    );
    assert!(
        context
            .validate_for_model_controlled(&model, &OperationControl::new())
            .unwrap()
            .is_err()
    );
}

#[test]
fn collapse_validation_checks_endpoints_without_reconstructing_hidden_ancestry() {
    let mut model = model();
    // Agentflow's projection can omit hidden parent assignments while retaining their identities.
    for subgraph in &mut model.subgraphs {
        subgraph.nodes.clear();
    }
    let mut context = context(&model);
    context.set_collapsed_replacements([
        ("A".to_owned(), "G".to_owned()),
        ("H".to_owned(), "G".to_owned()),
    ]);
    assert_eq!(
        context
            .validate_for_model_controlled(&model, &OperationControl::new())
            .unwrap(),
        Ok(())
    );

    for (id, replacement) in [("missing", "G"), ("A", "missing"), ("G", "G")] {
        let mut invalid = context.clone();
        invalid.set_collapsed_replacements([(id.to_owned(), replacement.to_owned())]);
        assert!(
            invalid
                .validate_for_model_controlled(&model, &OperationControl::new())
                .unwrap()
                .is_err(),
            "{id} -> {replacement}"
        );
    }
    let mut invalid = context.clone();
    invalid.collapsed_subgraphs.insert("missing".to_owned());
    assert!(
        invalid
            .validate_for_model_controlled(&model, &OperationControl::new())
            .unwrap()
            .is_err()
    );
    let mut invalid = context;
    invalid.collapsed_subgraphs.remove("H");
    invalid.set_collapsed_replacements([("A".to_owned(), "H".to_owned())]);
    assert!(
        invalid
            .validate_for_model_controlled(&model, &OperationControl::new())
            .unwrap()
            .is_err()
    );
}

#[test]
fn palette_ordinals_are_unique_in_the_subgraph_domain_but_may_be_sparse() {
    let model = model();
    let mut context = context(&model);
    context.set_subgraph_color_ordinals([("H".to_owned(), 1)]);
    assert_eq!(
        context
            .validate_for_model_controlled(&model, &OperationControl::new())
            .unwrap(),
        Ok(())
    );

    for ordinals in [
        vec![("missing".to_owned(), 0)],
        vec![("G".to_owned(), 2)],
        vec![("G".to_owned(), 0), ("H".to_owned(), 0)],
    ] {
        context.set_subgraph_color_ordinals(ordinals);
        assert!(
            context
                .validate_for_model_controlled(&model, &OperationControl::new())
                .unwrap()
                .is_err()
        );
    }
}

#[test]
fn dom_ordinals_reject_foreign_targets_and_collisions() {
    let model = model();
    let mut context = context(&model);
    for indices in [
        vec![("missing".to_owned(), 0)],
        vec![("A".to_owned(), 7), ("B".to_owned(), 7)],
    ] {
        context.set_node_dom_indices(indices);
        assert!(
            context
                .validate_for_model_controlled(&model, &OperationControl::new())
                .unwrap()
                .is_err()
        );
    }
}

#[test]
fn validation_observes_cancellation_before_invalid_data_and_during_indexing() {
    let mut model = model();
    let mut context = context(&model);
    context
        .labels
        .insert_node("missing".to_owned(), "label".to_owned());
    let control = OperationControl::new();
    control.cancel();
    assert!(
        context
            .validate_for_model_controlled(&model, &control)
            .is_err()
    );
    assert!(
        model
            .subgraph_vertex_styles
            .validate_for_model_controlled(&model, &control)
            .is_err()
    );

    model
        .nodes
        .extend((0..512).map(|index| node(&format!("node-{index}"))));
    let control = OperationControl::new();
    control.cancel_after_checkpoints(128);
    assert!(
        context
            .validate_for_model_controlled(&model, &control)
            .is_err()
    );
}
