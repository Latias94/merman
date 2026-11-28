use super::*;
use crate::test_support::presentation_config;
use merman_model::{
    OperationControl,
    families::usecase::{
        UsecaseArrowType, UsecaseBoundary, UsecaseNode, UsecaseNodeKind, UsecaseRelationship,
        UsecaseRelationshipType,
    },
};
use merman_render_core::{
    resources::{RenderResourcePolicy, ResourceLimitId},
    text::DeterministicTextMeasurer,
};

fn node(id: &str, kind: UsecaseNodeKind, parent: Option<String>) -> UsecaseNode {
    UsecaseNode {
        id: id.into(),
        label: id.into(),
        kind,
        parent_id: parent,
        label_type: Default::default(),
        actor_type: Default::default(),
        icon: None,
        business: false,
        stereotype: None,
        classes: Vec::new(),
        styles: Vec::new(),
    }
}

fn relationship(id: &str, source: &str, target: &str) -> UsecaseRelationship {
    UsecaseRelationship {
        id: id.into(),
        explicit_id: true,
        source: source.into(),
        target: target.into(),
        relationship_type: UsecaseRelationshipType::Association,
        arrow_type: UsecaseArrowType::Point,
        label: Some("review".into()),
        label_type: Some(Default::default()),
        dotted: false,
        minlen: 1,
        classes: Vec::new(),
        styles: Vec::new(),
        animate: false,
        animation: None,
    }
}

fn model() -> UsecaseDiagramModel {
    UsecaseDiagramModel {
        direction: "LR".into(),
        nodes: vec![
            node(
                "Oval",
                UsecaseNodeKind::UseCaseEllipse,
                Some("System".into()),
            ),
            node("User", UsecaseNodeKind::Actor, None),
            node("Box", UsecaseNodeKind::UseCaseRect, Some("System".into())),
        ],
        boundaries: vec![UsecaseBoundary {
            id: "System".into(),
            label: "Package heading".into(),
            label_type: Default::default(),
            members: vec!["Oval".into(), "Box".into()],
            package: true,
            classes: Vec::new(),
            styles: Vec::new(),
        }],
        relationships: vec![
            relationship("review", "User", "Oval"),
            relationship("repeat", "Oval", "Oval"),
            relationship("box", "Oval", "Box"),
        ],
        notes: Vec::new(),
        json_nodes: Vec::new(),
        class_defs: Vec::new(),
        title: None,
        acc_title: None,
        acc_description: None,
    }
}

fn meter() -> Arc<OperationWorkMeter> {
    Arc::new(OperationWorkMeter::new(RenderResourcePolicy::default()))
}

#[test]
fn graph_matches_usecase_db_order_and_layered_spacing_rules() {
    let model = model();
    let config = presentation_config(serde_json::json!({
        "usecase": {"nodeSpacing": 0, "rankSpacing": -1},
        "flowchart": {"nodeSpacing": 81, "rankSpacing": 92, "subGraphTitleMargin": {"top": 7, "bottom": 3}},
        "layered": {"edgeRouting": "curved"}, "htmlLabels": false,
    }));
    let work = meter();
    let css = resolve_css(&config, None, None);
    let mut control = OperationLayoutWorkControl::new(Arc::clone(&work));
    let (nodes, edges) = measure::measure_layered(
        &model,
        &config,
        &css,
        &DeterministicTextMeasurer::default(),
        None,
        &mut control,
    )
    .unwrap();
    let measured = UsecaseMeasured { css, nodes, edges };
    assert_eq!(
        measured
            .nodes
            .iter()
            .map(|node| node.id.as_str())
            .collect::<Vec<_>>(),
        ["User", "Oval", "Box", "System"]
    );
    let (graph, routing, label_y_offset) =
        measured_graph(&model, &config, &measured, &work).unwrap();
    assert_eq!(
        (graph.node_spacing, graph.rank_spacing, label_y_offset),
        (81.0, 92.0, 5.0)
    );
    assert_eq!(routing, EdgeRouting::Curved);
    assert_eq!(
        graph
            .nodes
            .iter()
            .map(|node| node.parent)
            .collect::<Vec<_>>(),
        [None, Some(0), Some(0)]
    );
    assert!(graph.nodes.iter().all(|node| node.port_span.is_none()));
    assert_eq!(graph.groups[0].padding.y0, 22.0);
    let label = measured.edges[0].label.as_ref().unwrap();
    let size = graph.edges[0].label.unwrap();
    assert_eq!(
        (size.width, size.height),
        (label.metrics.width + 12.0, label.metrics.height + 4.0)
    );
    let prepared = prepare_measured(&model, &config, measured, &work).unwrap();
    assert_eq!(
        prepared
            .layout
            .edges
            .iter()
            .map(|edge| edge.id.as_str())
            .collect::<Vec<_>>(),
        ["review", "repeat", "box"]
    );
    assert!(prepared.layout.edges[1].points.len() >= 4);
}

#[test]
fn duplicate_solver_id_releases_the_attempt_and_host_errors_stay_typed() {
    let mut model = model();
    model.relationships[1].id = model.relationships[0].id.clone();
    let config = presentation_config(serde_json::json!({"htmlLabels": true}));
    let font = DeterministicTextMeasurer::default();
    let work = meter();
    let retry = prepare(&model, &config, None, None, &font, None, &work).unwrap_err();
    assert!(matches!(&retry, UsecaseLayeredError::DuplicateEdgeId(id) if id == "review"));
    assert!(retry.is_recoverable());
    assert!(work.prepared_text_retained_bytes_peak() > 0);
    assert_eq!(work.prepared_text_retained_bytes(), 0);
    assert!(work.used() > 0);

    let limits = RenderResourcePolicy::unbounded_for_trusted_input()
        .with_limit(ResourceLimitId::MaxLayoutWorkUnits, 1)
        .unwrap();
    let small = Arc::new(OperationWorkMeter::new(limits));
    let limited = prepare(&model, &config, None, None, &font, None, &small).unwrap_err();
    assert!(
        matches!(&limited, UsecaseLayeredError::Host(Error::ResourceLimitExceeded(limit)) if limit.limit == "max_layout_work_units")
    );
    assert!(!limited.is_recoverable());

    let control = OperationControl::new();
    control.cancel_after_checkpoints(6);
    let cancelled = Arc::new(OperationWorkMeter::new_with_control(
        RenderResourcePolicy::default(),
        control,
    ));
    let interrupted = prepare(&model, &config, None, None, &font, None, &cancelled).unwrap_err();
    assert!(
        matches!(&interrupted, UsecaseLayeredError::Host(Error::Cancelled(error)) if error.phase == OperationPhase::Layout)
    );
    assert!(!interrupted.is_recoverable());
    assert_eq!(cancelled.prepared_text_retained_bytes(), 0);
}
