use super::*;
use crate::er::ErConfigView;
use merman_model::{
    OperationControl,
    families::er::{ErRelSpec, ErRelationship},
};
use merman_render_core::{
    resources::{RenderResourcePolicy, ResourceLimitId},
    text::{DeterministicTextMeasurer, TextMetrics, TextStyle},
};

pub(super) fn settings(model: &ErDiagramModel, config: &Value) -> ErLayoutSettings {
    let config = ErConfigView::new(config);
    config.layout_settings_with_native_style(
        &model.direction,
        None,
        None,
        config.relationship_html_labels(false),
    )
}

fn relationship(source: &str, target: &str, label: &str) -> ErRelationship {
    ErRelationship {
        entity_a: source.into(),
        entity_b: target.into(),
        role_a: label.into(),
        rel_spec: ErRelSpec {
            card_a: "ZERO_OR_MORE".into(),
            card_b: "ONLY_ONE".into(),
            rel_type: "IDENTIFYING".into(),
        },
    }
}

fn model() -> ErDiagramModel {
    let mut graph = ErDiagramModel {
        direction: "LR".into(),
        subgraphs: vec![
            ErSubgraph {
                id: "Outer".into(),
                title: "Outhouse".into(),
                label_type: "string".into(),
                nodes: vec!["Inner".into()],
                ..Default::default()
            },
            ErSubgraph {
                id: "Inner".into(),
                title: "Annex".into(),
                label_type: "string".into(),
                nodes: vec!["Z".into()],
                ..Default::default()
            },
        ],
        relationships: vec![
            relationship("entity-Z-8", "entity-Z-8", "LoopRole"),
            relationship("entity-Z-8", "entity-A-1", "FirstRole"),
            relationship("entity-Z-8", "entity-A-1", "SecondRole"),
            relationship("Outer", "entity-Z-8", "GroupRole"),
        ],
        ..Default::default()
    };
    for (key, id, label) in [
        ("Outer", "hidden", "HiddenEntity"),
        ("Z", "entity-Z-8", "ZebraEntity"),
        ("A", "entity-A-1", "AppleEntity"),
    ] {
        graph.entities.insert(
            key.into(),
            ErEntity {
                id: id.into(),
                label: label.into(),
                ..Default::default()
            },
        );
    }
    graph
}

#[test]
fn measure_groups_then_visible_tables_and_relationships_and_project_source_slots() {
    use std::sync::Mutex;

    #[derive(Default)]
    struct Recording(Mutex<Vec<String>>);
    impl TextMeasurer for Recording {
        fn measure(&self, text: &str, style: &TextStyle) -> TextMetrics {
            self.0.lock().unwrap().push(text.into());
            DeterministicTextMeasurer::default().measure(text, style)
        }
    }

    let model = model();
    let config = serde_json::json!({"htmlLabels":false, "layered":{"edgeRouting":"stepped"},
        "flowchart":{"nodeSpacing":71, "rankSpacing":91, "subGraphTitleMargin":{"top":7,"bottom":3}},
        "er":{"nodeSpacing":36,"rankSpacing":68}});
    let settings = settings(&model, &config);
    let measurer = Recording::default();
    let work = Arc::new(OperationWorkMeter::new(
        RenderResourcePolicy::unbounded_for_trusted_input(),
    ));
    let labels = ErLayeredLabels::prepare(&model, &config, &measurer, &settings, &work).unwrap();
    let calls = measurer.0.into_inner().unwrap();
    let mut seen = Vec::new();
    for word in [
        "Annex",
        "Outhouse",
        "ZebraEntity",
        "AppleEntity",
        "LoopRole",
        "FirstRole",
        "SecondRole",
        "GroupRole",
    ] {
        seen.push(
            calls
                .iter()
                .position(|text| text.contains(word))
                .unwrap_or_else(|| panic!("missing {word}: {calls:?}")),
        );
    }
    assert!(seen.is_sorted_by(|a, b| a < b), "{calls:?}");
    assert!(!calls.iter().any(|text| text.contains("HiddenEntity")));
    assert!(labels.labels.entity("hidden").is_none());

    let input = Input::new(&model, &config, &settings, &labels, &work).unwrap();
    assert_eq!(
        (input.graph.node_spacing, input.graph.rank_spacing),
        (36.0, 68.0)
    );
    assert_eq!(input.graph.direction, Direction::Right);
    assert_eq!(input.graph.routing, RoutingStyle::Orthogonal);
    assert_eq!(input.graph.nodes[0].parent, Some(0));
    assert_eq!(input.graph.groups[0].parent, Some(1));
    assert_eq!(input.graph.groups[0].padding.y0, 22.0);
    let first = input.relationships[0].1.unwrap();
    assert_eq!(
        input.graph.edges[0].label.unwrap(),
        Size {
            width: first.width + 12.0,
            height: first.height + 4.0
        }
    );
    let layout = input.solve(&work).unwrap();
    assert_eq!(
        layout
            .nodes
            .iter()
            .map(|node| node.id.as_str())
            .collect::<Vec<_>>(),
        ["Outer", "Inner", "entity-Z-8", "entity-A-1"]
    );
    assert_eq!(
        layout
            .edges
            .iter()
            .map(|edge| edge.id.as_str())
            .collect::<Vec<_>>(),
        ["er-rel-0", "er-rel-1", "er-rel-2", "er-rel-3"]
    );
    let paint = layout.paint.routes().unwrap();
    assert_eq!(paint.curve, Curve::Step);
    for (edge, route) in layout.edges.iter().zip(&paint.edges) {
        let label = edge.label.as_ref().unwrap();
        assert_eq!(label.y, route.label.as_ref().unwrap().y + 5.0);
        assert_eq!(edge.start_marker.as_deref(), Some("ONLY_ONE_START"));
        assert_eq!(edge.end_marker.as_deref(), Some("ZERO_OR_MORE_END"));
        for (point, id) in [
            (edge.points.first().unwrap(), &edge.from),
            (edge.points.last().unwrap(), &edge.to),
        ] {
            let node = layout.nodes.iter().find(|node| &node.id == id).unwrap();
            let (dx, dy) = ((point.x - node.x).abs(), (point.y - node.y).abs());
            assert!(dx <= node.width / 2.0 + 1e-8 && dy <= node.height / 2.0 + 1e-8);
            assert!(
                (dx - node.width / 2.0).abs() <= 1e-8 || (dy - node.height / 2.0).abs() <= 1e-8
            );
        }
    }
}

