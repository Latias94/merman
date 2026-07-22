use merman_core::{Engine, ParseOptions, ParsedDiagramRender, RenderSemanticModel};
use merman_render::class::layout_class_diagram_typed_with_config;
use merman_render::environment::{RenderEnvironment, RenderSession, TextMeasurementPhase};
use merman_render::model::ClassDiagramLayout;
use std::path::PathBuf;

fn workspace_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("..")
}

fn parse_class(text: &str) -> ParsedDiagramRender {
    Engine::new()
        .parse_diagram_for_render_model_sync(text, ParseOptions::default())
        .expect("parse ok")
        .expect("diagram detected")
}

fn class_model(parsed: &ParsedDiagramRender) -> &merman_core::models::class_diagram::ClassDiagram {
    let RenderSemanticModel::Class(model) = parsed.model() else {
        panic!("expected Class render model");
    };
    model
}

fn layout_class_with_dagre(
    parsed: &ParsedDiagramRender,
    session: &RenderSession,
) -> ClassDiagramLayout {
    let model = class_model(parsed);
    session
        .resource_policy()
        .check_class_complexity(model)
        .expect("class complexity within test limits");
    let measurer = session.text_measurer(TextMeasurementPhase::Layout);
    layout_class_diagram_typed_with_config(model, &parsed.metadata().effective_config, &measurer)
        .expect("Dagre class layout")
}

fn load_class_layout_fixture(name: &str) -> ClassDiagramLayout {
    let session = RenderEnvironment::parity().begin_session().unwrap();
    let path = workspace_root()
        .join("fixtures")
        .join("class")
        .join(format!("{name}.mmd"));
    let text = std::fs::read_to_string(&path).expect("fixture");

    let parsed = parse_class(&text);
    layout_class_with_dagre(&parsed, &session)
}

fn layout_class_text(text: &str) -> (ClassDiagramLayout, ParsedDiagramRender) {
    let session = RenderEnvironment::parity().begin_session().unwrap();
    let parsed = parse_class(text);
    let layout = layout_class_with_dagre(&parsed, &session);
    (layout, parsed)
}

fn nested_class_namespace_text(depth: usize) -> String {
    let mut lines = vec!["classDiagram".to_string()];
    for i in 0..depth {
        lines.push(format!("{}namespace N{i} {{", "  ".repeat(i)));
    }
    lines.push(format!("{}class Leaf", "  ".repeat(depth)));
    for i in (0..depth).rev() {
        lines.push(format!("{}}}", "  ".repeat(i)));
    }
    lines.join("\n")
}

fn rect_from_node(n: &merman_render::model::LayoutNode) -> (f64, f64, f64, f64) {
    let hw = n.width / 2.0;
    let hh = n.height / 2.0;
    (n.x - hw, n.y - hh, n.x + hw, n.y + hh)
}

fn rect_from_cluster(c: &merman_render::model::LayoutCluster) -> (f64, f64, f64, f64) {
    let hw = c.width / 2.0;
    let hh = c.height / 2.0;
    (c.x - hw, c.y - hh, c.x + hw, c.y + hh)
}

fn rect_contains(outer: (f64, f64, f64, f64), inner: (f64, f64, f64, f64), eps: f64) -> bool {
    let (omin_x, omin_y, omax_x, omax_y) = outer;
    let (imin_x, imin_y, imax_x, imax_y) = inner;
    imin_x + eps >= omin_x
        && imax_x <= omax_x + eps
        && imin_y + eps >= omin_y
        && imax_y <= omax_y + eps
}

