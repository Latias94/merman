use merman_core::{Engine, ParseOptions};
use merman_render::LayoutOptions;
use merman_render::environment::RenderEnvironment;
use merman_render::family;
use merman_render::model::{SwimlaneDirection, SwimlaneLayout};
use merman_render::svg::{SvgDebugOptions, SvgRenderOptions};

const NESTED_MIXED: &str =
    include_str!("../../../fixtures/swimlane/local_nested_mixed_lane_security.mmd");
const NESTED_MIXED_UPSTREAM_SVG: &str =
    include_str!("../../../fixtures/upstream-svgs/swimlane/local_nested_mixed_lane_security.svg");

fn render_swimlane_with_layout(source: &str, diagram_id: &str) -> (String, SwimlaneLayout) {
    let parsed = Engine::new()
        .parse_diagram_for_render_model_sync(source, ParseOptions::strict())
        .expect("parse swimlane")
        .expect("detect swimlane");
    let session = RenderEnvironment::deterministic().begin_session().unwrap();
    let artifact = family::prepare(&parsed, &LayoutOptions::default(), session).unwrap();
    let layout = serde_json::from_value(
        artifact.layout_json().unwrap()["layout"]["SwimlaneDiagram"].clone(),
    )
    .unwrap();
    let svg = artifact
        .render_svg(
            &SvgRenderOptions {
                diagram_id: Some(diagram_id.to_string()),
                ..Default::default()
            },
            &SvgDebugOptions::default(),
        )
        .unwrap()
        .svg()
        .to_string();
    (svg, layout)
}

fn render_swimlane(source: &str, diagram_id: &str) -> String {
    render_swimlane_with_layout(source, diagram_id).0
}

#[test]
fn long_html_edge_labels_wrap_with_the_measured_content_box() {
    for width in [96, 144] {
        let source = format!(
            r#"---
config:
  htmlLabels: true
  themeVariables:
    fontSize: 22px
  flowchart:
    wrappingWidth: {width}
---
swimlane-beta LR
A -->|First <b>review</b> the complete request before sending the final reply| B
"#
        );
        let (svg, layout) = render_swimlane_with_layout(&source, "wrapped-label");
        let document = roxmltree::Document::parse(&svg).unwrap();
        let label = semantic_element(&document, "L_A_B_0", "edge-label");
        let foreign_object = label
            .descendants()
            .find(|node| node.has_tag_name("foreignObject"))
            .unwrap();
        let div = foreign_object
            .children()
            .find(|node| node.has_tag_name("div"))
            .unwrap();
        let style = div.attribute("style").unwrap();
        assert!(
            style.contains("white-space: break-spaces"),
            "long HTML labels must wrap at {width}px: {style}"
        );
        assert!(style.contains(&format!("max-width: {width}px;")));
        assert!(style.contains(&format!("width: {width}px;")));
        assert!(
            foreign_object
                .attribute("style")
                .is_some_and(|style| style.contains("overflow: visible"))
        );
        let measured = layout.nodes.iter().find(|node| node.is_edge_label).unwrap();
        assert_eq!(
            foreign_object
                .attribute("width")
                .unwrap()
                .parse::<f64>()
                .unwrap(),
            measured.label_width
        );
        assert_eq!(
            foreign_object
                .attribute("height")
                .unwrap()
                .parse::<f64>()
                .unwrap(),
            measured.label_height
        );
        assert!(
            measured.label_height > 33.0,
            "22px labels must span multiple lines"
        );
        assert_eq!(
            text_content(foreign_object),
            "First review the complete request before sending the final reply"
        );
    }
}

