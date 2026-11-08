#![cfg(feature = "diagram-er")]

use merman_core::{MermaidEngine, ParseOptions};
use merman_model::families::er::{ErDiagramModel, ErEntity, ErRelationship, ErSubgraph};
use merman_model::presentation::{
    AuthoredPresentationMetadata, PresentationContext, PresentationDiagram, RenderSemanticModel,
};
use merman_model::resources::InputResourcePolicy;
use merman_model::{MermaidConfig, OperationControl};
use merman_render::LayoutOptions;
use merman_render::environment::RenderEnvironment;
use merman_render::family::{self, FamilyRenderArtifact};
use merman_render::model::{ErDiagramLayout, LayoutNode};
use merman_render::svg::{SvgDebugOptions, SvgRenderOptions};
use serde_json::json;

fn parse_er(text: &str) -> PresentationDiagram {
    MermaidEngine::new()
        .parse_diagram_for_render_model_sync(text, ParseOptions::default())
        .expect("parse ER fixture")
        .expect("detect ER diagram")
        .into_presentation()
}

fn prepare_er(parsed: &PresentationDiagram) -> FamilyRenderArtifact<'_> {
    let session = RenderEnvironment::deterministic().begin_session().unwrap();
    family::prepare(parsed, &LayoutOptions::default(), session).expect("prepare ER Swimlane")
}

fn project_layout(artifact: &FamilyRenderArtifact<'_>) -> ErDiagramLayout {
    let projection = artifact.layout_json().expect("serialize ER layout");
    serde_json::from_value(projection["layout"]["ErDiagram"].clone())
        .expect("project ER family layout")
}

fn layout_er(text: &str) -> ErDiagramLayout {
    let parsed = parse_er(text);
    project_layout(&prepare_er(&parsed))
}

fn fixture(body: &str, direction: &str, layout: &str) -> String {
    format!("---\nconfig:\n  layout: {layout}\n---\nerDiagram\ndirection {direction}\n{body}\n")
}

fn node<'a>(layout: &'a ErDiagramLayout, id: &str) -> &'a LayoutNode {
    layout
        .nodes
        .iter()
        .find(|node| node.id == id)
        .unwrap_or_else(|| panic!("missing ER node {id}"))
}

fn assert_contains(group: &LayoutNode, child: &LayoutNode) {
    let epsilon = 1.0e-6;
    assert!(group.x - group.width / 2.0 <= child.x - child.width / 2.0 + epsilon);
    assert!(group.x + group.width / 2.0 + epsilon >= child.x + child.width / 2.0);
    assert!(group.y - group.height / 2.0 <= child.y - child.height / 2.0 + epsilon);
    assert!(group.y + group.height / 2.0 + epsilon >= child.y + child.height / 2.0);
}

#[test]
fn er_swimlane_confirmed_basic_and_compound_fixtures_select_the_registered_provider() {
    // Mermaid 12.3 runtime batch 6: er-07-swimlane-layout and er-09-swimlane-basic.
    for (body, group_id, entity_count) in [
        (
            "subgraph Group\nA\nB\nend\nA ||--|| B : connects",
            "Group",
            2,
        ),
        (
            "A ||--|| B : ab\nA ||--|| C : ac",
            "__swimlane_default__",
            3,
        ),
    ] {
        let source = fixture(body, "TB", "swimlane");
        let parsed = parse_er(&source);
        let artifact = prepare_er(&parsed);
        let layout = project_layout(&artifact);
        assert_eq!(
            layout.nodes.iter().filter(|node| !node.is_cluster).count(),
            entity_count
        );
        let lane = node(&layout, group_id);
        assert!(lane.is_cluster);
        for entity in layout.nodes.iter().filter(|node| !node.is_cluster) {
            assert_contains(lane, entity);
        }
        assert_eq!(layout.clusters.len(), 1);
        assert!(layout.clusters[0].width > 0.0 && layout.clusters[0].height > 0.0);
        for edge in &layout.edges {
            assert!(edge.points.len() >= 2);
            assert!(
                edge.points
                    .iter()
                    .all(|point| point.x.is_finite() && point.y.is_finite())
            );
            let label = edge.label.as_ref().expect("measured relationship label");
            assert!(label.x.is_finite() && label.y.is_finite());
            assert!(label.width > 0.0 && label.height > 0.0);
            assert_eq!(edge.start_marker.as_deref(), Some("ONLY_ONE_START"));
            assert_eq!(edge.end_marker.as_deref(), Some("ONLY_ONE_END"));
        }
        let svg = artifact
            .render_svg(
                &SvgRenderOptions {
                    diagram_id: Some("er-swimlane".into()),
                    ..Default::default()
                },
                &SvgDebugOptions::default(),
            )
            .expect("render selected ER Swimlane provider")
            .svg()
            .to_string();
        let document = roxmltree::Document::parse(&svg).expect("well-formed ER Swimlane SVG");
        assert!(document.descendants().any(|element| {
            element.has_tag_name("path")
                && element.attribute("marker-start").is_some()
                && element.attribute("marker-end").is_some()
        }));
        assert!(svg.contains(if group_id == "Group" {
            "connects"
        } else {
            "ab"
        }));

        let dagre = layout_er(&fixture(body, "TB", "dagre"));
        assert_ne!(
            serde_json::to_value(&layout).unwrap(),
            serde_json::to_value(&dagre).unwrap(),
            "explicit Swimlane must not silently execute Dagre",
        );
    }
}