#[test]
fn class_layout_produces_positions_and_routes() {
    let session = RenderEnvironment::parity().begin_session().unwrap();
    let path = workspace_root()
        .join("fixtures")
        .join("class")
        .join("basic.mmd");
    let text = std::fs::read_to_string(&path).expect("fixture");

    let parsed = parse_class(&text);
    let layout = layout_class_with_dagre(&parsed, &session);

    assert!(layout.nodes.len() >= 2);
    assert!(!layout.edges.is_empty());

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
fn class_namespaces_contain_member_classes() {
    let session = RenderEnvironment::parity().begin_session().unwrap();
    let path = workspace_root()
        .join("fixtures")
        .join("class")
        .join("upstream_namespaces_and_generics.mmd");
    let text = std::fs::read_to_string(&path).expect("fixture");

    let parsed = parse_class(&text);
    let layout = layout_class_with_dagre(&parsed, &session);

    let mut node_by_id = std::collections::HashMap::new();
    for n in &layout.nodes {
        if !n.is_cluster {
            node_by_id.insert(n.id.as_str(), n);
        }
    }
    let mut cluster_by_id = std::collections::HashMap::new();
    for c in &layout.clusters {
        cluster_by_id.insert(c.id.as_str(), c);
    }

    for (id, class) in &class_model(&parsed).classes {
        let parent = class.parent.as_deref().unwrap_or("");
        if parent.is_empty() {
            continue;
        }
        let Some(node) = node_by_id.get(id.as_str()) else {
            continue;
        };
        let Some(cluster) = cluster_by_id.get(parent) else {
            panic!("missing cluster {parent}");
        };
        assert!(
            rect_contains(rect_from_cluster(cluster), rect_from_node(node), 0.01),
            "cluster {parent} should contain {id}"
        );
    }
}

#[test]
fn class_layout_dense_namespaces_follow_declaration_order() {
    let layout = load_class_layout_fixture("stress_class_dense_namespaces_generics_001");

    let cluster_ids = layout
        .clusters
        .iter()
        .map(|cluster| cluster.id.as_str())
        .collect::<Vec<_>>();
    assert_eq!(cluster_ids, vec!["Core", "API"]);
}

#[test]
fn class_layout_dotted_namespace_builds_hierarchical_clusters() {
    let (layout, _parsed) = layout_class_text(
        r#"classDiagram
namespace Company.Project.Module {
  class User
}
"#,
    );

    let cluster_ids = layout
        .clusters
        .iter()
        .map(|cluster| cluster.id.as_str())
        .collect::<Vec<_>>();
    assert_eq!(
        cluster_ids,
        vec!["Company", "Company.Project", "Company.Project.Module"]
    );

    let mut cluster_by_id = std::collections::HashMap::new();
    for c in &layout.clusters {
        cluster_by_id.insert(c.id.as_str(), c);
    }
    let user = layout
        .nodes
        .iter()
        .find(|node| node.id == "User")
        .expect("User node");
    let module = cluster_by_id
        .get("Company.Project.Module")
        .expect("module cluster");
    let project = cluster_by_id
        .get("Company.Project")
        .expect("project cluster");
    let company = cluster_by_id.get("Company").expect("company cluster");

    assert!(
        rect_contains(rect_from_cluster(module), rect_from_node(user), 0.01),
        "module cluster should contain User"
    );
    assert!(
        rect_contains(rect_from_cluster(project), rect_from_cluster(module), 0.01),
        "project cluster should contain module"
    );
    assert!(
        rect_contains(rect_from_cluster(company), rect_from_cluster(project), 0.01),
        "company cluster should contain project"
    );
}

#[test]
fn class_layout_nested_namespace_cross_edge_stays_in_parent_compound() {
    let layout = load_class_layout_fixture("upstream_pkgtests_classdiagram_spec_003");

    let admin = layout
        .nodes
        .iter()
        .find(|node| node.id == "Admin")
        .expect("Admin node");
    let report = layout
        .nodes
        .iter()
        .find(|node| node.id == "Report")
        .expect("Report node");
    let module = layout
        .clusters
        .iter()
        .find(|cluster| cluster.id == "Company.Project.Module")
        .expect("module cluster");

    assert!(
        report.y > admin.y + 100.0,
        "nested namespace cross-edge should stack Report below Admin"
    );
    assert!(
        (report.x - admin.x).abs() < 25.0,
        "nested namespace cross-edge should stay vertically aligned"
    );
    assert!(
        module.height > module.width,
        "module cluster should stay in the surrounding TB compound layout"
    );
}

#[test]
fn class_layout_lr_namespace_cross_edge_extracts_parent_compound() {
    let layout = load_class_layout_fixture("upstream_namespaces_and_generics");

    let generic = layout
        .nodes
        .iter()
        .find(|node| node.id == "GenericClass")
        .expect("GenericClass node");
    let admin = layout
        .nodes
        .iter()
        .find(|node| node.id == "Admin")
        .expect("Admin node");
    let user = layout
        .nodes
        .iter()
        .find(|node| node.id == "User")
        .expect("User node");
    let module = layout
        .clusters
        .iter()
        .find(|cluster| cluster.id == "Company.Project.Module")
        .expect("module cluster");

    assert!(
        user.x > admin.x + 350.0,
        "LR namespace cross-edge should place User to the right of Admin"
    );
    assert!(
        (user.y - admin.y).abs() < 25.0,
        "LR namespace cross-edge should keep User aligned with Admin"
    );
    assert!(
        generic.y + 100.0 < admin.y,
        "module's unrelated GenericClass should stay above Admin"
    );
    assert!(
        module.height > module.width,
        "module cluster should stay as a vertical stack inside the extracted parent"
    );
}

#[test]
fn class_layout_nested_namespace_copy_order_keeps_leaf_cluster_vertical() {
    let layout = load_class_layout_fixture("upstream_pkgtests_classdiagram_spec_006");

    let admin = layout
        .nodes
        .iter()
        .find(|node| node.id == "Admin")
        .expect("Admin node");
    let user = layout
        .nodes
        .iter()
        .find(|node| node.id == "User")
        .expect("User node");
    let module = layout
        .clusters
        .iter()
        .find(|cluster| cluster.id == "Company.Project.Module")
        .expect("module cluster");

    assert!(
        user.y > admin.y + 150.0,
        "child-before-parent extraction should keep User below Admin"
    );
    assert!(
        (user.x - admin.x).abs() < 25.0,
        "nested leaf namespace should not be laid out horizontally"
    );
    assert!(
        module.height > module.width * 2.0,
        "leaf namespace should be tall and narrow after the moved child extraction"
    );
}

#[test]
fn class_layout_v3_namespace_node_order_matches_mermaid_copy_order() {
    let layout = load_class_layout_fixture("stress_class_nested_namespaces_cross_edges_008");

    let one_a = layout
        .nodes
        .iter()
        .find(|node| node.id == "OneA")
        .expect("OneA node");
    let two_b = layout
        .nodes
        .iter()
        .find(|node| node.id == "TwoB")
        .expect("TwoB node");
    let two_c = layout
        .nodes
        .iter()
        .find(|node| node.id == "TwoC")
        .expect("TwoC node");
    let one_two = layout
        .clusters
        .iter()
        .find(|cluster| cluster.id == "One.Two")
        .expect("One.Two cluster");

    assert!(
        two_b.y + 100.0 < two_c.y && two_c.y + 100.0 < one_a.y,
        "Mermaid v3 copy order should stack TwoB, TwoC, then OneA; got TwoB={}, TwoC={}, OneA={}",
        two_b.y,
        two_c.y,
        one_a.y
    );
    assert!(
        one_two.y + one_two.height / 2.0 < one_a.y,
        "nested namespace cluster should stay above OneA after recursive extraction"
    );
}

#[test]
fn class_layout_namespace_note_stays_inside_namespace_cluster() {
    let (layout, parsed) = layout_class_text(
        r#"classDiagram
namespace Company.Project {
  class User
  note "Module scoped note"
}
"#,
    );

    assert_eq!(
        class_model(&parsed).notes[0].parent.as_deref(),
        Some("Company.Project")
    );

    let note = layout
        .nodes
        .iter()
        .find(|node| node.id == "note0")
        .expect("note node");
    let cluster = layout
        .clusters
        .iter()
        .find(|cluster| cluster.id == "Company.Project")
        .expect("namespace cluster");
    assert!(
        rect_contains(rect_from_cluster(cluster), rect_from_node(note), 0.01),
        "namespace cluster should contain its note"
    );
}

#[test]
fn class_layout_deep_namespaces_fall_back_without_stack_growth() {
    let depth = 24;
    let (layout, _parsed) = layout_class_text(&nested_class_namespace_text(depth));

    assert_eq!(layout.clusters.len(), depth);
    assert!(
        layout.nodes.iter().any(|node| node.id == "Leaf"),
        "expected deeply nested class member to remain in the layout"
    );

    for cluster in &layout.clusters {
        assert!(cluster.width.is_finite() && cluster.width > 0.0);
        assert!(cluster.height.is_finite() && cluster.height > 0.0);
        assert!(cluster.x.is_finite() && cluster.y.is_finite());
    }
}

#[test]
fn class_layout_hierarchical_namespaces_false_keeps_flat_dotted_cluster() {
    let (layout, parsed) = layout_class_text(
        r#"---
config:
  class:
    hierarchicalNamespaces: false
---
classDiagram
namespace Company.Project.Module {
  class User
}
"#,
    );

    assert_eq!(
        layout
            .clusters
            .iter()
            .map(|cluster| cluster.id.as_str())
            .collect::<Vec<_>>(),
        vec!["Company.Project.Module"]
    );
    assert_eq!(
        layout.clusters[0].title, "Company.Project.Module",
        "compact mode should use the full namespace id as the label"
    );
    assert_eq!(
        class_model(&parsed).classes["User"].parent.as_deref(),
        Some("Company.Project.Module")
    );
}

#[test]
fn class_terminal_labels_exist_for_cardinalities_fixture() {
    let session = RenderEnvironment::parity().begin_session().unwrap();
    let path = workspace_root()
        .join("fixtures")
        .join("class")
        .join("upstream_relation_types_and_cardinalities_spec.mmd");
    let text = std::fs::read_to_string(&path).expect("fixture");

    let parsed = parse_class(&text);
    let layout = layout_class_with_dagre(&parsed, &session);

    let has_terminal = layout.edges.iter().any(|e| {
        e.start_label_left.is_some()
            || e.start_label_right.is_some()
            || e.end_label_left.is_some()
            || e.end_label_right.is_some()
    });
    assert!(has_terminal, "expected at least one terminal label");
}

fn point_inside(rect: (f64, f64, f64, f64), x: f64, y: f64, eps: f64) -> bool {
    let (min_x, min_y, max_x, max_y) = rect;
    x >= min_x - eps && x <= max_x + eps && y >= min_y - eps && y <= max_y + eps
}

#[test]
fn class_terminal_labels_are_outside_endpoint_nodes_for_cardinalities_fixture() {
    let session = RenderEnvironment::parity().begin_session().unwrap();
    let path = workspace_root()
        .join("fixtures")
        .join("class")
        .join("upstream_relation_types_and_cardinalities_spec.mmd");
    let text = std::fs::read_to_string(&path).expect("fixture");

    let parsed = parse_class(&text);
    let layout = layout_class_with_dagre(&parsed, &session);

    let mut node_rect_by_id = std::collections::HashMap::new();
    for n in &layout.nodes {
        if n.is_cluster {
            continue;
        }
        node_rect_by_id.insert(n.id.as_str(), rect_from_node(n));
    }

    let eps = 0.01;
    let mut checked = 0usize;
    for e in &layout.edges {
        let Some(from_rect) = node_rect_by_id.get(e.from.as_str()) else {
            continue;
        };
        let Some(to_rect) = node_rect_by_id.get(e.to.as_str()) else {
            continue;
        };

        for lbl in [
            e.start_label_left.as_ref(),
            e.start_label_right.as_ref(),
            e.end_label_left.as_ref(),
            e.end_label_right.as_ref(),
        ] {
            let Some(lbl) = lbl else {
                continue;
            };
            checked += 1;
            assert!(
                !point_inside(*from_rect, lbl.x, lbl.y, eps),
                "terminal label center should not be inside start node for edge {}",
                e.id
            );
            assert!(
                !point_inside(*to_rect, lbl.x, lbl.y, eps),
                "terminal label center should not be inside end node for edge {}",
                e.id
            );
        }
    }
    assert!(checked > 0, "expected to check at least one terminal label");
}

#[test]
fn class_svg_title_widths_scale_with_measured_content() {
    let session = merman_render::environment::RenderEnvironment::parity()
        .begin_session()
        .unwrap();
    let text = r#"---
config:
  htmlLabels: false
---
classDiagram
A <|-- B
LongClassName <|-- B
"#;

    let parsed = parse_class(text);
    let layout = layout_class_with_dagre(&parsed, &session);

    let node_a = layout
        .nodes
        .iter()
        .find(|n| n.id == "A")
        .expect("class A node");
    let node_b = layout
        .nodes
        .iter()
        .find(|n| n.id == "B")
        .expect("class B node");
    let long = layout
        .nodes
        .iter()
        .find(|n| n.id == "LongClassName")
        .expect("long-title class node");

    assert!(node_a.width.is_finite() && node_a.width > 0.0);
    assert!(node_b.width.is_finite() && node_b.width > 0.0);
    assert!(long.width > node_a.width && long.width > node_b.width);
}
