use merman_core::{MermaidEngine, ParseOptions};
use merman_model::families::er::{ErDiagramModel, ErEntity, ErRelationship, ErSubgraph};
use merman_model::presentation::{
    AuthoredPresentationMetadata, PresentationContext, PresentationDiagram, RenderSemanticModel,
};
use merman_model::resources::InputResourcePolicy;
use merman_model::{MermaidConfig, OperationControl, OperationPhase};
use merman_render::LayoutOptions;
use merman_render::environment::RenderEnvironment;
use merman_render::family;
use merman_render::model::ErDiagramLayout;
use std::path::PathBuf;

fn workspace_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("..")
}

fn layout_er(text: &str) -> ErDiagramLayout {
    let parsed = MermaidEngine::new()
        .parse_diagram_for_render_model_sync(text, ParseOptions::default())
        .expect("parse ok")
        .expect("diagram detected")
        .into_presentation();
    let session = RenderEnvironment::deterministic().begin_session().unwrap();
    let artifact = family::prepare(&parsed, &LayoutOptions::default(), session).expect("ER layout");
    let projection = artifact.layout_json().expect("serialize ER layout");
    serde_json::from_value(projection["layout"]["ErDiagram"].clone()).expect("ER layout projection")
}

fn er_presentation_from_model(model: ErDiagramModel) -> PresentationDiagram {
    let config = MermaidConfig::from_value(serde_json::json!({"layout": "dagre"}));
    PresentationDiagram::try_new(
        AuthoredPresentationMetadata {
            diagram_type: "erDiagram".to_string(),
            config: config.clone(),
            effective_config: config,
            title: None,
            theme_recipe: None,
        },
        RenderSemanticModel::Er(model),
        PresentationContext::None,
        &InputResourcePolicy::default(),
        &OperationControl::new(),
    )
    .expect("construct ER presentation")
    .expect("admit ER presentation data")
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
        let presentation = er_presentation_from_model(model);
        let session = RenderEnvironment::deterministic().begin_session().unwrap();
        let error = family::prepare(&presentation, &LayoutOptions::default(), session)
            .err()
            .expect("external ER containment cycle must return an error");
        let merman_render::Error::InvalidModel { message } = error else {
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
        let presentation = er_presentation_from_model(model);
        let session = RenderEnvironment::deterministic().begin_session().unwrap();
        let artifact = family::prepare(&presentation, &LayoutOptions::default(), session)
            .expect("repeated ER memberships remain valid");
        artifact.layout_json().unwrap()["layout"]["ErDiagram"].clone()
    };
    assert_eq!(project(model), project(final_membership));
}