#[test]
fn an_unbound_relationship_from_a_hidden_table_requests_retry() {
    let mut model = model();
    model
        .relationships
        .insert(1, relationship("hidden", "entity-A-1", "UnboundRole"));
    let config = serde_json::json!({"htmlLabels":false});
    let settings = settings(&model, &config);
    let work = Arc::new(OperationWorkMeter::new(
        RenderResourcePolicy::unbounded_for_trusted_input(),
    ));
    let labels = ErLayeredLabels::prepare(
        &model,
        &config,
        &DeterministicTextMeasurer::default(),
        &settings,
        &work,
    )
    .unwrap();
    let input = Input::new(&model, &config, &settings, &labels, &work).unwrap();
    assert_eq!(
        input
            .relationships
            .iter()
            .map(|&(index, _)| index)
            .collect::<Vec<_>>(),
        [0, 2, 3, 4]
    );
    let before_solve = work.used();
    let error = input.solve(&work).unwrap_err();
    assert!(matches!(error, ErLayeredError::UnroutedRelationship(1)) && error.is_recoverable());
    assert!(work.used() > before_solve);
}

#[test]
fn finite_positive_spacing_and_javascript_label_trim() {
    for (value, expected) in [
        (Value::Null, 140.0),
        (serde_json::json!(0), 140.0),
        (serde_json::json!(-1), 50.0),
        (serde_json::json!("45"), 50.0),
        (serde_json::json!(45), 45.0),
    ] {
        assert_eq!(
            spacing(
                &serde_json::json!({"flowchart":{"nodeSpacing":300}, "er":{"nodeSpacing":value}}),
                "nodeSpacing",
                140.0
            ),
            expected
        );
    }
    assert!(edge_label_size(" \u{feff}\r\n", 12.0, 5.0).is_none());
    assert!(edge_label_size("\u{0085}", 12.0, 0.0).is_some());
    assert!(edge_label_size("role", f64::INFINITY, 5.0).is_none());
}

#[test]
fn exact_layout_budget_and_cancelled_attempt_release_labels() {
    let model = model();
    let config = serde_json::json!({"htmlLabels":false});
    let settings = settings(&model, &config);
    let text = DeterministicTextMeasurer::default();
    let prepare = |work: &Arc<OperationWorkMeter>| {
        let labels = ErLayeredLabels::prepare(&model, &config, &text, &settings, work)?;
        layout(&model, &config, &settings, &labels, work)
    };
    let policy = RenderResourcePolicy::unbounded_for_trusted_input();
    let measured = Arc::new(OperationWorkMeter::new(policy));
    let control = prepare(&measured).unwrap();
    let required = measured.used();
    assert_eq!(measured.prepared_text_retained_bytes(), 0);
    for limit in [required, required - 1] {
        let work = Arc::new(OperationWorkMeter::new(
            policy
                .with_limit(ResourceLimitId::MaxLayoutWorkUnits, limit)
                .unwrap(),
        ));
        let outcome = prepare(&work);
        if limit == required {
            let exact = outcome.unwrap();
            assert_eq!(control.edges.len(), exact.edges.len());
            assert_eq!(work.used(), required);
        } else {
            assert!(
                matches!(outcome, Err(ErLayeredError::Host(Error::ResourceLimitExceeded(error))) if error.limit == "max_layout_work_units")
            );
        }
        assert_eq!(work.prepared_text_retained_bytes(), 0);
    }

    struct CancellingRole(OperationControl);
    impl TextMeasurer for CancellingRole {
        fn measure(&self, text: &str, style: &TextStyle) -> TextMetrics {
            if text.contains("LoopRole") {
                self.0.cancel();
            }
            DeterministicTextMeasurer::default().measure(text, style)
        }
    }
    let control = OperationControl::new();
    let work = Arc::new(OperationWorkMeter::new_with_control(
        policy,
        control.clone(),
    ));
    assert!(matches!(
        ErLayeredLabels::prepare(&model, &config, &CancellingRole(control), &settings, &work),
        Err(Error::Cancelled(_))
    ));
    assert!(work.prepared_text_retained_bytes_peak() > 0);
    assert_eq!(work.prepared_text_retained_bytes(), 0);
}