#[test]
fn er_swimlane_retains_entity_measurements_and_supports_all_four_directions() {
    let body = "CUSTOMER {\n int id PK\n string display_name\n}\nCUSTOMER ||--o{ ORDER : places\nORDER {\n int id PK\n int customer_id FK\n}";
    let dagre = layout_er(&fixture(body, "TB", "dagre"));
    for direction in ["TB", "BT", "LR", "RL"] {
        let layout = layout_er(&fixture(body, direction, "swimlane"));
        for expected in dagre.nodes.iter().filter(|node| !node.is_cluster) {
            let actual = node(&layout, &expected.id);
            assert_eq!(
                (actual.width, actual.height),
                (expected.width, expected.height)
            );
        }
        let from = node(&layout, "entity-CUSTOMER-0");
        let to = node(&layout, "entity-ORDER-1");
        assert!(
            match direction {
                "TB" => from.y < to.y,
                "BT" => from.y > to.y,
                "LR" => from.x < to.x,
                "RL" => from.x > to.x,
                _ => unreachable!(),
            },
            "ER direction {direction} must reach Swimlane"
        );
        assert_contains(node(&layout, "__swimlane_default__"), from);
        assert_contains(node(&layout, "__swimlane_default__"), to);
        let edge = &layout.edges[0];
        assert_eq!(edge.from, from.id);
        assert_eq!(edge.to, to.id);
        let label = edge.label.as_ref().unwrap();
        assert_eq!(
            (label.width, label.height),
            (
                dagre.edges[0].label.as_ref().unwrap().width,
                dagre.edges[0].label.as_ref().unwrap().height
            ),
        );
    }
}

#[test]
fn er_swimlane_keeps_nested_compound_ids_loose_entities_and_group_endpoints() {
    let body = concat!(
        "subgraph Outer[Customer Domain]\n",
        "subgraph Inner[Accounts]\nA\nB\nend\n",
        "C\nend\n",
        "subgraph Warehouse[Warehouse Domain]\nD\nend\n",
        "A ||--o{ B : owns\n",
        "C ||--|| D : references\n",
        "OUTSIDE ||--|| Warehouse : supplies\n",
    );
    for direction in ["TB", "LR"] {
        let source = fixture(body, direction, "swimlane");
        let parsed = parse_er(&source);
        let artifact = prepare_er(&parsed);
        let layout = project_layout(&artifact);
        let outer = node(&layout, "Outer");
        let inner = node(&layout, "Inner");
        let warehouse = node(&layout, "Warehouse");
        assert_contains(outer, inner);
        assert_contains(inner, node(&layout, "entity-A-0"));
        assert_contains(inner, node(&layout, "entity-B-1"));
        assert_contains(outer, node(&layout, "entity-C-2"));
        assert_contains(warehouse, node(&layout, "entity-D-3"));
        assert_contains(
            node(&layout, "__swimlane_default__"),
            node(&layout, "entity-OUTSIDE-4"),
        );
        assert_eq!(layout.clusters.len(), 4);
        let outer_cluster = layout
            .clusters
            .iter()
            .find(|group| group.id == "Outer")
            .unwrap();
        assert_eq!(outer_cluster.title, "Customer Domain");
        assert_eq!(outer_cluster.effective_dir, direction);
        assert!(layout.edges.iter().any(|edge| {
            edge.to == "Warehouse"
                && edge.to_cluster.as_deref() == Some("Warehouse")
                && edge.label.is_some()
                && edge.points.len() >= 2
        }));
        assert!(
            layout
                .nodes
                .iter()
                .all(|node| !node.id.starts_with("edge-label-"))
        );
        let svg = artifact
            .render_svg(
                &SvgRenderOptions {
                    diagram_id: Some("er-swimlane-compound".into()),
                    ..Default::default()
                },
                &SvgDebugOptions::default(),
            )
            .expect("paint nested ER compounds")
            .svg()
            .to_string();
        let document = roxmltree::Document::parse(&svg).unwrap();
        let outer = document
            .descendants()
            .find(|element| element.attribute("data-id") == Some("Outer"))
            .unwrap();
        let inner = document
            .descendants()
            .find(|element| element.attribute("id") == Some("er-swimlane-compound-Inner"))
            .unwrap();
        assert!(has_class(outer, "swimlane"));
        assert!(
            has_class(inner, "cluster") && !has_class(inner, "swimlane"),
            "only top-level ER groups become lanes; nested compounds retain ordinary boxes"
        );
    }
}

