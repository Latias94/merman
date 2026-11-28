#![cfg(all(feature = "diagram-requirement", feature = "layout-layered"))]

use base64::Engine as _;
use merman_core::{MermaidEngine, ParseOptions, presentation::SourcePresentationConfig};
use merman_model::{
    CancelReason, OperationControl, OperationPhase, presentation::PresentationDiagram,
};
use merman_render::{
    DiagramFamilyId, Error, LayoutOptions, RenderCapability,
    environment::{
        HostMeasurementResult, HostTextMeasurementRequest, HostTextMeasurer, MeasurementProfileId,
        RenderEnvironment, RenderSession, TextMeasurementPhase, TextMeasurementPolicy,
        TextMeasurementProfileIdentity,
    },
    family,
    theme::{NativeTheme, ThemeCompiler},
};
use serde_json::{Value, json};
use std::sync::Arc;

const LOOP: &str =
    "requirementDiagram\ndirection LR\nrequirement login {\n  id: 1\n}\nlogin - traces -> login\n";

fn config(routing: &str) -> Value {
    json!({
        "layout": "layered",
        "look": "classic",
        "htmlLabels": false,
        "layered": {"edgeRouting": routing, "lineHops": false}
    })
}

fn parsed(source: &str, config: Value) -> PresentationDiagram {
    MermaidEngine::new()
        .with_site_config(SourcePresentationConfig::from_value(config))
        .parse_diagram_for_render_model_sync(source, ParseOptions::strict())
        .unwrap()
        .unwrap()
        .into_presentation()
}

fn render(diagram: &PresentationDiagram, session: RenderSession) -> String {
    family::prepare(diagram, &LayoutOptions::default(), session)
        .unwrap()
        .render_svg(&Default::default(), &Default::default())
        .unwrap()
        .svg()
        .to_owned()
}

fn relationship<'a, 'input>(doc: &'a roxmltree::Document<'input>) -> roxmltree::Node<'a, 'input> {
    doc.descendants()
        .find(|node| {
            node.attribute("data-id") == Some("login-login-0") && node.has_tag_name("path")
        })
        .expect("Layered self-loop path")
}

#[test]
fn requirement_layered_keeps_relationships_and_their_label_layer_in_source_order() {
    // The Requirement 12.4.0 Layered fixture uses the common renderer and mixed relations.
    let source = r#"---
config:
  layout: layered
  look: classic
  htmlLabels: false
  layered:
    edgeRouting: straight
    lineHops: false
---
requirementDiagram
  requirement login {
    id: 1
    text: Users can sign in.
  }
  functionalRequirement reset {
    id: 2
    text: Users can reset a password.
  }
  element auth_service {
    type: service
  }
  login - contains -> reset
  auth_service - satisfies -> login
  reset - derives -> login
"#;
    let parsed = MermaidEngine::new()
        .parse_diagram_for_render_model_sync(source, ParseOptions::strict())
        .unwrap()
        .unwrap()
        .into_presentation();
    let session = RenderEnvironment::deterministic().begin_session().unwrap();
    let plan = family::plan_render(&parsed, &session).unwrap();
    assert_eq!(plan.family_id(), DiagramFamilyId::REQUIREMENT);
    assert_eq!(
        plan.required_capabilities(),
        [RenderCapability::LayoutLayered]
    );
    let artifact = family::prepare(&parsed, &LayoutOptions::default(), session).unwrap();
    let layout = artifact.layout_geometry_json().unwrap();
    assert_eq!(
        layout["layout"]["RequirementDiagram"]["edges"]
            .as_array()
            .unwrap()
            .len(),
        3
    );
    let svg = artifact
        .render_svg(&Default::default(), &Default::default())
        .unwrap();
    let doc = roxmltree::Document::parse(svg.svg()).unwrap();
    assert_eq!(doc.root_element().attribute("data-layout-fallback"), None);
    let edges = doc
        .descendants()
        .find(|node| node.attribute("class") == Some("edges edgePaths"))
        .unwrap();
    let paths: Vec<_> = edges
        .children()
        .filter(|node| node.has_tag_name("path"))
        .collect();
    assert_eq!(
        paths
            .iter()
            .map(|path| path.attribute("data-id"))
            .collect::<Vec<_>>(),
        [
            Some("login-reset-0"),
            Some("auth_service-login-1"),
            Some("reset-login-2"),
        ]
    );
    assert!(
        paths[0]
            .attribute("marker-start")
            .unwrap()
            .contains("requirement_containsStart")
    );
    assert!(
        paths[1]
            .attribute("marker-end")
            .unwrap()
            .contains("requirement_arrowEnd")
    );
    for path in paths {
        let bytes = base64::engine::general_purpose::STANDARD
            .decode(path.attribute("data-points").unwrap())
            .unwrap();
        let points: Vec<serde_json::Value> = serde_json::from_slice(&bytes).unwrap();
        assert!(points.len() >= 2);
        assert!(
            points
                .iter()
                .all(|point| matches!(point["axis"].as_str(), Some("h" | "v")))
        );
        assert!(!path.attribute("d").unwrap().contains(['A', 'C', 'Q']));
    }
    let labels = doc
        .descendants()
        .find(|node| node.attribute("class") == Some("edgeLabels"))
        .unwrap();
    for relationship in ["<<contains>>", "<<satisfies>>", "<<derives>>"] {
        assert!(
            labels
                .descendants()
                .any(|node| node.text() == Some(relationship))
        );
    }
}

