use merman_model::{
    MermaidConfig, OperationControl,
    families::flowchart::{FlowchartModel, FlowchartRenderContext},
    resources::{InputResourcePolicy, ResourceProfile},
};
use merman_render_core::graph_layout::Endpoint;
use merman_render_core::{
    resources::{RenderResourcePolicy, ResourceLimitId},
    text::DeterministicTextMeasurer,
};
use serde_json::json;

use super::*;

fn fixture() -> (FlowchartModel, FlowchartRenderContext) {
    let model: FlowchartModel = serde_json::from_value(json!({
        "keyword": "flowchart", "direction": "LR",
        "nodes": [
            {"id":"A", "label":"From", "layoutShape":"circle"},
            {"id":"B", "label":"To", "layoutShape":"diamond"},
            {"id":"P", "label":"Peer"},
            {"id":"H", "label":"Hidden"},
            {"id":"J", "label":"Hidden too"}
        ],
        "subgraphs": [
            {"id":"Inner", "title":"Short", "nodes":["B"], "dir":"BT"},
            {"id":"Outer", "title":"The unwrapped group title", "nodes":["Inner"]},
            {"id":"Peer", "title":"", "nodes":["P"]},
            {"id":"Fold", "title":"Folded", "nodes":["H", "J"]}
        ],
        "edges": [
            {"id":"hidden", "from":"H", "to":"J", "length":1},
            {"id":"inside", "from":"Outer", "to":"B", "length":1, "label":"Inner edge"},
            {"id":"redirected", "from":"H", "to":"A", "length":1},
            {"id":"loop", "from":"A", "to":"A", "length":1, "label":"Loop"},
            {"id":"first", "from":"A", "to":"B", "length":2, "label":"Edge title"},
            {"id":"second", "from":"A", "to":"B", "length":1, "label":"\u{feff} "},
            {"id":"invisible", "from":"A", "to":"Peer", "length":1, "stroke":"invisible"},
            {"id":"unbound", "from":"A", "to":"absent", "length":1}
        ]
    }))
    .unwrap();
    let context = FlowchartRenderContext::new(
        Default::default(),
        Default::default(),
        ["Fold".to_owned()].into_iter().collect(),
        &model.subgraphs,
    );
    (model, context)
}

fn config() -> PresentationConfig {
    let values = MermaidConfig::from_value(json!({
        "htmlLabels": false,
        "flowchart": {"wrappingWidth": 32, "subGraphTitleMargin": {"top": 3, "bottom": 7}},
        "layered": {"edgeRouting": "stepped"}
    }));
    PresentationConfig::from_config_controlled(
        values,
        &InputResourcePolicy::for_profile(ResourceProfile::UnboundedForTrustedInput),
        &OperationControl::default(),
    )
    .unwrap()
    .unwrap()
}

fn run(
    model: &FlowchartModel,
    context: &FlowchartRenderContext,
    work: &OperationWorkMeter,
) -> Result<FlowLayeredLayout, FlowLayeredError> {
    run_with_labels(model, context, None, work)
}

fn run_with_labels(
    model: &FlowchartModel,
    context: &FlowchartRenderContext,
    labels: Option<&FlowchartSvgLabelSidecarBuilder>,
    work: &OperationWorkMeter,
) -> Result<FlowLayeredLayout, FlowLayeredError> {
    let config = config();
    let classes = FlowchartClassDefinitions::new(&model.class_defs);
    let styles = FlowchartEdgeStylePlan::prepare_with_class_definitions(
        model, &config, false, &classes, work,
    )?;
    layout(
        FlowchartRenderModelRef::new(model, context),
        &classes,
        &config,
        &DeterministicTextMeasurer::default(),
        None,
        labels,
        &styles,
        work,
    )
}

