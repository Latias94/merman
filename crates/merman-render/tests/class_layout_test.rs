use merman_core::{Engine, ParseOptions, ParsedDiagramRender, RenderSemanticModel};
use merman_render::LayoutOptions;
use merman_render::environment::RenderEnvironment;
use merman_render::family;
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

fn layout_class_with_dagre(text: &str, environment: &RenderEnvironment) -> ClassDiagramLayout {
    let parsed = parse_class(text);
    let session = environment.begin_session().unwrap();
    let artifact =
        family::prepare(parsed, &LayoutOptions::default(), session).expect("Dagre class layout");
    let projection = artifact.layout_json().expect("Class layout projection");
    serde_json::from_value(projection["layout"]["ClassDiagramV2"].clone()).expect("Class layout")
}

fn load_class_layout_fixture(name: &str) -> ClassDiagramLayout {
    let environment = RenderEnvironment::deterministic();
    let path = workspace_root()
        .join("fixtures")
        .join("class")
        .join(format!("{name}.mmd"));
    let text = std::fs::read_to_string(&path).expect("fixture");
    let text = format!("---\nconfig:\n  layout: dagre\n---\n{text}");

    layout_class_with_dagre(&text, &environment)
}

fn layout_class_text(text: &str) -> (ClassDiagramLayout, ParsedDiagramRender) {
    let environment = RenderEnvironment::deterministic();
    let parsed = parse_class(text);
    let layout = layout_class_with_dagre(text, &environment);
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
    let environment = RenderEnvironment::deterministic();
    let path = workspace_root()
        .join("fixtures")
        .join("class")
        .join("basic.mmd");
    let text = std::fs::read_to_string(&path).expect("fixture");

    let layout = layout_class_with_dagre(&text, &environment);

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
    let environment = RenderEnvironment::deterministic();
    let path = workspace_root()
        .join("fixtures")
        .join("class")
        .join("upstream_namespaces_and_generics.mmd");
    let text = std::fs::read_to_string(&path).expect("fixture");

    let parsed = parse_class(&text);
    let layout = layout_class_with_dagre(&text, &environment);

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
    let text = std::fs::read_to_string(
        workspace_root().join("fixtures/class/stress_class_nested_namespaces_cross_edges_008.mmd"),
    )
    .expect("fixture");
    let layout = layout_class_with_dagre(
        &format!("%%{{init: {{\"layout\": \"dagre\"}}}}%%\n{text}"),
        &RenderEnvironment::deterministic(),
    );

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
fn class_layout_notes_precede_lollipop_interfaces_in_pinned_source_order() {
    let text = std::fs::read_to_string(
        workspace_root()
            .join("fixtures/class/upstream_html_demos_classchart_class_diagram_demos_006.mmd"),
    )
    .expect("fixture");
    let layout = layout_class_with_dagre(
        &format!("%%{{init: {{\"layout\": \"dagre\"}}}}%%\n{text}"),
        &RenderEnvironment::deterministic(),
    );

    assert_eq!(
        layout
            .nodes
            .iter()
            .map(|node| node.id.as_str())
            .collect::<Vec<_>>(),
        vec!["Dog", "Cat", "note0", "interface0", "interface1"]
    );
    assert_eq!(
        layout
            .edges
            .iter()
            .map(|edge| edge.id.as_str())
            .collect::<Vec<_>>(),
        vec!["edgeNote0", "0", "1"]
    );

    let node = |id: &str| {
        layout
            .nodes
            .iter()
            .find(|node| node.id == id)
            .unwrap_or_else(|| panic!("node {id}"))
    };
    let note = node("note0");
    let first_interface = node("interface0");
    let second_interface = node("interface1");
    let dog = node("Dog");

    assert!((note.x - first_interface.x).abs() < 1e-6);
    assert!((note.x - second_interface.x).abs() < 1e-6);
    assert!(
        note.y < second_interface.y && second_interface.y < first_interface.y,
        "pinned note/interface order should be note0, interface1, interface0; note={}, interface1={}, interface0={}",
        note.y,
        second_interface.y,
        first_interface.y
    );
    assert!(
        (first_interface.y - dog.y).abs() < 1e-6,
        "interface0 and Dog should remain on the same horizontal route"
    );
}

#[test]
fn class_layout_note_edge_ids_use_declaration_index_and_precede_relations() {
    let (layout, parsed) = layout_class_text(
        r#"classDiagram
class A
note "detached"
A --> B
note for B "attached"
"#,
    );

    assert_eq!(
        class_model(&parsed)
            .notes
            .iter()
            .map(|note| note.id.as_str())
            .collect::<Vec<_>>(),
        vec!["note0", "note1"]
    );
    assert_eq!(
        layout
            .edges
            .iter()
            .map(|edge| edge.id.as_str())
            .collect::<Vec<_>>(),
        vec!["edgeNote1", "0"]
    );

    let note_edge = &layout.edges[0];
    assert_eq!(note_edge.from, "note1");
    assert_eq!(note_edge.to, "B");
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
    let environment = RenderEnvironment::deterministic();
    let path = workspace_root()
        .join("fixtures")
        .join("class")
        .join("upstream_relation_types_and_cardinalities_spec.mmd");
    let text = std::fs::read_to_string(&path).expect("fixture");

    let layout = layout_class_with_dagre(&text, &environment);

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
    let environment = RenderEnvironment::deterministic();
    let path = workspace_root()
        .join("fixtures")
        .join("class")
        .join("upstream_relation_types_and_cardinalities_spec.mmd");
    let text = std::fs::read_to_string(&path).expect("fixture");

    let layout = layout_class_with_dagre(&text, &environment);

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
    let environment = RenderEnvironment::deterministic();
    let text = r#"---
config:
  htmlLabels: false
---
classDiagram
A <|-- B
LongClassName <|-- B
"#;

    let layout = layout_class_with_dagre(text, &environment);

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

#[cfg(feature = "layout-elk")]
#[test]
fn class_elk_lollipop_interfaces_remain_outside_nested_namespace_frames() {
    for relation in ["A --() ProvidedInterface", "ProvidedInterface ()-- A"] {
        let source = format!(
            "---\nconfig:\n  layout: elk\n---\nclassDiagram\nnamespace Outer {{\nnamespace Inner {{\nclass A\n}}\n}}\n{relation}\n"
        );
        let (layout, _) = layout_class_text(&source);
        let class = layout.nodes.iter().find(|node| node.id == "A").unwrap();
        let interface = layout
            .nodes
            .iter()
            .find(|node| node.id == "interface0")
            .unwrap();
        for cluster in &layout.clusters {
            let frame = rect_from_cluster(cluster);
            assert!(point_inside(frame, class.x, class.y, 0.01));
            assert!(!point_inside(frame, interface.x, interface.y, 0.01));
        }
        assert_eq!(layout.edges.len(), 1);
        assert!(layout.edges[0].points.len() >= 2);
    }
}

#[test]
fn class_cardinality_metrics_are_independent_of_class_member_font_size() {
    for layout in ["dagre", "elk"] {
        if layout == "elk" && !cfg!(feature = "layout-elk") {
            continue;
        }
        for html in [false, true] {
            let measured = [14, 30].map(|font_size| {
                let source = format!("---\nconfig:\n  layout: {layout}\n  htmlLabels: {html}\n  themeVariables:\n    fontSize: {font_size}px\n---\nclassDiagram\nA \"one\" --> \"manyLongCardinalityWords\" B\n");
                let (layout, _) = layout_class_text(&source);
                let edge = &layout.edges[0];
                let labels = [edge.start_label_right.as_ref().unwrap(), edge.end_label_left.as_ref().unwrap()];
                for label in labels {
                    let bounds = layout.bounds.as_ref().unwrap();
                    assert!(label.x - label.width / 2.0 >= bounds.min_x - 1e-6);
                    assert!(label.x + label.width / 2.0 <= bounds.max_x + 1e-6);
                    assert!(label.y - label.height / 2.0 >= bounds.min_y - 1e-6);
                    assert!(label.y + label.height / 2.0 <= bounds.max_y + 1e-6);
                }
                labels.map(|label| (label.width, label.height))
            });
            assert_eq!(
                measured[0], measured[1],
                "terminal CSS is fixed at 11px: {layout}/{html}"
            );
            assert!(measured[0][1].0 > measured[0][0].0);
        }
    }
}

#[cfg(feature = "layout-elk")]
#[test]
fn class_elk_cardinalities_clear_namespace_frames_and_endpoint_boxes() {
    // The crossNamespacesDiagram terminalLabels.spec.ts case, also nested one level deeper.
    for nested in [false, true] {
        let groups = "namespace Shop {\nclass Order\n}\nnamespace Catalog {\nclass Product\n}\n";
        let body = if nested {
            format!("namespace Root {{\n{groups}}}\n")
        } else {
            groups.into()
        };
        let source = format!(
            "---\nconfig:\n  layout: elk\n  htmlLabels: true\n---\nclassDiagram\ndirection LR\n{body}Order \"*\" --> \"1..*\" Product\n"
        );
        let (layout, _) = layout_class_text(&source);
        let edge = &layout.edges[0];
        for label in [
            edge.start_label_right.as_ref().unwrap(),
            edge.end_label_left.as_ref().unwrap(),
        ] {
            let label_box = (
                label.x - label.width / 2.0,
                label.y - label.height / 2.0,
                label.x + label.width / 2.0,
                label.y + label.height / 2.0,
            );
            let padded = (
                label_box.0 - 2.0,
                label_box.1 - 2.0,
                label_box.2 + 2.0,
                label_box.3 + 2.0,
            );
            for node in &layout.nodes {
                let rect = rect_from_node(node);
                let overlaps = |b: (f64, f64, f64, f64)| {
                    b.0 < rect.2 && rect.0 < b.2 && b.1 < rect.3 && rect.1 < b.3
                };
                if node.is_cluster {
                    assert!(
                        rect_contains(rect, padded, 1e-6) || !overlaps(padded),
                        "{nested}: terminal {label:?} crosses frame {} {rect:?}",
                        node.id
                    );
                } else {
                    assert!(!overlaps(label_box), "terminal overlaps {}", node.id);
                }
            }
        }
        for (label, start) in [
            (edge.start_label_right.as_ref().unwrap(), true),
            (edge.end_label_left.as_ref().unwrap(), false),
        ] {
            let points = if start {
                &edge.points[..2]
            } else {
                &edge.points[edge.points.len() - 2..]
            };
            let side = (points[1].x - points[0].x) * (label.y - points[0].y)
                - (points[1].y - points[0].y) * (label.x - points[0].x);
            assert_eq!(side > 0.0, start, "terminal uses its source-defined side");
        }
    }
}

#[test]
fn class_dagre_terminal_fallback_uses_centered_end_coordinates() {
    let source =
        "---\nconfig:\n  layout: dagre\n---\nclassDiagram\ndirection LR\nA \"1\" --> \"many\" B\n";
    let (layout, _) = layout_class_text(source);
    let edge = &layout.edges[0];
    let first = edge.points.first().unwrap();
    let last = edge.points.last().unwrap();
    assert!(
        edge.points
            .iter()
            .all(|point| (point.y - first.y).abs() < 1e-6)
    );
    let start = edge.start_label_right.as_ref().unwrap();
    let end = edge.end_label_left.as_ref().unwrap();
    // utils.calcTerminalLabelPosition samples 25 + 10px along the route, takes its
    // midpoint with the endpoint, and offsets 10 + 10/2px normal to the route.
    assert!((start.x - first.x - 17.5).abs() < 1e-6);
    assert!((start.y - first.y - 15.0).abs() < 1e-6);
    assert!(
        (end.x - last.x + 17.5).abs() < 1e-6,
        "end no longer has the legacy -5px shift"
    );
    assert!(
        (end.y - last.y + 15.0).abs() < 1e-6,
        "end no longer has the legacy -5px shift"
    );
}

#[cfg(feature = "layout-elk")]
#[test]
fn class_non_layered_short_terminal_fallback_reports_the_source_error() {
    let source = "---\nconfig:\n  layout: elk.mrtree\n---\nclassDiagram\ndirection LR\nA \"1\" --> \"many\" B\n";
    let parsed = parse_class(source);
    let environment = RenderEnvironment::deterministic();
    let session = environment.begin_session().unwrap();
    let error = family::prepare(parsed, &LayoutOptions::default(), session)
        .err()
        .expect("Mermaid rejects terminal samples longer than the non-layered route");
    assert!(
        error
            .to_string()
            .contains("Could not find a suitable point for the given distance")
    );
}