#[test]
fn er_swimlane_preserves_parallel_cycle_and_empty_label_relationships() {
    let source = fixture(
        concat!(
            "A ||..o{ B : first\n",
            "A |o--|| B : second\n",
            "B ||--|| A : reverse\n",
            "A ||--|| B : \"\"\n",
        ),
        "TB",
        "swimlane",
    );
    let parsed = parse_er(&source);
    let artifact = prepare_er(&parsed);
    let layout = project_layout(&artifact);
    assert_eq!(layout.edges.len(), 4);
    assert_eq!(
        layout.nodes.iter().filter(|node| !node.is_cluster).count(),
        2
    );
    assert!(layout.nodes.iter().all(|node| !node.id.contains("---")));
    for (index, edge) in layout.edges.iter().enumerate() {
        assert_eq!(edge.id, format!("er-rel-{index}"));
        assert!(
            edge.points.len() >= 2,
            "relationship {index} must be routed"
        );
        assert!(
            edge.points
                .iter()
                .all(|point| point.x.is_finite() && point.y.is_finite())
        );
        assert_eq!(edge.label.is_some(), index != 3);
    }
    assert_eq!(layout.edges[0].stroke_dasharray.as_deref(), Some("8,8"));
    assert!(layout.edges[1].stroke_dasharray.is_none());
    assert_eq!(
        layout.edges[0].start_marker.as_deref(),
        Some("ONLY_ONE_START")
    );
    assert_eq!(
        layout.edges[0].end_marker.as_deref(),
        Some("ZERO_OR_MORE_END")
    );
    assert_eq!(
        layout.edges[1].start_marker.as_deref(),
        Some("ZERO_OR_ONE_START")
    );
    assert_eq!(layout.edges[1].end_marker.as_deref(), Some("ONLY_ONE_END"));
    assert_ne!(
        (
            layout.edges[0].label.as_ref().unwrap().x,
            layout.edges[0].label.as_ref().unwrap().y
        ),
        (
            layout.edges[1].label.as_ref().unwrap().x,
            layout.edges[1].label.as_ref().unwrap().y
        ),
        "parallel relationships need separate measured label anchors",
    );
    let svg = artifact
        .render_svg(&SvgRenderOptions::default(), &SvgDebugOptions::default())
        .expect("render parallel and feedback Swimlane relationships")
        .svg()
        .to_string();
    for label in ["first", "second", "reverse"] {
        assert!(svg.contains(label), "retain relationship label {label}");
    }
    assert_eq!(
        serde_json::to_value(&layout).unwrap(),
        serde_json::to_value(layout_er(&source)).unwrap(),
        "Swimlane relationship ownership and geometry must be deterministic",
    );
}

#[test]
fn er_swimlane_rejects_upstream_unsupported_self_loops_without_falling_back() {
    // Mermaid 12.3 batch 9 accepts parsing but its selected Swimlane renderer fails.
    let body = "A ||--o{ A : recursive";
    let source = fixture(body, "TB", "swimlane");
    let parsed = parse_er(&source);
    let session = RenderEnvironment::deterministic().begin_session().unwrap();
    let error = family::prepare(&parsed, &LayoutOptions::default(), session)
        .err()
        .expect("the selected provider must reject its unsupported self-loop");
    assert!(
        matches!(error, merman_render::Error::InvalidModel { message }
        if message.contains("Swimlane") && message.contains("self-loop"))
    );
    assert!(!layout_er(&fixture(body, "TB", "dagre")).edges.is_empty());
}

#[test]
fn er_swimlane_spacing_uses_er_renderer_values_and_ignores_root_and_flowchart_overrides() {
    let source = |config: serde_json::Value| {
        format!(
            "---\nconfig: {}\n---\nerDiagram\nA ||--|| B : links\nB ||--|| C : continues\n",
            serde_json::to_string(&config).unwrap(),
        )
    };
    let default = layout_er(&source(json!({"layout": "swimlane"})));
    let overridden = layout_er(&source(json!({
        "layout": "swimlane", "nodeSpacing": 601, "rankSpacing": 603,
        "flowchart": {"nodeSpacing": 701, "rankSpacing": 703},
    })));
    assert_eq!(
        serde_json::to_value(&default).unwrap(),
        serde_json::to_value(overridden).unwrap()
    );
    let spaced = layout_er(&source(
        json!({"layout": "swimlane", "er": {"rankSpacing": 180}}),
    ));
    let span =
        |layout: &ErDiagramLayout| node(layout, "entity-C-2").y - node(layout, "entity-A-0").y;
    assert!(
        span(&spaced) > span(&default),
        "ER rank spacing reaches Swimlane's own core"
    );
}