#[test]
fn measured_slots_keep_collapse_routes_labels_and_dom_order() {
    let (model, context) = fixture();
    let meter = OperationWorkMeter::new(RenderResourcePolicy::trusted_native());
    let placed = run(&model, &context, &meter).unwrap();
    assert_eq!(
        placed.dom_node_order_by_root[""],
        ["Peer", "Outer", "Inner", "Fold", "A", "B", "P"]
    );
    assert_eq!(placed.routes.routing, FlowLayeredRouting::Stepped);
    assert_eq!(placed.routes.group_parents, [None, None, Some(1)]);
    assert_eq!(
        placed
            .nodes
            .iter()
            .map(|node| node.id.as_str())
            .collect::<Vec<_>>(),
        ["Fold", "A", "B", "P", "Peer", "Outer", "Inner"]
    );
    assert_eq!(
        placed
            .edge_owners
            .iter()
            .map(|key| key.semantic_index())
            .collect::<Vec<_>>(),
        [1, 2, 3, 4, 5, 6]
    );
    assert_eq!(placed.edges[0].id, "inside");
    assert_eq!(placed.edges[0].from_cluster.as_deref(), Some("Outer"));
    assert_eq!(placed.edges[1].from, "Fold");
    assert_eq!(placed.edges[2].from, placed.edges[2].to);
    assert_eq!(placed.routes.edges[2].source, Endpoint::Node(1));
    assert_eq!(placed.routes.edges[2].points.len(), 4);
    assert!(placed.edges[4].label.is_none());
    assert_eq!(placed.edges[5].id, "invisible");
    assert!(placed.clusters[1].title_label.width > 32.0);
    assert_eq!(placed.clusters[1].title_margin_top, 3.0);

    for (edge, route) in placed.edges.iter().zip(&placed.routes.edges) {
        assert_eq!(edge.points.len(), route.points.len());
        for (xy, axis) in edge.points.iter().zip(&route.points) {
            assert_eq!(
                (xy.x.to_bits(), xy.y.to_bits()),
                (axis.x.to_bits(), axis.y.to_bits())
            );
        }
        if let Some(label) = &edge.label {
            let reserved = route.label.as_ref().unwrap();
            assert_eq!((label.x, label.y), (reserved.x, reserved.y));
            assert_eq!(reserved.width, label.width + 12.0);
            assert_eq!(reserved.height, label.height + 4.0);
        }
    }
}

#[test]
fn input_failures_are_typed_and_do_not_terminate_the_meter() {
    let (mut model, context) = fixture();
    let meter = OperationWorkMeter::new(RenderResourcePolicy::trusted_native());
    model.edges[4].id = "inside".to_owned();
    let error = run(&model, &context, &meter).unwrap_err();
    assert!(matches!(&error, FlowLayeredError::DuplicateEdgeId { id } if id == "inside"));
    assert!(error.is_recoverable());
    assert!(meter.checkpoint(OperationPhase::Layout).is_ok());

    model.subgraphs[0].nodes.push("Outer".to_owned());
    let error = run(&model, &context, &meter).unwrap_err();
    assert!(matches!(error, FlowLayeredError::InvalidNesting { .. }));
    assert!(error.is_recoverable());
    assert!(meter.checkpoint(OperationPhase::Layout).is_ok());
}

#[test]
fn adapter_and_solver_charge_the_same_meter_and_keep_terminal_errors() {
    let (model, context) = fixture();
    let policy = RenderResourcePolicy::trusted_native();
    let baseline = OperationWorkMeter::new(policy);
    baseline.charge(17).unwrap();
    run(&model, &context, &baseline).unwrap();
    let total = baseline.used();
    for limit in [total, total - 1] {
        let meter = OperationWorkMeter::new(
            policy
                .with_limit(ResourceLimitId::MaxLayoutWorkUnits, limit)
                .unwrap(),
        );
        meter.charge(17).unwrap();
        let result = run(&model, &context, &meter);
        if limit == total {
            assert!(result.is_ok());
            assert_eq!(meter.used(), limit);
        } else {
            let error = result.unwrap_err();
            assert!(matches!(
                error,
                FlowLayeredError::Host(Error::ResourceLimitExceeded(_))
            ));
            assert!(!error.is_recoverable());
            assert!(meter.checkpoint(OperationPhase::Layout).is_err());
        }
    }
    let cancelled = OperationWorkMeter::new(policy);
    cancelled.control().cancel();
    assert!(matches!(
        run(&model, &context, &cancelled),
        Err(FlowLayeredError::Host(Error::Cancelled(_)))
    ));
}

#[test]
fn sidecar_resource_failure_precedes_a_recoverable_layout_failure() {
    let (mut model, context) = fixture();
    model.edges[4].id = "inside".to_owned();
    let work = std::sync::Arc::new(OperationWorkMeter::new(
        RenderResourcePolicy::trusted_native()
            .with_limit(ResourceLimitId::MaxPreparedTextRetainedBytes, 1)
            .unwrap(),
    ));
    let labels = FlowchartSvgLabelSidecarBuilder::new_with_work_meter(work.clone());
    let error = run_with_labels(&model, &context, Some(&labels), &work).unwrap_err();
    assert!(!error.is_recoverable());
    assert!(
        matches!(error, FlowLayeredError::Host(Error::ResourceLimitExceeded(resource))
        if resource.limit == "max_prepared_text_retained_bytes")
    );
    drop(labels);
    assert_eq!(work.prepared_text_retained_bytes(), 0);
}
