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

fn cytoscape_service_union_size(layout: &ArchitectureDiagramLayout, id: &str) -> (f64, f64) {
    let bounds = layout
        .cytoscape_service_bounds
        .iter()
        .find(|b| b.id == id)
        .unwrap_or_else(|| panic!("missing cytoscape service bounds for {id}"));
    assert!(
        bounds.label_bounds.is_some(),
        "expected {id} to preserve a label contribution phase"
    );
    (
        bounds.union_bounds.max_x - bounds.union_bounds.min_x,
        bounds.union_bounds.max_y - bounds.union_bounds.min_y,
    )
}

fn with_architecture_config(diagram: &str, config: &str) -> String {
    format!("%%{{init: {{\"architecture\": {config}}}}}%%\n{diagram}")
}

fn deep_group_chain_diagram(depth: usize) -> String {
    let mut lines = vec![
        r#"%%{init: {"architecture": {"numIter": 1, "randomize": false}}}%%"#.to_string(),
        "architecture-beta".to_string(),
    ];
    for i in 0..depth {
        let parent = (i > 0)
            .then(|| format!(" in g{}", i - 1))
            .unwrap_or_default();
        lines.push(format!("  group g{i}(cloud)[G{i}]{parent}"));
    }
    lines.push(format!("  service leaf(server)[Leaf] in g{}", depth - 1));
    lines.join("\n")
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
fn architecture_parse_for_render_model_handles_deep_group_chain() {
    const DEPTH: usize = 64;
    let source = deep_group_chain_diagram(DEPTH);
    let handle = std::thread::Builder::new()
        .name("architecture-deep-group-parse".to_string())
        .stack_size(128 * 1024)
        .spawn(move || {
            let engine = Engine::new();
            engine
                .parse_diagram_for_render_model_sync(&source, ParseOptions::strict())
                .expect("parse ok")
                .expect("diagram detected");
        })
        .expect("spawn architecture deep group parse test");
    handle
        .join()
        .expect("architecture deep group parse should finish without stack overflow");
}

#[test]
fn architecture_layout_handles_deep_group_chain() {
    const DEPTH: usize = 64;
    let source = deep_group_chain_diagram(DEPTH);
    let handle = std::thread::Builder::new()
        .name("architecture-deep-group-layout".to_string())
        .stack_size(128 * 1024)
        .spawn(move || layout_architecture(&source))
        .expect("spawn architecture deep group layout test");
    let layout = handle
        .join()
        .expect("architecture deep group layout should finish without stack overflow");

    assert!(
        layout.nodes.iter().any(|node| node.id == "leaf"),
        "expected deepest service to remain in Architecture layout"
    );
    assert!(
        layout
            .fcose_compound_bounds
            .iter()
            .any(|bounds| bounds.id == format!("g{}", DEPTH - 1)),
        "expected deepest group to preserve FCoSE compound bounds"
    );
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
fn architecture_layout_exposes_cytoscape_service_child_bounds_by_service_id() {
    let layout = layout_architecture(
        r#"architecture-beta
  group app(cloud)[App]
  service gateway(server)[A very long gateway label for group sizing] in app
"#,
    );

    let service = layout
        .cytoscape_service_bounds
        .iter()
        .find(|b| b.id == "gateway")
        .expect("gateway service bounds");
    assert_eq!(service.in_group.as_deref(), Some("app"));
    let metrics = service
        .label_metrics
        .as_ref()
        .expect("gateway service label metrics");
    assert!(
        metrics.text_width > 80.0 && metrics.half_width > 40.0,
        "expected raw label metrics to be exposed for service contribution audit, got text_width={:.3} half_width={:.3}",
        metrics.text_width,
        metrics.half_width
    );
    assert!(
        metrics.applied_scale >= 1.0,
        "expected service label metric scale to be recorded, got {:.3}",
        metrics.applied_scale
    );
    let (width, height) = cytoscape_service_union_size(&layout, "gateway");
    assert!(
        width > 80.0 && height > 80.0,
        "expected label contribution to expand service child union, got {width:.3}x{height:.3}"
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