fn present_model(model: ErDiagramModel) -> PresentationDiagram {
    let config = MermaidConfig::from_value(json!({"layout": "swimlane"}));
    PresentationDiagram::try_new(
        AuthoredPresentationMetadata {
            diagram_type: "erDiagram".into(),
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
    .expect("admit direct ER model")
}

#[test]
fn er_swimlane_rejects_compound_parent_cycles_and_preserves_cancellation() {
    let model = ErDiagramModel {
        direction: "TB".into(),
        subgraphs: [("Left", "Right"), ("Right", "Left")]
            .into_iter()
            .map(|(id, child)| ErSubgraph {
                id: id.into(),
                title: id.into(),
                nodes: vec![child.into()],
                ..Default::default()
            })
            .collect(),
        ..Default::default()
    };
    let parsed = present_model(model);
    let session = RenderEnvironment::deterministic().begin_session().unwrap();
    let error = family::prepare(&parsed, &LayoutOptions::default(), session)
        .err()
        .expect("parent cycle must be rejected before ancestor traversal");
    assert!(
        matches!(error, merman_render::Error::InvalidModel { message } if message.contains("parent cycle"))
    );

    let control = OperationControl::new();
    let session = RenderEnvironment::deterministic()
        .begin_session_with_control(control.clone())
        .unwrap();
    control.cancel();
    assert!(matches!(
        family::prepare(&parsed, &LayoutOptions::default(), session),
        Err(merman_render::Error::Cancelled(_)),
    ));
}

#[test]
fn er_swimlane_label_helper_id_collision_keeps_the_authored_compound() {
    let label_id = "edge-label-A-B-er-rel-0";
    let model = ErDiagramModel {
        direction: "TB".into(),
        entities: ["A", "B"]
            .into_iter()
            .map(|id| {
                (
                    id.to_string(),
                    ErEntity {
                        id: id.into(),
                        label: id.into(),
                        ..Default::default()
                    },
                )
            })
            .collect(),
        subgraphs: vec![ErSubgraph {
            id: label_id.into(),
            title: "Authored group".into(),
            nodes: vec!["A".into(), "B".into()],
            ..Default::default()
        }],
        relationships: vec![ErRelationship {
            entity_a: "A".into(),
            entity_b: "B".into(),
            role_a: "connects".into(),
            ..Default::default()
        }],
        ..Default::default()
    };
    let parsed = present_model(model);
    let layout = project_layout(&prepare_er(&parsed));
    assert!(node(&layout, label_id).is_cluster);
    assert_eq!(layout.clusters[0].title, "Authored group");
    assert_contains(node(&layout, label_id), node(&layout, "A"));
    assert_contains(node(&layout, label_id), node(&layout, "B"));
    assert_eq!(layout.edges.len(), 1);
    assert_eq!(layout.edges[0].from, "A");
    assert_eq!(layout.edges[0].to, "B");
    assert!(layout.edges[0].label.as_ref().unwrap().width > 0.0);
}

fn has_class(element: roxmltree::Node<'_, '_>, class: &str) -> bool {
    element
        .attribute("class")
        .unwrap_or("")
        .split_whitespace()
        .any(|value| value == class)
}

fn svg_number(element: roxmltree::Node<'_, '_>, name: &str) -> f64 {
    element
        .attribute(name)
        .unwrap_or_else(|| panic!("missing SVG {name}"))
        .parse()
        .unwrap()
}

#[test]
fn er_swimlane_svg_paints_source_lane_shells_rounded_edges_and_prepared_label_nodes() {
    for (direction, html_labels) in [("TB", true), ("LR", false)] {
        let source = format!(
            concat!(
                "---\nconfig:\n  layout: swimlane\n  look: classic\n",
                "  htmlLabels: {html_labels}\n  flowchart:\n    htmlLabels: {html_labels}\n",
                "  swimlane:\n    lineHops: false\n---\nerDiagram\ndirection {direction}\n",
                "subgraph Group[Customer Domain]\nA\nB\nend\n",
                "A ||--o{{ B : owns\nA ||--|| OUTSIDE : references\nB ||--|| OUTSIDE : connects\n",
            ),
            html_labels = html_labels,
            direction = direction
        );
        let parsed = parse_er(&source);
        let artifact = prepare_er(&parsed);
        let layout = project_layout(&artifact);
        let svg = artifact
            .render_svg(
                &SvgRenderOptions {
                    diagram_id: Some("er-swimlane-paint".into()),
                    ..Default::default()
                },
                &SvgDebugOptions::default(),
            )
            .expect("render prepared ER Swimlane geometry")
            .svg()
            .to_string();
        let document = roxmltree::Document::parse(&svg).expect("well-formed Swimlane paint");
        for (id, expected_dom_id) in [
            ("Group", "er-swimlane-paint-Group"),
            ("__swimlane_default__", "__swimlane_default__"),
        ] {
            let lane = document
                .descendants()
                .find(|element| {
                    element.has_tag_name("g")
                        && has_class(*element, "swimlane")
                        && element.attribute("data-id") == Some(id)
                })
                .expect("paint authored and synthesized lanes");
            assert_eq!(lane.attribute("id"), Some(expected_dom_id));
            assert_eq!(lane.attribute("data-et"), Some("cluster"));
            let title = lane
                .children()
                .find(|element| has_class(*element, "swimlane-title"))
                .unwrap();
            let body = lane
                .children()
                .find(|element| has_class(*element, "swimlane-body"))
                .unwrap();
            assert!(title.has_tag_name("rect") && body.has_tag_name("rect"));
            assert_eq!(body.attribute("fill"), Some("none"));
            assert!(title.attribute("stroke").is_some());
            let label = lane
                .children()
                .find(|element| has_class(*element, "swimlane-label"))
                .unwrap();
            if direction == "LR" {
                assert!(
                    (svg_number(title, "x") + svg_number(title, "width") - svg_number(body, "x"))
                        .abs()
                        < 1.0e-4
                );
                assert!(
                    label
                        .attribute("transform")
                        .unwrap()
                        .contains("rotate(-90)")
                );
            } else {
                assert!(
                    (svg_number(title, "y") + svg_number(title, "height") - svg_number(body, "y"))
                        .abs()
                        < 1.0e-4
                );
                assert!(!label.attribute("transform").unwrap().contains("rotate"));
                if id == "__swimlane_default__" {
                    assert_eq!(svg_number(title, "height"), 0.0);
                }
            }
            assert_eq!(
                label
                    .descendants()
                    .any(|element| element.has_tag_name("foreignObject")),
                html_labels
            );
        }
        let edge_paths = document
            .descendants()
            .filter(|element| {
                element.has_tag_name("path") && element.attribute("data-edge") == Some("true")
            })
            .collect::<Vec<_>>();
        assert_eq!(edge_paths.len(), layout.edges.len());
        assert!(
            edge_paths
                .iter()
                .any(|path| path.attribute("d").unwrap().contains('Q'))
        );
        assert!(
            edge_paths
                .iter()
                .all(|path| !path.attribute("d").unwrap().contains('C')),
            "Swimlane must paint rounded routes instead of Dagre's basis splines"
        );
        assert!(
            edge_paths
                .iter()
                .all(|path| !path.attribute("d").unwrap().contains('A')),
            "lineHops:false bypasses the after-paint line-hop processor"
        );
        let labels = document
            .descendants()
            .filter(|element| {
                element.has_tag_name("g")
                    && has_class(*element, "label")
                    && has_class(*element, "edgeLabel")
                    && element
                        .attribute("id")
                        .is_some_and(|id| id.starts_with("edge-label-"))
            })
            .collect::<Vec<_>>();
        assert_eq!(labels.len(), layout.edges.len());
        for (index, edge) in layout.edges.iter().enumerate() {
            let id = format!(
                "edge-label-{}-{}-id_{}_{}_{}",
                edge.from, edge.to, edge.from, edge.to, index
            );
            let label_node = labels
                .iter()
                .find(|node| node.attribute("id") == Some(id.as_str()))
                .unwrap();
            let transform = label_node.attribute("transform").unwrap();
            let coordinates = transform
                .strip_prefix("translate(")
                .unwrap()
                .strip_suffix(')')
                .unwrap()
                .split(',')
                .map(|value| value.trim().parse::<f64>().unwrap())
                .collect::<Vec<_>>();
            let label = edge.label.as_ref().unwrap();
            assert!((coordinates[0] - label.x).abs() < 1.0e-4);
            assert!((coordinates[1] - label.y).abs() < 1.0e-4);
            assert!(
                label_node
                    .ancestors()
                    .any(|ancestor| has_class(ancestor, "nodes"))
            );
        }
        let ordinary_labels = document
            .descendants()
            .find(|element| has_class(*element, "edgeLabels"))
            .unwrap();
        assert_eq!(
            ordinary_labels
                .children()
                .filter(|child| child.is_element())
                .count(),
            0,
            "relationship text is owned by the prepared Swimlane label nodes"
        );
    }
}

#[test]
fn er_swimlane_title_html_labels_follow_flowchart_and_hand_drawn_lanes_keep_both_shells() {
    for (look, title_html) in [("classic", true), ("handDrawn", false)] {
        let source = format!(
            concat!(
                "---\nconfig:\n  layout: swimlane\n  look: {look}\n  handDrawnSeed: 42\n",
                "  htmlLabels: false\n  flowchart:\n    htmlLabels: {title_html}\n",
                "  swimlane:\n    lineHops: false\n---\nerDiagram\n",
                "subgraph Group[**Customer Domain**]\nA\nB\nend\nA ||--|| B : connects\n",
            ),
            look = look,
            title_html = title_html
        );
        let parsed = parse_er(&source);
        let artifact = prepare_er(&parsed);
        let svg = artifact
            .render_svg(&SvgRenderOptions::default(), &SvgDebugOptions::default())
            .expect("render ER Swimlane source title mode and look")
            .svg()
            .to_string();
        let document = roxmltree::Document::parse(&svg).unwrap();
        let label = document
            .descendants()
            .find(|element| has_class(*element, "swimlane-label"))
            .unwrap();
        assert_eq!(
            label
                .descendants()
                .any(|element| element.has_tag_name("foreignObject")),
            title_html,
            "Swimlane title mode follows flowchart.htmlLabels independently of ER entity mode"
        );
        for class in ["swimlane-title", "swimlane-body"] {
            let shell = document
                .descendants()
                .find(|element| has_class(*element, class))
                .unwrap();
            if look == "handDrawn" {
                assert!(shell.has_tag_name("g"));
                assert!(
                    shell
                        .descendants()
                        .any(|element| element.has_tag_name("path"))
                );
            } else {
                assert!(shell.has_tag_name("rect"));
            }
        }
    }
}

#[test]
fn er_swimlane_relationship_labels_keep_literal_markdown_in_html_and_svg() {
    // Mermaid 12.3 batch 8 cases 10 and 11 preserve the asterisks in both modes.
    for html in [true, false] {
        let source = format!(
            "---\nconfig: {{\"layout\":\"swimlane\",\"htmlLabels\":{html}}}\n---\nerDiagram\nA ||--|| B : \"**owns**\"\n",
        );
        let parsed = parse_er(&source);
        let artifact = prepare_er(&parsed);
        let layout = project_layout(&artifact);
        let svg = artifact
            .render_svg(&SvgRenderOptions::default(), &SvgDebugOptions::default())
            .expect("paint literal Swimlane relationship text")
            .svg()
            .to_string();
        let document = roxmltree::Document::parse(&svg).unwrap();
        let label = document
            .descendants()
            .find(|element| {
                element.has_tag_name("g")
                    && has_class(*element, "edgeLabel")
                    && element
                        .attribute("id")
                        .is_some_and(|id| id.starts_with("edge-label-"))
            })
            .unwrap();
        let visible = label
            .descendants()
            .filter(|element| element.is_text())
            .filter_map(|element| element.text())
            .collect::<String>();
        assert_eq!(visible, "**owns**");
        assert!(
            !label
                .descendants()
                .any(|element| element.has_tag_name("strong"))
        );
        assert_eq!(
            label
                .descendants()
                .any(|element| element.has_tag_name("foreignObject")),
            html
        );
        let plain_source = source.replace("**owns**", "owns");
        let plain = layout_er(&plain_source);
        assert!(
            layout.edges[0].label.as_ref().unwrap().width
                > plain.edges[0].label.as_ref().unwrap().width,
            "prepared width includes the literal Markdown punctuation"
        );
    }
}

#[test]
fn er_swimlane_relationship_nodes_and_lane_titles_resolve_html_modes_independently() {
    for (root, flowchart) in [(None, false), (Some(true), false), (Some(false), true)] {
        let mut config = json!({"layout": "swimlane", "flowchart": {"htmlLabels": flowchart}});
        if let Some(root) = root {
            config["htmlLabels"] = json!(root);
        }
        let source = format!(
            concat!(
                "---\nconfig: {}\n---\nerDiagram\n",
                "subgraph Group[Lane title]\nA\nB\nend\nA ||--|| B : owns\n",
            ),
            serde_json::to_string(&config).unwrap()
        );
        let parsed = parse_er(&source);
        let artifact = prepare_er(&parsed);
        let svg = artifact
            .render_svg(&SvgRenderOptions::default(), &SvgDebugOptions::default())
            .expect("paint independently selected node and title label modes")
            .svg()
            .to_string();
        let document = roxmltree::Document::parse(&svg).unwrap();
        let relationship = document
            .descendants()
            .find(|element| {
                has_class(*element, "edgeLabel")
                    && element
                        .attribute("id")
                        .is_some_and(|id| id.starts_with("edge-label-"))
            })
            .unwrap();
        let title = document
            .descendants()
            .find(|element| has_class(*element, "swimlane-label"))
            .unwrap();
        assert_eq!(
            relationship
                .descendants()
                .any(|element| element.has_tag_name("foreignObject")),
            root.unwrap_or(true),
            "generated labelRect uses root htmlLabels, with undefined defaulting to true"
        );
        assert_eq!(
            title
                .descendants()
                .any(|element| element.has_tag_name("foreignObject")),
            flowchart,
            "the lane title continues to use flowchart.htmlLabels"
        );
    }
}

#[test]
fn er_swimlane_long_relationship_labels_measure_and_paint_the_configured_wrapping_width() {
    let text = "an unusually long relationship label that needs several wrapped lines";
    for html in [true, false] {
        let source = |width: usize| {
            format!(
                "---\nconfig: {{\"layout\":\"swimlane\",\"htmlLabels\":{html},\"flowchart\":{{\"wrappingWidth\":{width}}}}}\n---\nerDiagram\nA ||--|| B : \"{text}\"\n",
            )
        };
        let narrow = source(80);
        let parsed = parse_er(&narrow);
        let artifact = prepare_er(&parsed);
        let layout = project_layout(&artifact);
        let wide = layout_er(&source(1000));
        let label = layout.edges[0].label.as_ref().unwrap();
        assert!(label.height > wide.edges[0].label.as_ref().unwrap().height);
        assert!(label.width <= 80.0 + 1.0e-6);
        let svg = artifact
            .render_svg(&SvgRenderOptions::default(), &SvgDebugOptions::default())
            .expect("paint frozen wrapped Swimlane relationship terminal")
            .svg()
            .to_string();
        let document = roxmltree::Document::parse(&svg).unwrap();
        let terminal = document
            .descendants()
            .find(|element| {
                has_class(*element, "edgeLabel")
                    && element
                        .attribute("id")
                        .is_some_and(|id| id.starts_with("edge-label-"))
            })
            .unwrap();
        if html {
            let foreign = terminal
                .descendants()
                .find(|element| element.has_tag_name("foreignObject"))
                .unwrap();
            assert!((svg_number(foreign, "width") - label.width).abs() < 1.0e-6);
            assert!((svg_number(foreign, "height") - label.height).abs() < 1.0e-6);
            let div = foreign
                .descendants()
                .find(|element| element.has_tag_name("div"))
                .unwrap();
            let style = div.attribute("style").unwrap();
            assert!(style.contains("white-space: break-spaces") && style.contains("width: 80px"));
        } else {
            let rows = terminal
                .descendants()
                .filter(|element| {
                    element.has_tag_name("tspan") && has_class(*element, "text-outer-tspan")
                })
                .count();
            assert!(
                rows > 1,
                "SVG paints the word rows frozen during provider preparation"
            );
        }
    }
}

fn html_relationship_fixture(label: &str, width: usize) -> String {
    format!(
        "---\nconfig: {{\"layout\":\"swimlane\",\"htmlLabels\":true,\"flowchart\":{{\"wrappingWidth\":{width}}}}}\n---\nerDiagram\nA ||--|| B : \"{label}\"\n",
    )
}

fn assert_relationship_font_viewport(
    artifact: FamilyRenderArtifact<'_>,
    layout: &ErDiagramLayout,
    font_declaration: &str,
) -> String {
    let label = layout.edges[0].label.as_ref().unwrap();
    let svg = artifact
        .render_svg(&SvgRenderOptions::default(), &SvgDebugOptions::default())
        .expect("paint measured inline-font relationship terminal")
        .svg()
        .to_string();
    let document = roxmltree::Document::parse(&svg).unwrap();
    let terminal = document
        .descendants()
        .find(|element| {
            has_class(*element, "edgeLabel")
                && element
                    .attribute("id")
                    .is_some_and(|id| id.starts_with("edge-label-"))
        })
        .unwrap();
    let foreign = terminal
        .descendants()
        .find(|element| element.has_tag_name("foreignObject"))
        .unwrap();
    assert!((svg_number(foreign, "width") - label.width).abs() < 1.0e-6);
    assert!((svg_number(foreign, "height") - label.height).abs() < 1.0e-6);
    assert!(foreign.descendants().any(|element| {
        element.has_tag_name("span")
            && element
                .attribute("style")
                .is_some_and(|style| style.contains(font_declaration))
    }));
    foreign
        .descendants()
        .find(|element| element.has_tag_name("div"))
        .unwrap()
        .attribute("style")
        .unwrap()
        .to_owned()
}

#[test]
fn er_swimlane_relationship_inline_font_reserves_the_retained_font_line_box() {
    // Exact Mermaid 12.3 batch 10 cases 17/18 reserve 21px versus 96px of HTML height.
    let plain = layout_er(&html_relationship_fixture("A", 200));
    let source = html_relationship_fixture("<span style='font-size:64px'>A</span>", 200);
    let parsed = parse_er(&source);
    let artifact = prepare_er(&parsed);
    let layout = project_layout(&artifact);
    let plain_label = plain.edges[0].label.as_ref().unwrap();
    let label = layout.edges[0].label.as_ref().unwrap();
    assert_eq!(plain_label.height, 21.0);
    assert_eq!(label.height, 96.0);
    assert!(label.width > plain_label.width * 4.0);
    assert!(label.width < 200.0);
    let wrapper = assert_relationship_font_viewport(artifact, &layout, "font-size:64px");
    assert!(wrapper.contains("white-space: nowrap"));
}

#[test]
fn er_swimlane_wrapped_inline_font_keeps_each_font_line_box_inside_the_viewport() {
    // Batch 10 confirms a 96px line box. Font shaping may select different wrapped rows;
    // every selected row must reserve its actual font size and share the painted viewport.
    let text = "<span style='font-size:64px'>A B C D E</span>";
    let mut dimensions = Vec::new();
    for width in [1000, 100] {
        let source = html_relationship_fixture(text, width);
        let parsed = parse_er(&source);
        let artifact = prepare_er(&parsed);
        let layout = project_layout(&artifact);
        let label = layout.edges[0].label.as_ref().unwrap();
        assert_eq!(label.height % 96.0, 0.0);
        let wrapper = assert_relationship_font_viewport(artifact, &layout, "font-size:64px");
        if width == 100 {
            assert_eq!(label.width, 100.0);
            assert!(label.height >= 2.0 * 96.0);
            assert!(wrapper.contains("white-space: break-spaces"));
            assert!(wrapper.contains("width: 100px"));
        } else {
            assert_eq!(label.height, 96.0);
            assert!(wrapper.contains("white-space: nowrap"));
        }
        dimensions.push((label.width, label.height));
    }
    assert!(dimensions[0].0 > dimensions[1].0);
    assert!(dimensions[0].1 < dimensions[1].1);
}

#[test]
fn er_swimlane_mixed_inline_fonts_size_only_the_rows_they_occupy() {
    // Exact Mermaid 12.3 batch 10 case 21 is 96 + 21 + 21 = 138px, not 96 * 3.
    for text in [
        "<span style='font-size:64px'>A</span><br/>B<br/>C",
        "A<br/><span style='font-size:64px'>B</span><br/>C",
        "A<br/>B<br/><span style='font-size:64px'>C</span>",
    ] {
        let source = html_relationship_fixture(text, 200);
        let parsed = parse_er(&source);
        let artifact = prepare_er(&parsed);
        let layout = project_layout(&artifact);
        assert_eq!(layout.edges[0].label.as_ref().unwrap().height, 138.0);
        assert_relationship_font_viewport(artifact, &layout, "font-size:64px");
    }
}

#[test]
fn er_swimlane_nested_relative_and_inherited_fonts_share_wrapped_terminal_metrics() {
    let direct = "<span style='font-size:64px'>A B C D E</span>";
    let nested = concat!(
        "<span style='font-size:32px'><span style='font-size:2em'>",
        "<span style='font-size:inherit'>A B C D E</span></span></span>",
    );
    for width in [100, 1000] {
        let expected = layout_er(&html_relationship_fixture(direct, width));
        let source = html_relationship_fixture(nested, width);
        let parsed = parse_er(&source);
        let artifact = prepare_er(&parsed);
        let layout = project_layout(&artifact);
        let label = layout.edges[0].label.as_ref().unwrap();
        let expected = expected.edges[0].label.as_ref().unwrap();
        assert_eq!(
            (label.width, label.height),
            (expected.width, expected.height)
        );
        assert_relationship_font_viewport(artifact, &layout, "font-size:inherit");
    }
}

#[test]
fn er_swimlane_inline_icons_preserve_the_text_font_viewport() {
    for (text, width) in [
        ("<span style='font-size:64px'>A</span>", 1000),
        ("<span style='font-size:64px'>A B C D E</span>", 100),
        ("<span style='font-size:64px'>ABCDE</span>", 100),
    ] {
        let plain = layout_er(&html_relationship_fixture(text, width));
        let expected = plain.edges[0].label.as_ref().unwrap();
        for with_icon in [format!("{text} fa:fa-user"), format!("fa:fa-user {text}")] {
            let source = html_relationship_fixture(&with_icon, width);
            let parsed = parse_er(&source);
            let artifact = prepare_er(&parsed);
            let layout = project_layout(&artifact);
            let label = layout.edges[0].label.as_ref().unwrap();
            assert!(label.width >= expected.width);
            assert!(label.height >= expected.height);
            assert!(label.height >= 96.0);
            assert_relationship_font_viewport(artifact, &layout, "font-size:64px");
        }
    }
}

#[test]
fn er_swimlane_small_child_fonts_after_breaks_follow_the_measured_upstream_rows() {
    // A live 64px ancestor supplies a 96px strut on both rows. Closing it after the
    // break leaves only the 14px operation-root strut on the second row.
    for (text, expected_height) in [
        (
            "<span style='font-size:64px'>A<br/><span style='font-size:14px'>B</span></span>",
            192.0,
        ),
        ("<span style='font-size:64px'>A<br/></span>B", 117.0),
    ] {
        let source = html_relationship_fixture(text, 1000);
        let parsed = parse_er(&source);
        let artifact = prepare_er(&parsed);
        let layout = project_layout(&artifact);
        assert_eq!(
            layout.edges[0].label.as_ref().unwrap().height,
            expected_height,
            "{text}"
        );
        assert_relationship_font_viewport(artifact, &layout, "font-size:64px");
    }
}
