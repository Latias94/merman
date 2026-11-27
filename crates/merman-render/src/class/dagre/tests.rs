use crate::class::{
    ClassRenderConfig, ClassTextThemePlan, debug_build_class_diagram_dagre_graph,
    layout_class_diagram_typed_with_config,
};
use crate::layout_work::OperationLayoutWorkControl;
use crate::model::ClassNodeLabelPlan;
use crate::text::DeterministicTextMeasurer;
use merman_core::{MermaidEngine, ParseOptions};
use merman_model::presentation::RenderSemanticModel;

#[test]
fn class_dagre_debug_input_uses_the_production_graph_and_source_identity_order() {
    let source =
        include_str!("../../../../../fixtures/class/stress_class_many_relations_labels_020.mmd");
    let parsed = MermaidEngine::new()
        .parse_diagram_for_render_model_sync(source, ParseOptions::default())
        .expect("parse Class fixture")
        .expect("detect Class fixture")
        .into_presentation();
    let RenderSemanticModel::Class(model) = parsed.model() else {
        panic!("expected Class render model");
    };
    let graph = debug_build_class_diagram_dagre_graph(
        model,
        &parsed.metadata().effective_config,
        &DeterministicTextMeasurer::default(),
    )
    .expect("build Class Dagre input");

    assert_eq!(graph.node_ids(), ["A", "B", "C", "D", "E"]);
    assert!(graph.node_ids().into_iter().all(|id| {
        graph
            .node(&id)
            .is_some_and(|node| node.x.is_none() && node.y.is_none())
    }));
    assert_eq!(
        graph
            .edge_keys()
            .into_iter()
            .map(|edge| edge.name.expect("named Class edge"))
            .collect::<Vec<_>>(),
        ["0", "1", "2", "3", "4", "5", "6", "7"]
    );
}

#[test]
fn class_html_prepared_labels_follow_public_layout_and_plan_owners() {
    use crate::resources::{OperationWorkMeter, RenderResourcePolicy};
    use std::sync::Arc;

    let parsed = MermaidEngine::new()
        .parse_diagram_for_render_model_sync(
            "classDiagram\nclass A {\n<<interface>>\n+value: String\n+call()\n}\n",
            ParseOptions::default(),
        )
        .unwrap()
        .unwrap()
        .into_presentation();
    let RenderSemanticModel::Class(model) = parsed.model() else {
        panic!("expected Class model");
    };
    let config = crate::test_support::presentation_config(serde_json::json!({"htmlLabels":true}));
    let typography = ClassTextThemePlan::resolve(None, &config);
    let render_config = ClassRenderConfig::resolve(&config, &typography);
    let meter = Arc::new(OperationWorkMeter::new(
        RenderResourcePolicy::unbounded_for_trusted_input(),
    ));
    let mut work = OperationLayoutWorkControl::new(Arc::clone(&meter));
    let (layout, sidecar) = layout_class_diagram_typed_with_config(
        model,
        &config,
        &DeterministicTextMeasurer::default(),
        None,
        &render_config,
        &mut work,
    )
    .unwrap();
    let retained = meter.prepared_text_retained_bytes();
    assert!(retained > 0);
    let plan = Arc::clone(&layout.class_label_plans_by_id["A"]);
    let ClassNodeLabelPlan::PreparedHtml(labels) = plan.as_ref() else {
        panic!("expected prepared HTML labels");
    };
    assert!(labels.annotation.is_some());
    assert_eq!(labels.members.len(), 1);
    assert_eq!(labels.methods.len(), 1);
    assert!(
        serde_json::to_value(&layout)
            .unwrap()
            .get("class_label_plans_by_id")
            .is_none()
    );

    let cloned_layout = layout.clone();
    assert!(Arc::ptr_eq(
        &layout.class_label_plans_by_id["A"],
        &cloned_layout.class_label_plans_by_id["A"]
    ));
    drop(sidecar);
    drop(work);
    drop(layout);
    assert_eq!(meter.prepared_text_retained_bytes(), retained);
    drop(cloned_layout);
    assert_eq!(meter.prepared_text_retained_bytes(), retained);
    drop(plan);
    assert_eq!(meter.prepared_text_retained_bytes(), 0);
}