#[test]
fn requirement_layered_curved_routing_reads_the_effective_flowchart_curve() {
    let render_path = |source, routing, curve| {
        let mut site = config(routing);
        site["flowchart"]["curve"] = json!(curve);
        let svg = render(
            &parsed(source, site),
            RenderEnvironment::deterministic().begin_session().unwrap(),
        );
        let doc = roxmltree::Document::parse(&svg).unwrap();
        let edge = relationship(&doc);
        (
            edge.attribute("d").unwrap().to_owned(),
            edge.attribute("data-points").unwrap().to_owned(),
        )
    };
    let (basis, points) = render_path(LOOP, "curved", "basis");
    let (linear, linear_points) = render_path(LOOP, "curved", "linear");
    assert_eq!(points, linear_points);
    assert!(basis.contains('C'));
    assert!(!linear.contains(['A', 'C', 'Q']));
    assert!(render_path(LOOP, "curved", "stepBefore").0.contains('Q'));

    let source_linear = format!("---\nconfig:\n  flowchart:\n    curve: linear\n---\n{LOOP}");
    assert_eq!(render_path(&source_linear, "curved", "basis").0, linear);
    assert!(render_path(LOOP, "rounded", "linear").0.contains('A'));
    assert!(render_path(LOOP, "stepped", "linear").0.contains('Q'));
    assert!(
        !render_path(LOOP, "straight", "basis")
            .0
            .contains(['A', 'C', 'Q'])
    );
}

#[test]
fn requirement_layered_offsets_the_painted_label_and_its_viewport() {
    let positions = |margins| {
        let mut site = config("straight");
        site["flowchart"]["subGraphTitleMargin"] = margins;
        let diagram = parsed(LOOP, site);
        let session = RenderEnvironment::deterministic().begin_session().unwrap();
        let artifact = family::prepare(&diagram, &LayoutOptions::default(), session).unwrap();
        let geometry = artifact.layout_geometry_json().unwrap();
        let center = geometry["layout"]["RequirementDiagram"]["edges"][0]["label"]["y"]
            .as_f64()
            .unwrap();
        let rendered = artifact
            .render_svg(&Default::default(), &Default::default())
            .unwrap();
        let doc = roxmltree::Document::parse(rendered.svg()).unwrap();
        let label_y = doc
            .descendants()
            .find(|node| node.attribute("class") == Some("edgeLabel"))
            .unwrap()
            .attribute("transform")
            .unwrap()
            .strip_suffix(')')
            .unwrap()
            .split_once(", ")
            .unwrap()
            .1
            .parse::<f64>()
            .unwrap();
        let height = doc
            .root_element()
            .attribute("viewBox")
            .unwrap()
            .split_ascii_whitespace()
            .nth(3)
            .unwrap()
            .parse::<f64>()
            .unwrap();
        (center, label_y, height)
    };
    let (default_center, default_y, default_height) = positions(json!({"top": 0, "bottom": 0}));
    let (center, y, height) = positions(json!({"top": 7, "bottom": 3}));
    assert_eq!(center, default_center);
    assert!((y - default_y - 5.0).abs() < 1e-6);
    assert!((height - default_height - 5.0).abs() < 1e-6);
}