#[test]
fn html_edge_label_wrapping_uses_only_the_first_mermaid_label_style() {
    for (styles, font_size, needs_wrap) in [
        (
            "linkStyle default font-size:24px,font-weight:bold\nlinkStyle 0 font-size:12px",
            24,
            true,
        ),
        ("linkStyle 0 font-size:12px,font-weight:bold", 12, false),
        (
            "linkStyle default fill:blue,font-size:24px\nlinkStyle 0 font-size:24px",
            8,
            false,
        ),
    ] {
        let source = format!(
            r#"---
config:
  htmlLabels: true
  themeVariables:
    fontSize: 8px
  flowchart:
    wrappingWidth: 144
---
swimlane-beta LR
A styled@-->|Review request| B
{styles}
"#
        );
        let (svg, layout) = render_swimlane_with_layout(&source, "styled-label");
        let document = roxmltree::Document::parse(&svg).unwrap();
        let label = semantic_element(&document, "styled", "edge-label");
        let foreign_object = label
            .descendants()
            .find(|node| node.has_tag_name("foreignObject"))
            .unwrap();
        let div = foreign_object
            .children()
            .find(|node| node.has_tag_name("div"))
            .unwrap();
        let style = div.attribute("style").unwrap();
        assert_eq!(
            style.contains("white-space: break-spaces"),
            needs_wrap,
            "{styles}: {style}"
        );
        assert_eq!(style.contains("white-space: nowrap"), !needs_wrap);
        assert!(
            !style.contains("font-weight"),
            "only the first style is copied"
        );
        if font_size == 8 {
            assert!(style.contains("color:blue"), "{style}");
            assert!(!style.contains("font-size"));
        } else {
            assert!(style.contains(&format!("font-size:{font_size}px")));
        }
        let measured = layout.nodes.iter().find(|node| node.is_edge_label).unwrap();
        assert_eq!(
            measured.label_height > f64::from(font_size) * 1.5,
            needs_wrap,
            "{styles}: {measured:?}"
        );
    }
}

#[test]
fn explicit_swimlane_cluster_ids_are_scoped_to_each_diagram() {
    let source = "swimlane-beta LR\nsubgraph Lane\nA --> B\nend\n";
    let mut cluster_ids = std::collections::HashSet::new();
    for diagram_id in ["first", "second"] {
        let svg = render_swimlane(source, diagram_id);
        let document = roxmltree::Document::parse(&svg).expect("valid Swimlane SVG");
        let cluster = canonical_lane_cluster(&document, diagram_id, "Lane");
        assert!(cluster_ids.insert(cluster.attribute("id").unwrap().to_owned()));
    }
    assert_eq!(
        cluster_ids.len(),
        2,
        "diagram scopes must own distinct lane IDs"
    );
}

#[test]
fn synthetic_swimlane_cluster_keeps_semantic_id_in_a_canonical_scope() {
    let svg = render_swimlane("swimlane-beta LR\nA --> B\n", "synthetic");
    let document = roxmltree::Document::parse(&svg).expect("valid synthetic-lane SVG");
    let cluster = canonical_lane_cluster(&document, "synthetic", "__swimlane_default__");
    assert_ne!(cluster.attribute("id"), cluster.attribute("data-id"));
}

#[test]
fn explicitly_declared_default_swimlane_cluster_is_scoped() {
    let source = "swimlane-beta LR\nsubgraph __swimlane_default__[Named lane]\nA --> B\nend\n";
    let svg = render_swimlane(source, "declared");
    let document = roxmltree::Document::parse(&svg).expect("valid declared-lane SVG");
    let cluster = canonical_lane_cluster(&document, "declared", "__swimlane_default__");
    let title = cluster
        .children()
        .find(|node| has_class(*node, "cluster-label"))
        .unwrap();
    assert_eq!(text_content(title), "Named lane");
}

fn canonical_lane_cluster<'a>(
    document: &'a roxmltree::Document<'a>,
    diagram_id: &str,
    semantic_id: &str,
) -> roxmltree::Node<'a, 'a> {
    let cluster = canonical_cluster(document, diagram_id, semantic_id);
    assert!(has_class(cluster, "cluster") && has_class(cluster, "swimlane"));
    let ids = document
        .descendants()
        .filter_map(|node| node.attribute("id"))
        .collect::<Vec<_>>();
    assert_eq!(
        ids.len(),
        ids.iter()
            .copied()
            .collect::<std::collections::HashSet<_>>()
            .len(),
        "duplicate SVG DOM IDs"
    );
    cluster
}

fn semantic_element<'a>(
    document: &'a roxmltree::Document<'a>,
    semantic_id: &str,
    element_type: &str,
) -> roxmltree::Node<'a, 'a> {
    let mut matches = document.descendants().filter(|node| {
        node.attribute("data-id") == Some(semantic_id)
            && node.attribute("data-et") == Some(element_type)
    });
    let element = matches
        .next()
        .unwrap_or_else(|| panic!("missing {element_type} {semantic_id}"));
    assert!(
        matches.next().is_none(),
        "duplicate {element_type} {semantic_id}"
    );
    element
}

