use super::{ErDagreLayout, layout};
use crate::er::{ErConfigView, ErPreparedLabels};
use merman_core::{MermaidEngine, ParseOptions};
use merman_model::MermaidConfig;
use merman_model::families::er::{ErDiagramModel, ErEntity, ErRelationship, ErSubgraph};
use merman_model::presentation::RenderSemanticModel;
use merman_render_core::layout_work::OperationLayoutWorkControl;
use merman_render_core::resources::{OperationWorkMeter, RenderResourcePolicy};
use merman_render_core::text::DeterministicTextMeasurer;
use merman_render_core::{Error, Result};
use serde_json::Value;
use std::sync::Arc;

fn solve(model: &ErDiagramModel, config: &Value) -> Result<ErDagreLayout> {
    let settings = ErConfigView::new(config).layout_settings_for_test(&model.direction, false);
    let measurer = DeterministicTextMeasurer::default();
    let meter = Arc::new(OperationWorkMeter::new(
        RenderResourcePolicy::unbounded_for_trusted_input(),
    ));
    let mut work = OperationLayoutWorkControl::new(Arc::clone(&meter));
    let config = MermaidConfig::from_value(config.clone());
    let labels =
        ErPreparedLabels::prepare_for_layout(model, &measurer, &settings, &config, false, &meter)?;
    layout(model, &measurer, settings, &labels, &mut work)
}

fn er_cyclic_compound_model() -> ErDiagramModel {
    ErDiagramModel {
        direction: "TB".to_string(),
        subgraphs: [("A", "B"), ("B", "A")]
            .into_iter()
            .map(|(id, child)| ErSubgraph {
                id: id.to_string(),
                title: "Repeated label".to_string(),
                nodes: vec![child.to_string()],
                ..Default::default()
            })
            .collect(),
        ..Default::default()
    }
}

#[test]
fn er_direct_model_rejects_compound_and_helper_parent_cycles() {
    let helper_collision = ErDiagramModel {
        direction: "TB".to_string(),
        entities: [(
            "A".to_string(),
            ErEntity {
                id: "entity-A-0".to_string(),
                label: "A".to_string(),
                ..Default::default()
            },
        )]
        .into_iter()
        .collect(),
        subgraphs: vec![ErSubgraph {
            id: "entity-A-0---entity-A-0---1".to_string(),
            title: "Group".to_string(),
            nodes: vec!["A".to_string()],
            ..Default::default()
        }],
        relationships: vec![ErRelationship {
            entity_a: "entity-A-0".to_string(),
            entity_b: "entity-A-0".to_string(),
            ..Default::default()
        }],
        ..Default::default()
    };
    for model in [er_cyclic_compound_model(), helper_collision] {
        let error = solve(&model, &Value::Null)
            .expect_err("external ER containment cycle must return an error");
        let Error::InvalidModel { message } = error else {
            panic!("expected InvalidModel, got {error:?}");
        };
        assert!(message.contains("ER parent cycle"), "{message}");
    }
}

#[test]
fn er_direct_model_preserves_membership_overwrites_and_child_order() {
    let model = ErDiagramModel {
        direction: "TB".to_string(),
        entities: ["ONE", "TWO", "THREE"]
            .into_iter()
            .enumerate()
            .map(|(index, name)| {
                (
                    name.to_string(),
                    ErEntity {
                        id: format!("entity-{name}-{index}"),
                        label: "Repeated label".to_string(),
                        ..Default::default()
                    },
                )
            })
            .collect(),
        subgraphs: [
            ("Left", vec!["ONE", "TWO", "ONE", "THREE"]),
            ("Right", vec!["THREE"]),
        ]
        .into_iter()
        .map(|(id, nodes)| ErSubgraph {
            id: id.to_string(),
            title: "Repeated label".to_string(),
            nodes: nodes.into_iter().map(str::to_string).collect(),
            ..Default::default()
        })
        .collect(),
        ..Default::default()
    };
    let mut final_membership = model.clone();
    final_membership.subgraphs[0].nodes = vec!["TWO".to_string(), "ONE".to_string()];
    let project = |model| {
        let layout = solve(model, &Value::Null).expect("repeated ER memberships remain valid");
        serde_json::json!({
            "nodes": layout.nodes,
            "edges": layout.edges,
            "render_edges": layout.render_edges,
            "clusters": layout.clusters,
            "bounds": layout.bounds,
        })
    };
    assert_eq!(project(&model), project(&final_membership));
}

#[test]
fn er_dagre_recursive_relationship_keeps_internal_helper_ranks() {
    // Recursive ER fixture with the facade test's explicit Dagre selection.
    let source = r#"---
config:
  layout: dagre
  logLevel: 1.0
---

erDiagram
    CUSTOMER ||..o{ CUSTOMER : refers
    CUSTOMER ||--o{ ORDER : places
    ORDER ||--|{ LINE-ITEM : contains
"#;

    let parsed = MermaidEngine::new()
        .parse_diagram_for_render_model_sync(source, ParseOptions::default())
        .expect("parse ok")
        .expect("diagram detected")
        .into_presentation();
    let RenderSemanticModel::Er(model) = parsed.model() else {
        panic!("expected ER model");
    };
    let layout =
        solve(model, parsed.metadata().effective_config.as_value()).expect("ER Dagre layout");
    let node = |id: &str| {
        layout
            .nodes
            .iter()
            .find(|node| node.id == id)
            .unwrap_or_else(|| panic!("missing ER layout node {id}"))
    };
    let customer = node("entity-CUSTOMER-0");
    let helper_1 = node("entity-CUSTOMER-0---entity-CUSTOMER-0---1");
    let helper_2 = node("entity-CUSTOMER-0---entity-CUSTOMER-0---2");
    let order = node("entity-ORDER-1");
    let line_item = node("entity-LINE-ITEM-2");

    assert!(customer.y < helper_1.y && helper_1.y < helper_2.y);
    assert!((helper_1.y - order.y).abs() < 1e-9);
    assert!((helper_2.y - line_item.y).abs() < 1e-9);

    let loop_middle = layout
        .edges
        .iter()
        .find(|edge| edge.id == "er-rel-0")
        .expect("recursive relationship middle segment");
    assert_eq!(
        layout
            .edges
            .iter()
            .filter(|edge| edge.id.starts_with("er-rel-0"))
            .count(),
        3,
    );
    assert_eq!(loop_middle.from, helper_1.id);
    assert_eq!(loop_middle.to, helper_2.id);
    assert!(loop_middle.label.is_some());
    assert!(customer.y < order.y && order.y < line_item.y);

    let [first, ..] = layout.render_edges.as_slice() else {
        panic!("missing merged visual self-loop");
    };
    assert_eq!(layout.render_edges.len(), model.relationships.len());
    assert_eq!(first.id, "er-rel-0");
    assert_eq!(first.from, customer.id);
    assert_eq!(first.to, customer.id);
    assert!(first.label.is_some());
}
