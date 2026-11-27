use merman_core::{MermaidEngine, ParseOptions};
use merman_model::families::er::{ErDiagramModel, ErSubgraph};
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

#[cfg(feature = "layout-shoal")]
#[test]
fn er_shoal_gives_cardinality_markers_room_on_a_crowded_face() {
    for direction in ["LR", "RL", "TB", "BT"] {
        let source = format!(
            "---\nconfig:\n  layout: shoal\n  look: classic\n  htmlLabels: false\n---\nerDiagram\n  direction {direction}\n  A }}|--|{{ B : first\n  A }}|--|{{ B : second\n  A }}|--|{{ B : third\n  A }}|--|{{ B : fourth\n  A }}|--|{{ B : fifth\n"
        );
        let layout = layout_er(&source);
        let horizontal = matches!(direction, "LR" | "RL");
        for node in &layout.nodes {
            let length = if horizontal { node.height } else { node.width };
            let mut ports = layout
                .edges
                .iter()
                .map(|edge| {
                    let point = if edge.from == node.id {
                        edge.points.first().unwrap()
                    } else {
                        edge.points.last().unwrap()
                    };
                    if horizontal { point.y } else { point.x }
                })
                .collect::<Vec<_>>();
            ports.sort_by(f64::total_cmp);
            assert_eq!(ports.len(), 5);
            assert!(
                ports.windows(2).all(|pair| pair[1] - pair[0] >= 18.0),
                "{direction} {}: {ports:?}",
                node.id
            );
            assert!(length >= 5.0 * 18.0, "{direction} {}: {length}", node.id);
        }
    }
}

#[cfg(feature = "layout-shoal")]
#[test]
fn er_shoal_uses_the_largest_face_and_checks_reversed_assignments() {
    for direction in ["LR", "RL", "TB", "BT"] {
        let layout = |body: &str| {
            layout_er(&format!(
                "---\nconfig:\n  layout: shoal\n  htmlLabels: false\n---\nerDiagram\ndirection {direction}\n{body}"
            ))
        };
        let horizontal = matches!(direction, "LR" | "RL");
        let face_length = |node: &merman_render::model::LayoutNode| {
            if horizontal { node.height } else { node.width }
        };
        let chain = layout(
            "A }|--|{ B : ab1\nA }|--|{ B : ab2\nA }|--|{ B : ab3\nB }|--|{ C : bc1\nB }|--|{ C : bc2\nB }|--|{ C : bc3\n",
        );
        let [a, b, c] = chain.nodes.as_slice() else {
            panic!("three chain entities")
        };
        assert!(
            face_length(b) <= face_length(a).max(face_length(c)) + 0.01,
            "{direction}: opposite faces share the same reserve"
        );

        let cycle = layout(
            "A }|--|{ B : ab1\nA }|--|{ B : ab2\nA }|--|{ B : ab3\nB }|--|{ A : ba1\nB }|--|{ A : ba2\n",
        );
        for node in &cycle.nodes {
            let mut faces = [Vec::new(), Vec::new()];
            for edge in &cycle.edges {
                let point = if edge.from == node.id {
                    edge.points.first()
                } else {
                    edge.points.last()
                }
                .unwrap();
                let (cross, rank, center) = if horizontal {
                    (point.y, point.x, node.x)
                } else {
                    (point.x, point.y, node.y)
                };
                faces[usize::from(rank > center)].push(cross);
            }
            assert!(
                faces.iter().any(|face| face.len() > 3),
                "{direction}: backedges must exercise the shared face"
            );
            for face in &mut faces {
                face.sort_by(f64::total_cmp);
                assert!(
                    face.windows(2).all(|pair| pair[1] - pair[0] >= 18.0),
                    "{direction} {}: {face:?}",
                    node.id
                );
            }
        }
    }
}

#[cfg(feature = "layout-shoal")]
#[test]
fn er_shoal_reserves_the_physical_axis_of_a_nested_direction() {
    let layout = layout_er(concat!(
        "---\nconfig:\n  layout: shoal\n  htmlLabels: false\n---\nerDiagram\n",
        "direction LR\nsubgraph Inner [Inner]\ndirection BT\n",
        "A }|--|{ B : first\nA }|--|{ B : second\nA }|--|{ B : third\n",
        "A }|--|{ B : fourth\nA }|--|{ B : fifth\nend\n",
    ));
    assert_eq!(layout.clusters[0].effective_dir, "BT");
    for node in layout.nodes.iter().filter(|node| !node.is_cluster) {
        assert!(
            node.width >= 5.0 * 18.0 && node.height < node.width,
            "{node:?}"
        );
        let mut contacts = Vec::new();
        for edge in &layout.edges {
            let point = if edge.from == node.id {
                edge.points.first()
            } else {
                edge.points.last()
            }
            .unwrap();
            assert!(((point.y - node.y).abs() - node.height / 2.0).abs() < 1e-6);
            contacts.push(point.x);
        }
        contacts.sort_by(f64::total_cmp);
        assert!(contacts.windows(2).all(|pair| pair[1] - pair[0] >= 18.0));
    }
}

#[cfg(not(feature = "layout-shoal"))]
#[test]
fn er_shoal_reports_the_missing_build_capability() {
    let source = "---\nconfig:\n  layout: shoal\n---\nerDiagram\nA ||--o{ B : relates\n";
    let parsed = MermaidEngine::new()
        .parse_diagram_for_render_model_sync(source, ParseOptions::strict())
        .unwrap()
        .unwrap()
        .into_presentation();
    let session = RenderEnvironment::deterministic().begin_session().unwrap();
    let plan = family::plan_render(&parsed, &session).unwrap();
    assert_eq!(
        plan.required_capabilities(),
        &[merman_render::RenderCapability::LayoutShoal]
    );
    assert!(!plan.is_ready());
    assert!(matches!(
        family::prepare(&parsed, &LayoutOptions::default(), session),
        Err(merman_render::Error::MissingCapability {
            capability: merman_render::RenderCapability::LayoutShoal,
            ..
        })
    ));
}

fn er_presentation_from_model(model: ErDiagramModel) -> PresentationDiagram {
    let config = MermaidConfig::from_value(serde_json::json!({"layout": "dagre"}));
    PresentationDiagram::try_new(
        AuthoredPresentationMetadata {
            diagram_type: "erDiagram".to_string(),
            config: config.clone(),
            effective_config: config,
            title: None,
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
fn er_numeric_zero_spacing_uses_defaults_but_string_zero_remains_explicit() {
    let source = "erDiagram\nA ||--|| B : ab\nA ||--|| C : ac\n";
    let defaults = layout_er(&format!("---\nconfig:\n  layout: dagre\n---\n{source}"));
    let numeric_zero = layout_er(&format!(
        "---\nconfig:\n  layout: dagre\n  er:\n    nodeSpacing: 0\n    rankSpacing: 0\n---\n{source}"
    ));
    let string_zero = layout_er(&format!(
        "---\nconfig:\n  layout: dagre\n  er:\n    nodeSpacing: \"0\"\n    rankSpacing: \"0\"\n---\n{source}"
    ));
    let positions = |layout: &ErDiagramLayout| {
        layout
            .nodes
            .iter()
            .map(|node| (node.id.clone(), node.x, node.y))
            .collect::<Vec<_>>()
    };
    assert_eq!(positions(&numeric_zero), positions(&defaults));
    assert_ne!(positions(&string_zero), positions(&defaults));
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