fn assert_canonical_id(node: roxmltree::Node<'_, '_>, diagram_id: &str, local_kind: &str) {
    let prefix = format!("{diagram_id}-merman-flowchart-document-{local_kind}-");
    let ordinal = node
        .attribute("id")
        .unwrap()
        .strip_prefix(&prefix)
        .expect("DOM identity belongs to its canonical diagram namespace and element owner");
    assert!(!ordinal.is_empty() && ordinal.bytes().all(|byte| byte.is_ascii_digit()));
}

fn canonical_cluster<'a>(
    document: &'a roxmltree::Document<'a>,
    diagram_id: &str,
    semantic_id: &str,
) -> roxmltree::Node<'a, 'a> {
    let cluster = semantic_element(document, semantic_id, "cluster");
    assert!(has_class(cluster, "cluster"));
    assert_canonical_id(
        cluster,
        diagram_id,
        if has_class(cluster, "swimlane") {
            "lane"
        } else {
            "cluster"
        },
    );
    cluster
}

fn has_class(node: roxmltree::Node<'_, '_>, class: &str) -> bool {
    node.attribute("class")
        .is_some_and(|classes| classes.split_ascii_whitespace().any(|value| value == class))
}

fn element_by_id<'a>(document: &'a roxmltree::Document<'a>, id: &str) -> roxmltree::Node<'a, 'a> {
    document
        .descendants()
        .find(|node| node.attribute("id") == Some(id))
        .unwrap_or_else(|| panic!("missing element {id}"))
}

fn node_element<'a>(
    document: &'a roxmltree::Document<'a>,
    diagram_id: &str,
    node_id: &str,
) -> roxmltree::Node<'a, 'a> {
    let node = semantic_element(document, node_id, "node");
    assert!(has_class(node, "node"));
    assert_canonical_id(node, diagram_id, "node");
    node
}

fn text_content(node: roxmltree::Node<'_, '_>) -> String {
    node.descendants()
        .filter_map(|child| child.is_text().then(|| child.text()).flatten())
        .collect()
}

#[test]
fn unchanged_pinned_routes_keep_edge_labels_at_layout_anchors_inside_the_root() {
    for (source, edge_id) in [
        (
            include_str!("../../../fixtures/swimlane/upstream_7_car_sales_constr.mmd"),
            "L_I_K_0",
        ),
        (
            include_str!("../../../fixtures/swimlane/upstream_docs_basic.mmd"),
            "L_triage_investigate_0",
        ),
    ] {
        let (svg, layout) = render_swimlane_with_layout(source, "pinned-label");
        let document = roxmltree::Document::parse(&svg).unwrap();
        let edge = layout.edges.iter().find(|edge| edge.id == edge_id).unwrap();
        assert_eq!(edge.points[0].x, edge.points[1].x);
        assert_eq!(edge.points[0].y, edge.points[1].y);
        let label_id = edge.label_node_id.as_deref().unwrap();
        let label = layout
            .nodes
            .iter()
            .find(|node| node.id == label_id)
            .unwrap();
        let element = semantic_element(&document, edge_id, "edge-label");
        assert_canonical_id(element, "pinned-label", "synthetic-label");
        let coordinates = element
            .attribute("transform")
            .unwrap()
            .trim_start_matches("translate(")
            .trim_end_matches(')')
            .split(',')
            .map(|coordinate| coordinate.trim().parse::<f64>().unwrap())
            .collect::<Vec<_>>();
        // Mermaid 12.2 resolveEdgeLabelPosition keeps the prepared anchor when
        // insertEdge returns only originalPath, even when clipping removes pins.
        assert!((coordinates[0] - label.x).abs() < 1e-6, "{edge_id} x");
        assert!((coordinates[1] - label.y).abs() < 1e-6, "{edge_id} y");
        let viewport = document
            .root_element()
            .attribute("viewBox")
            .unwrap()
            .split_ascii_whitespace()
            .map(|value| value.parse::<f64>().unwrap())
            .collect::<Vec<_>>();
        assert!(coordinates[0] - label.label_width / 2.0 >= viewport[0]);
        assert!(coordinates[1] - label.label_height / 2.0 >= viewport[1]);
        assert!(coordinates[0] + label.label_width / 2.0 <= viewport[0] + viewport[2]);
        assert!(coordinates[1] + label.label_height / 2.0 <= viewport[1] + viewport[3]);
    }
}