#[test]
fn requirement_layered_hand_drawn_edges_use_the_seed_and_keep_native_sketch() {
    let native = ThemeCompiler::new()
        .compile_json(
            br#"{"schema_version":2,"extends":"studio","form":{"edge_style":"sketch"}}"#,
            None,
        )
        .unwrap();
    let edge = |look, seed, sketch| {
        let mut site = config("straight");
        site["look"] = json!(look);
        site["handDrawnSeed"] = json!(seed);
        let env = RenderEnvironment::deterministic();
        let session = if sketch {
            env.begin_session_with_native_theme(NativeTheme::new(native.clone()))
        } else {
            env.begin_session()
        }
        .unwrap();
        let svg = render(&parsed(LOOP, site), session);
        let doc = roxmltree::Document::parse(&svg).unwrap();
        let edge = relationship(&doc);
        assert!(
            edge.attribute("marker-end")
                .unwrap()
                .contains("requirement_arrowEnd")
        );
        (
            edge.attribute("d").unwrap().to_owned(),
            edge.attribute("data-points").unwrap().to_owned(),
        )
    };
    let (classic, points) = edge("classic", 58, false);
    let (drawn, drawn_points) = edge("handDrawn", 58, false);
    assert_eq!(points, drawn_points);
    assert!(!classic.contains('C'));
    assert!(drawn.contains('C'));
    assert_eq!(drawn, edge("handDrawn", 58, false).0);
    assert_ne!(drawn, edge("handDrawn", 59, false).0);

    let (sketch, sketch_points) = edge("classic", 58, true);
    assert!(sketch.contains('C'));
    assert_eq!((sketch, sketch_points), edge("handDrawn", 58, true));
}

#[test]
fn requirement_layered_replays_host_cancellation_before_the_next_invalid_name() {
    struct CancellingHost;
    impl HostTextMeasurer for CancellingHost {
        fn measure(&self, _: HostTextMeasurementRequest<'_>) -> HostMeasurementResult {
            panic!("expected the active control during Requirement measurement");
        }

        fn measure_with_control(
            &self,
            _: HostTextMeasurementRequest<'_>,
            control: &OperationControl,
        ) -> HostMeasurementResult {
            control.cancel();
            Ok(None)
        }
    }

    let diagram = parsed(
        "requirementDiagram\nrequirement Valid {\n}\nrequirement \" \" {\n}\n",
        config("straight"),
    );
    let env = RenderEnvironment::deterministic();
    let ordinary = family::prepare(
        &diagram,
        &LayoutOptions::default(),
        env.begin_session().unwrap(),
    );
    assert!(
        matches!(ordinary, Err(Error::InvalidModel { message }) if message.contains("missing requirement name label"))
    );

    let identity = TextMeasurementProfileIdentity::new(
        MeasurementProfileId::new("test.requirement.cancel").unwrap(),
        "v1",
    )
    .unwrap();
    let session = env
        .with_text_measurement_policy(TextMeasurementPolicy::host_display(
            identity,
            Arc::new(CancellingHost),
            TextMeasurementPhase::ALL,
        ))
        .begin_session()
        .unwrap();
    let result = family::prepare(&diagram, &LayoutOptions::default(), session);
    assert!(
        matches!(result, Err(Error::Cancelled(error)) if error.phase == OperationPhase::Layout && error.reason == CancelReason::Requested)
    );
}

#[test]
fn requirement_layered_path_bounds_checkpoint_reports_emit() {
    let diagram = parsed(&LOOP.replace("  id: 1\n", ""), config("straight"));
    let env = RenderEnvironment::deterministic();
    for checkpoints in 0..1024 {
        let control = OperationControl::new();
        let session = env.begin_session_with_control(control.clone()).unwrap();
        let artifact = family::prepare(&diagram, &LayoutOptions::default(), session).unwrap();
        control.cancel_after_checkpoints(checkpoints);
        match artifact.render_svg(&Default::default(), &Default::default()) {
            Ok(_) => return,
            Err(Error::Cancelled(error)) => assert_eq!(
                error.phase,
                OperationPhase::Emit,
                "checkpoint {checkpoints}"
            ),
            Err(error) => panic!("unexpected error at checkpoint {checkpoints}: {error}"),
        }
    }
    panic!("failed to exhaust the one-edge SVG's emission checkpoints");
}
