use merman_core::{Engine, ParseOptions};
use merman_render::model::{ArchitectureDiagramLayout, LayoutDiagram};
use merman_render::{LayoutOptions, layout_parsed_render_layout_only};

fn layout_architecture(text: &str) -> ArchitectureDiagramLayout {
    let engine = Engine::new();
    let parsed = engine
        .parse_diagram_for_render_model_sync(text, ParseOptions::strict())
        .expect("parse ok")
        .expect("diagram detected");
    let layout = layout_parsed_render_layout_only(&parsed, &LayoutOptions::headless_svg_defaults())
        .expect("layout ok");
    match layout {
        LayoutDiagram::ArchitectureDiagram(layout) => *layout,
        other => panic!("expected architecture layout, got {other:?}"),
    }
}

fn node_center(layout: &ArchitectureDiagramLayout, id: &str) -> (f64, f64) {
    let node = layout
        .nodes
        .iter()
        .find(|n| n.id == id)
        .unwrap_or_else(|| panic!("missing node {id}"));
    (node.x + node.width / 2.0, node.y + node.height / 2.0)
}

fn center_distance(layout: &ArchitectureDiagramLayout, a: &str, b: &str) -> f64 {
    let (ax, ay) = node_center(layout, a);
    let (bx, by) = node_center(layout, b);
    ((bx - ax).powi(2) + (by - ay).powi(2)).sqrt()
}

fn position_signature(layout: &ArchitectureDiagramLayout) -> Vec<(String, i64, i64)> {
    let mut sig: Vec<(String, i64, i64)> = layout
        .nodes
        .iter()
        .map(|n| {
            (
                n.id.clone(),
                (n.x * 1000.0).round() as i64,
                (n.y * 1000.0).round() as i64,
            )
        })
        .collect();
    sig.sort();
    sig
}

fn fcose_compound_size(layout: &ArchitectureDiagramLayout, id: &str) -> (f64, f64) {
    let bounds = layout
        .fcose_compound_bounds
        .iter()
        .find(|b| b.id == id)
        .unwrap_or_else(|| panic!("missing fcose compound bounds for {id}"));
    (
        bounds.bounds.max_x - bounds.bounds.min_x,
        bounds.bounds.max_y - bounds.bounds.min_y,
    )
}

fn with_architecture_config(diagram: &str, config: &str) -> String {
    format!("%%{{init: {{\"architecture\": {config}}}}}%%\n{diagram}")
}

fn chain_diagram() -> &'static str {
    r#"architecture-beta
  group app(cloud)[App]
  service a(server)[A] in app
  service b(server)[B] in app
  service c(server)[C] in app
  service d(server)[D] in app
  a:R -- L:b
  b:R -- L:c
  c:R -- L:d
"#
}

fn disconnected_diagram() -> &'static str {
    r#"architecture-beta
  service a(server)[A]
  service b(server)[B]
  service c(server)[C]
  service d(server)[D]
"#
}

#[test]
fn architecture_default_fcose_layout_is_deterministic() {
    let first = layout_architecture(chain_diagram());
    let second = layout_architecture(chain_diagram());

    assert_eq!(position_signature(&first), position_signature(&second));
}

#[test]
fn architecture_layout_exposes_fcose_compound_bounds_by_group_id() {
    let layout = layout_architecture(chain_diagram());

    assert_eq!(layout.fcose_compound_bounds.len(), 1);
    let (width, height) = fcose_compound_size(&layout, "app");
    assert!(
        width > 80.0 && height > 80.0,
        "expected FCoSE compound bounds to include child graph padding, got {width:.3}x{height:.3}"
    );
}

#[test]
fn architecture_ideal_edge_length_multiplier_changes_same_group_spacing() {
    let diagram = r#"architecture-beta
  group app(cloud)[App]
  service a(server)[A] in app
  service b(server)[B] in app
  a:R -- L:b
"#;

    let default_layout = layout_architecture(diagram);
    let roomy_layout = layout_architecture(&with_architecture_config(
        diagram,
        r#"{"idealEdgeLengthMultiplier": 3}"#,
    ));

    let default_distance = center_distance(&default_layout, "a", "b");
    let roomy_distance = center_distance(&roomy_layout, "a", "b");
    assert!(
        roomy_distance > default_distance * 1.25,
        "expected higher idealEdgeLengthMultiplier to spread same-group nodes: default={default_distance:.3}, roomy={roomy_distance:.3}"
    );
}

#[test]
fn architecture_randomize_and_node_separation_change_layout() {
    let compact = layout_architecture(&with_architecture_config(
        disconnected_diagram(),
        r#"{"randomize": true, "nodeSeparation": 75}"#,
    ));
    let spread = layout_architecture(&with_architecture_config(
        disconnected_diagram(),
        r#"{"randomize": true, "nodeSeparation": 180}"#,
    ));

    assert_ne!(
        position_signature(&compact),
        position_signature(&spread),
        "expected nodeSeparation to affect randomized Architecture FCoSE layout"
    );
}

#[test]
fn architecture_num_iter_changes_layout_budget() {
    let early_stop = layout_architecture(&with_architecture_config(
        chain_diagram(),
        r#"{"numIter": 25}"#,
    ));
    let default_budget = layout_architecture(chain_diagram());

    assert_ne!(
        position_signature(&early_stop),
        position_signature(&default_budget),
        "expected numIter to affect Architecture FCoSE convergence budget"
    );
}

#[test]
fn architecture_edge_elasticity_changes_same_group_layout() {
    let loose = layout_architecture(&with_architecture_config(
        chain_diagram(),
        r#"{"edgeElasticity": 0.05, "numIter": 80}"#,
    ));
    let stiff = layout_architecture(&with_architecture_config(
        chain_diagram(),
        r#"{"edgeElasticity": 0.9, "numIter": 80}"#,
    ));

    assert_ne!(
        position_signature(&loose),
        position_signature(&stiff),
        "expected edgeElasticity to affect same-group Architecture FCoSE layout"
    );
}