#[test]
fn synthetic_lane_palette_follows_visible_container_declaration_slots() {
    let reference = roxmltree::Document::parse(NESTED_MIXED_UPSTREAM_SVG).unwrap();
    let reference_diagram_id = "local_nested_mixed_lane_security";
    // The Mermaid 12.2 SVG records preorder slots for Outer and ordinary Inner;
    // prepareLayoutForSwimlanes then appends the synthetic lane after both slots.
    for (id, expected) in [
        (format!("{reference_diagram_id}-Outer"), "color-0"),
        (format!("{reference_diagram_id}-Inner"), "color-1"),
        ("__swimlane_default__".to_string(), "color-2"),
    ] {
        assert_eq!(
            element_by_id(&reference, &id).attribute("data-color-id"),
            Some(expected)
        );
    }
    for theme in ["redux-color", "redux-dark-color"] {
        for html_labels in [false, true] {
            let source = NESTED_MIXED
                .replace(
                    "securityLevel: strict",
                    &format!("theme: {theme}\n  look: neo\n  securityLevel: strict"),
                )
                .replace("htmlLabels: false", &format!("htmlLabels: {html_labels}"));
            let diagram_id = "nested-colors";
            let svg = render_swimlane(&source, diagram_id);
            let document = roxmltree::Document::parse(&svg).unwrap();
            for id in ["Outer", "Inner", "__swimlane_default__"] {
                let scoped_id = |diagram_id: &str| {
                    if id == "__swimlane_default__" {
                        id.to_string()
                    } else {
                        format!("{diagram_id}-{id}")
                    }
                };
                assert_eq!(
                    canonical_cluster(&document, diagram_id, id).attribute("data-color-id"),
                    element_by_id(&reference, &scoped_id(reference_diagram_id))
                        .attribute("data-color-id"),
                    "{theme}, htmlLabels={html_labels}, {id}"
                );
            }
        }
    }
}