#[test]
fn er_direct_model_parent_admission_preserves_operation_terminals() {
    use merman_render::environment::{
        HostMeasurementResult, HostTextMeasurementRequest, HostTextMeasurer, MeasurementProfileId,
        TextMeasurementPhase, TextMeasurementPolicy, TextMeasurementProfileIdentity,
    };
    use std::sync::Arc;

    struct CancellingHost;
    impl HostTextMeasurer for CancellingHost {
        fn measure(&self, _request: HostTextMeasurementRequest<'_>) -> HostMeasurementResult {
            Ok(None)
        }

        fn measure_with_control(
            &self,
            _request: HostTextMeasurementRequest<'_>,
            control: &OperationControl,
        ) -> HostMeasurementResult {
            control.cancel();
            Ok(None)
        }
    }

    let presentation = er_presentation_from_model(er_cyclic_compound_model());
    let control = OperationControl::new();
    let identity = TextMeasurementProfileIdentity::new(
        MeasurementProfileId::new("test.er-parent-cancellation").unwrap(),
        "v1",
    )
    .unwrap();
    let session = RenderEnvironment::deterministic()
        .with_text_measurement_policy(TextMeasurementPolicy::host_display(
            identity,
            Arc::new(CancellingHost),
            TextMeasurementPhase::ALL,
        ))
        .begin_session_with_control(control.clone())
        .unwrap();
    assert!(
        matches!(
            family::prepare(&presentation, &LayoutOptions::default(), session),
            Err(merman_render::Error::Cancelled(_)),
        ),
        "cancellation during group label measurement must precede the cycle error"
    );

    for cancel_first in [true, false] {
        let control = OperationControl::new();
        let policy = merman_render::RenderResourcePolicy::interactive()
            .with_limit(merman_render::ResourceLimitId::MaxLayoutWorkUnits, 1)
            .unwrap();
        let session = RenderEnvironment::deterministic()
            .with_resource_policy(policy)
            .begin_session_with_control(control.clone())
            .unwrap();
        if cancel_first {
            control.cancel();
        }
        let error = family::prepare(&presentation, &LayoutOptions::default(), session)
            .err()
            .expect("operation terminal must precede ER structural rejection");
        if cancel_first {
            assert!(matches!(error, merman_render::Error::Cancelled(_)));
        } else {
            let merman_render::Error::ResourceLimitExceeded(limit) = error else {
                panic!("expected adapter resource rejection, got {error:?}");
            };
            assert_eq!(limit.limit, "max_layout_work_units");
            assert!(limit.actual > limit.max);
        }
        let terminal = control
            .terminal_checkpoint_at(OperationPhase::Layout)
            .unwrap_err();
        control.cancel();
        assert_eq!(
            control
                .terminal_checkpoint_at(OperationPhase::Layout)
                .unwrap_err(),
            terminal,
            "later cancellation must preserve the first operation terminal",
        );
    }
}

#[test]
fn er_layout_produces_positions_and_routes() {
    let path = workspace_root()
        .join("fixtures")
        .join("er")
        .join("basic.mmd");
    let text = std::fs::read_to_string(&path).expect("fixture");

    let layout = layout_er(&text);

    assert!(layout.nodes.len() >= 3);
    assert!(layout.edges.len() >= 2);

    for n in &layout.nodes {
        assert!(n.width.is_finite() && n.width > 0.0);
        assert!(n.height.is_finite() && n.height > 0.0);
        assert!(n.x.is_finite() && n.y.is_finite());
    }

    for e in &layout.edges {
        assert!(
            e.points.len() >= 2,
            "edge {} should have at least two points",
            e.id
        );
        for p in &e.points {
            assert!(p.x.is_finite() && p.y.is_finite());
        }
    }
}

#[test]
fn er_layout_emits_markers_and_dashes_from_rel_spec() {
    let path = workspace_root()
        .join("fixtures")
        .join("er")
        .join("upstream_relationship_aliases.mmd");
    let text = std::fs::read_to_string(&path).expect("fixture");

    let layout = layout_er(&text);

    let mut has_marker = false;
    let mut has_dashed = false;
    for e in &layout.edges {
        if e.start_marker.is_some() || e.end_marker.is_some() {
            has_marker = true;
        }
        if e.stroke_dasharray.as_deref() == Some("8,8") {
            has_dashed = true;
        }
    }

    assert!(has_marker, "expected at least one edge to have ER markers");
    assert!(
        has_dashed,
        "expected at least one NON_IDENTIFYING relationship to be dashed"
    );
}

#[test]
fn er_dagre_recursive_relationship_keeps_internal_helper_ranks() {
    let path = workspace_root()
        .join("fixtures")
        .join("er")
        .join(
            "upstream_cypress_erdiagram_spec_should_render_an_er_diagram_with_a_recursive_relationship_002.mmd",
        );
    let text = std::fs::read_to_string(&path).expect("fixture");
    let text = text.replacen("  logLevel: 1.0", "  layout: dagre\n  logLevel: 1.0", 1);

    let layout = layout_er(&text);
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
}