#[test]
fn distinct_nested_and_synthetic_lanes_preserve_ownership_ids_and_cross_lane_routes() {
    // Upstream prepares only top-level groups as Swimlanes, then synthesizes the
    // loose-node lane after render.ts assigns DOM IDs to the explicit groups.
    for (direction, expected_direction) in
        [("LR", SwimlaneDirection::Lr), ("RL", SwimlaneDirection::Rl)]
    {
        let source =
            NESTED_MIXED.replace("swimlane-beta LR", &format!("swimlane-beta {direction}"));
        for diagram_id in ["nested-first", "nested-second"] {
            let (svg, layout) = render_swimlane_with_layout(&source, diagram_id);
            assert_eq!(layout.direction, expected_direction);
            let document = roxmltree::Document::parse(&svg).expect("valid mixed-lane SVG");
            let outer = canonical_lane_cluster(&document, diagram_id, "Outer");
            let inner = canonical_cluster(&document, diagram_id, "Inner");
            let synthetic = canonical_lane_cluster(&document, diagram_id, "__swimlane_default__");
            assert!(has_class(outer, "cluster") && has_class(outer, "swimlane"));
            assert!(has_class(inner, "cluster") && !has_class(inner, "swimlane"));
            assert!(has_class(synthetic, "cluster") && has_class(synthetic, "swimlane"));
            assert_eq!(outer.attribute("data-id"), Some("Outer"));
            assert_eq!(synthetic.attribute("data-id"), Some("__swimlane_default__"));
            let ids = document
                .descendants()
                .filter_map(|node| node.attribute("id"))
                .collect::<Vec<_>>();
            let unique = ids
                .iter()
                .copied()
                .collect::<std::collections::HashSet<_>>();
            assert_eq!(ids.len(), unique.len(), "duplicate DOM IDs");

            let lane = |id: &str| layout.lanes.iter().find(|lane| lane.id == id).unwrap();
            assert!(lane("Outer").parent_id.is_none());
            assert_eq!(lane("Inner").parent_id.as_deref(), Some("Outer"));
            assert!(lane("__swimlane_default__").parent_id.is_none());
            let node = |id: &str| layout.nodes.iter().find(|node| node.id == id).unwrap();
            for id in ["A", "B"] {
                assert_eq!(node(id).parent_id.as_deref(), Some("Inner"));
                assert_eq!(node(id).top_lane_id.as_deref(), Some("Outer"));
                node_element(&document, diagram_id, id);
            }
            assert_eq!(node("C").parent_id.as_deref(), Some("__swimlane_default__"));
            assert_eq!(
                node("C").top_lane_id.as_deref(),
                Some("__swimlane_default__")
            );
            node_element(&document, diagram_id, "C");
            if direction == "LR" {
                assert!(node("A").x < node("B").x);
            } else {
                assert!(node("A").x > node("B").x);
            }
            let edge = layout
                .edges
                .iter()
                .find(|edge| edge.from == "B" && edge.to == "C")
                .unwrap();
            assert!(edge.points.len() >= 2);
            assert!(
                edge.points
                    .iter()
                    .all(|point| point.x.is_finite() && point.y.is_finite())
            );
            assert!(edge.points.windows(2).all(|pair| {
                (pair[0].x - pair[1].x).abs() < 1e-6 || (pair[0].y - pair[1].y).abs() < 1e-6
            }));
            for (id, endpoint) in [
                ("B", edge.points.first().unwrap()),
                ("C", edge.points.last().unwrap()),
            ] {
                let bounds = node(id);
                let dx = (endpoint.x - bounds.x).abs();
                let dy = (endpoint.y - bounds.y).abs();
                assert!(dx <= bounds.width / 2.0 + 1e-6 && dy <= bounds.height / 2.0 + 1e-6);
                assert!(
                    (dx - bounds.width / 2.0).abs() < 1e-6
                        || (dy - bounds.height / 2.0).abs() < 1e-6,
                    "{id}: {endpoint:?} is not on the node boundary"
                );
            }
            assert!(document.descendants().any(|node| {
                node.has_tag_name("path")
                    && node.attribute("data-id") == Some(edge.id.as_str())
                    && node.attribute("data-points").is_some()
                    && node
                        .attribute("d")
                        .is_some_and(|path| path.starts_with('M'))
            }));
        }
    }
}

#[test]
fn nested_mixed_lane_titles_and_nodes_escape_entities_and_remove_strict_unsafe_links() {
    for html_labels in [false, true] {
        let source =
            NESTED_MIXED.replace("htmlLabels: false", &format!("htmlLabels: {html_labels}"));
        let diagram_id = "nested-strict";
        let svg = render_swimlane(&source, diagram_id);
        let document = roxmltree::Document::parse(&svg).expect("valid escaped mixed-lane SVG");
        for (id, expected) in [("Outer", "Outer & < lane"), ("Inner", "Inner & < group")] {
            let cluster = canonical_cluster(&document, diagram_id, id);
            let title = cluster
                .children()
                .find(|node| has_class(*node, "cluster-label"))
                .unwrap();
            assert_eq!(text_content(title), expected);
        }
        for (id, expected) in [
            ("A", "A & < node"),
            ("B", "B & < node"),
            ("C", "Loose & < node"),
        ] {
            assert_eq!(
                text_content(node_element(&document, diagram_id, id)),
                expected
            );
        }
        assert!(document.descendants().all(|node| {
            !node.has_tag_name("script")
                && node.attributes().all(|attribute| {
                    !attribute.name().starts_with("on")
                        && !attribute.value().contains("javascript:")
                })
        }));
        let linked = node_element(&document, diagram_id, "A");
        let anchor = linked
            .ancestors()
            .find(|node| node.has_tag_name("a"))
            .expect("declared link keeps the upstream anchor wrapper");
        assert!(
            anchor
                .attributes()
                .all(|attribute| attribute.name() != "href")
        );
        assert_eq!(
            linked
                .descendants()
                .any(|node| node.has_tag_name("foreignObject")),
            html_labels
        );
    }
}