#[test]
fn er_layout_materializes_compound_subgraphs() {
    let text = concat!(
        "erDiagram\n",
        "subgraph Orders [Order Domain]\n",
        "  CUSTOMER ||--o{ ORDER : places\n",
        "end\n",
        "WAREHOUSE ||--o{ Orders : ships\n",
    );

    let layout = layout_er(text);
    println!("{layout:#?}");
    assert_eq!(layout.clusters.len(), 1);
    let cluster = &layout.clusters[0];
    assert_eq!(cluster.id, "Orders");
    assert!(cluster.width.is_finite() && cluster.width > 0.0);
    assert!(cluster.height.is_finite() && cluster.height > 0.0);
    assert!(
        layout
            .nodes
            .iter()
            .any(|node| node.id == "Orders" && node.is_cluster)
    );
    assert!(layout.edges.iter().any(|edge| edge.to == "Orders"));
}

#[cfg(feature = "layout-elk")]
#[test]
fn er_layout_config_selects_source_ported_elk_geometry() {
    let elk_source = r#"---
config:
  layout: elk
---
erDiagram
  CUSTOMER ||--o{ ORDER : places
  ORDER ||--|{ LINE-ITEM : contains
  CUSTOMER }|..|{ DELIVERY-ADDRESS : uses
"#;
    let dagre_source = r#"---
config:
  layout: dagre
---
erDiagram
  CUSTOMER ||--o{ ORDER : places
  ORDER ||--|{ LINE-ITEM : contains
  CUSTOMER }|..|{ DELIVERY-ADDRESS : uses
"#;

    let elk = layout_er(elk_source);
    let dagre = layout_er(dagre_source);

    let elk_positions = elk
        .nodes
        .iter()
        .map(|node| (node.id.as_str(), node.x, node.y))
        .collect::<Vec<_>>();
    let dagre_positions = dagre
        .nodes
        .iter()
        .map(|node| (node.id.as_str(), node.x, node.y))
        .collect::<Vec<_>>();
    assert_ne!(elk_positions, dagre_positions);
    assert!(elk.edges.iter().all(|edge| {
        edge.points.windows(2).all(|segment| {
            (segment[0].x - segment[1].x).abs() < 1e-9 || (segment[0].y - segment[1].y).abs() < 1e-9
        })
    }));
}

#[cfg(feature = "layout-elk")]
#[test]
fn er_elk_layout_materializes_compound_subgraphs_and_svg_groups() {
    let text = concat!(
        "---\nconfig:\n  layout: elk\n---\n",
        "erDiagram\n",
        "subgraph Orders [Order Domain]\n",
        "  CUSTOMER ||--o{ ORDER : places\n",
        "end\n",
        "WAREHOUSE ||--o{ Orders : ships\n",
    );
    let parsed = MermaidEngine::new()
        .parse_diagram_for_render_model_sync(text, ParseOptions::default())
        .expect("parse ok")
        .expect("diagram detected")
        .into_presentation();
    let session = RenderEnvironment::deterministic().begin_session().unwrap();
    let artifact = family::prepare(&parsed, &LayoutOptions::default(), session).expect("ER layout");
    let projection = artifact.layout_json().expect("serialize ER layout");
    let layout: ErDiagramLayout =
        serde_json::from_value(projection["layout"]["ErDiagram"].clone()).expect("layout");
    assert_eq!(layout.clusters.len(), 1);
    assert!(
        layout
            .nodes
            .iter()
            .any(|node| node.id == "Orders" && node.is_cluster)
    );
    assert!(layout.edges.iter().any(|edge| edge.to == "Orders"));

    let svg = artifact
        .render_svg(
            &merman_render::svg::SvgRenderOptions {
                diagram_id: Some("er-elk-subgraph".to_string()),
                ..Default::default()
            },
            &merman_render::svg::SvgDebugOptions::default(),
        )
        .expect("render ER SVG")
        .svg()
        .to_owned();
    assert!(svg.contains(r#"class="clusters""#), "{svg}");
    assert!(svg.contains("er-elk-subgraph-Orders"), "{svg}");
    assert!(svg.contains("Order Domain"), "{svg}");
}
