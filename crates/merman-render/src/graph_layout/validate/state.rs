use super::{
    BOX_EPSILON, Body, CONTACT_TOLERANCE, CrossAxis, Edge, FLATNESS, Label, LabelOwner,
    PointMarker, Scene, assert_same_bounds, contour_gap, rect_face_normal, svg_box,
    svg_group_transform, svg_has_class, svg_label_text, svg_number, validate,
};
use crate::environment::RenderEnvironment;
use crate::model::{LayoutLabel, LayoutPoint, StateDiagramLayout};
use crate::{LayoutOptions, family};
use base64::{Engine as _, engine::general_purpose::STANDARD};
use kurbo::{BezPath, Circle, Point, Rect, Shape};
use merman_core::{MermaidEngine, ParseOptions};
use merman_model::presentation::RenderSemanticModel;
use roxmltree::{Document, Node};
use std::collections::HashMap;

#[test]
fn state_mixed_direction_and_circle_ports_keep_geometric_contract() {
    for (look, html_labels) in [("classic", false), ("neo", true)] {
        let source = format!(
            "\
---
config:
  layout: shoal
  look: {look}
  htmlLabels: {html_labels}
---
stateDiagram-v2
  [*] --> Processing
  state Processing {{
    direction LR
    [*] --> Ready
    [*] --> Paused
    Ready --> Running : begin
    Running --> [*]
    Paused --> [*]
  }}
  Processing --> Completed : done
"
        );
        assert!(
            check_state_geometry(&source, look, 3, 0),
            "{look}: expected an off-center pseudo-state contact"
        );
    }
}

#[test]
fn state_concurrent_regions_keep_the_composite_title_clear() {
    for (look, direction) in [
        ("classic", "LR"),
        ("classic", "TB"),
        ("neo", "LR"),
        ("neo", "TB"),
    ] {
        let source = format!(
            "\
---
config:
  layout: shoal
  look: {look}
  htmlLabels: true
---
stateDiagram-v2
  [*] --> Processing
  state Processing {{
    direction {direction}
    [*] --> Ready
    Ready --> Running : begin
    --
    [*] --> Checking
    Checking --> Audited : record
    Audited --> [*]
  }}
  Processing --> Completed : done
"
        );
        check_state_geometry(&source, look, 4, 2);
    }
}

fn check_state_geometry(source: &str, look: &str, circles: usize, dividers: usize) -> bool {
    let parsed = MermaidEngine::new()
        .parse_diagram_for_render_model_sync(source, ParseOptions::strict())
        .unwrap()
        .unwrap()
        .into_presentation();
    let RenderSemanticModel::State(model) = parsed.model() else {
        panic!("expected State semantics");
    };
    let artifact = family::prepare(
        &parsed,
        &LayoutOptions::headless_svg_defaults(),
        RenderEnvironment::deterministic().begin_session().unwrap(),
    )
    .unwrap();
    let layout: StateDiagramLayout =
        serde_json::from_value(artifact.layout_json().unwrap()["layout"]["StateDiagramV2"].clone())
            .unwrap();
    let svg = artifact
        .render_svg(&Default::default(), &Default::default())
        .unwrap();
    let doc = Document::parse(svg.svg()).unwrap();
    let element = |id: &str, role: &str| {
        doc.descendants()
            .find(|node| {
                node.attribute("data-et") == Some(role) && node.attribute("data-id") == Some(id)
            })
            .unwrap_or_else(|| panic!("missing painted State {role} {id}"))
    };
    let mut scene = Scene::default();
    let mut bodies = HashMap::new();
    let mut divider_count = 0;
    let mut title_count = 0;
    for cluster in &layout.clusters {
        let semantic = model
            .nodes
            .iter()
            .find(|node| node.id == cluster.id)
            .unwrap();
        let divider = semantic.shape == "divider";
        let role = if divider { "divider" } else { "cluster" };
        let painted = element(&cluster.id, role);
        let frame_class = if divider { "divider" } else { "outer" };
        let frame = painted
            .children()
            .flat_map(|child| child.children())
            .find(|child| child.has_tag_name("rect") && svg_has_class(*child, frame_class))
            .unwrap();
        let bounds = svg_box(frame);
        assert_same_bounds(
            bounds,
            Rect::from_center_size((cluster.x, cluster.y), (cluster.width, cluster.height)),
        );
        let owner = scene.bodies.len();
        scene.bodies.push(Body::group(bounds));
        bodies.insert(cluster.id.as_str(), owner);
        let title = painted
            .children()
            .find(|child| svg_has_class(*child, "cluster-label"));
        if divider {
            assert!(title.is_none(), "a concurrent region has no visible title");
            divider_count += 1;
        } else {
            let title = title.unwrap();
            assert_eq!(svg_label_text(title), cluster.id);
            scene.labels.push(Label {
                owner: LabelOwner::Body(owner),
                bounds: state_title_box(title, &cluster.title_label),
            });
            title_count += 1;
        }
    }
    assert_eq!(divider_count, dividers);
    assert_eq!(title_count, 1);
    let mut circle_count = 0;
    for node in layout.nodes.iter().filter(|node| !node.is_cluster) {
        let owner = scene.bodies.len();
        let measured = Rect::from_center_size((node.x, node.y), (node.width, node.height));
        let semantic = model
            .nodes
            .iter()
            .find(|source| source.id == node.id)
            .unwrap();
        let painted = element(&node.id, "node");
        let body = if matches!(semantic.shape.as_str(), "stateStart" | "stateEnd") {
            let contour = terminal_contour(painted);
            let center = contour.bounding_box().center();
            assert!(center.distance(Point::new(node.x, node.y)) <= FLATNESS);
            circle_count += 1;
            Body {
                contour: Some(contour),
                ..Body::node(measured)
            }
        } else {
            let rect = painted
                .children()
                .find(|child| {
                    child.has_tag_name("rect") && svg_has_class(*child, "label-container")
                })
                .unwrap();
            let bounds = svg_box(rect);
            assert_same_bounds(bounds, measured);
            for label in painted
                .descendants()
                .filter(|child| child.has_tag_name("foreignObject"))
            {
                scene.labels.push(Label {
                    owner: LabelOwner::Body(owner),
                    bounds: svg_box(label),
                });
            }
            Body::node(bounds)
        };
        scene.bodies.push(body);
        bodies.insert(node.id.as_str(), owner);
    }
    assert_eq!(circle_count, circles);
    for node in &model.nodes {
        if let Some(&owner) = bodies.get(node.id.as_str()) {
            scene.bodies[owner].parent = node.parent_id.as_deref().map(|id| bodies[id]);
        }
    }

    let mut has_inset_contact = false;
    for (occurrence, route) in layout.edges.iter().enumerate() {
        let paths: Vec<_> = doc
            .descendants()
            .filter(|node| {
                node.has_tag_name("path")
                    && node.attribute("data-et") == Some("edge")
                    && node.attribute("data-id") == Some(route.id.as_str())
            })
            .collect();
        assert_eq!(paths.len(), 1, "State edge {}", route.id);
        let path = paths[0];
        let (from, to) = (bodies[route.from.as_str()], bodies[route.to.as_str()]);
        let mut edge = Edge::new(from, to, path.attribute("d").unwrap(), CrossAxis::X);
        edge.transform = svg_group_transform(path);
        let [start, .., end] = route.points.as_slice() else {
            panic!("missing State route endpoints for {}", route.id);
        };
        let painted_points: Vec<LayoutPoint> = serde_json::from_slice(
            &STANDARD
                .decode(path.attribute("data-points").unwrap())
                .unwrap(),
        )
        .unwrap();
        assert_eq!(painted_points.len(), route.points.len(), "{}", route.id);
        for (terminal, waypoint, contact) in [
            (&mut edge.start, start, painted_points.first().unwrap()),
            (&mut edge.end, end, painted_points.last().unwrap()),
        ] {
            let body = &scene.bodies[terminal.body];
            let waypoint = Point::new(waypoint.x, waypoint.y);
            assert!(
                contour_gap(&Body::node(body.bounds), waypoint) <= BOX_EPSILON,
                "State edge {}: layout waypoint {waypoint:?} is off its box {:?}",
                route.id,
                body.bounds
            );
            let normal = rect_face_normal(body.bounds, waypoint);
            terminal.outward_normal = Some(normal);
            if body.contour.is_some() {
                let contact = edge.transform * Point::new(contact.x, contact.y);
                assert!(
                    contour_gap(body, contact) <= CONTACT_TOLERANCE,
                    "State edge {}: data-points contact {contact:?} is off the painted contour",
                    route.id
                );
                let offset = waypoint - contact;
                assert!(
                    offset.cross(normal).abs() <= BOX_EPSILON && offset.dot(normal) >= -BOX_EPSILON,
                    "State edge {}: contact {contact:?} must stay on the waypoint's port axis",
                    route.id
                );
                has_inset_contact |= offset.dot(normal) > CONTACT_TOLERANCE;
            }
        }
        let marker_id = path
            .attribute("marker-end")
            .and_then(|value| value.strip_prefix("url(#"))
            .and_then(|value| value.strip_suffix(')'))
            .unwrap_or_else(|| panic!("State edge {} is missing its marker", route.id));
        let marker = doc
            .descendants()
            .find(|node| node.has_tag_name("marker") && node.attribute("id") == Some(marker_id))
            .unwrap();
        assert_eq!(marker.attribute("markerUnits"), Some("userSpaceOnUse"));
        let outline = marker
            .descendants()
            .find_map(|node| node.attribute("d"))
            .unwrap();
        let bounds = BezPath::from_svg(outline).unwrap().bounding_box();
        let reference_x: f64 = marker.attribute("refX").unwrap().parse().unwrap();
        let reference_y: f64 = marker.attribute("refY").unwrap().parse().unwrap();
        edge.end.setback = bounds.x1 - reference_x;
        assert_eq!(edge.end.setback, if look == "neo" { 2.0 } else { 0.0 });
        edge.end.marker = Some(PointMarker {
            depth: reference_x - bounds.x0,
            half_width: (bounds.y1 - reference_y).max(reference_y - bounds.y0),
        });
        scene.edges.push(edge);
        if let Some(label) = &route.label {
            let background = doc
                .descendants()
                .find(|node| {
                    node.has_tag_name("g")
                        && node.attribute("class") == Some("label")
                        && node.attribute("data-id") == Some(route.id.as_str())
                })
                .unwrap()
                .descendants()
                .find(|node| {
                    node.has_tag_name("foreignObject")
                        || node.attribute("class") == Some("background")
                })
                .unwrap();
            scene.labels.push(Label {
                owner: LabelOwner::Edge(occurrence),
                bounds: checked_label_box(svg_box(background), label),
            });
        }
    }
    let report = validate(&scene);
    assert_eq!(report.contacts_checked, 2 * layout.edges.len());
    assert_eq!(report.normals_checked, 2 * layout.edges.len());
    assert_eq!(report.markers_checked, layout.edges.len());
    assert!(report.hard.is_empty(), "{look}: {:#?}", report.hard);
    has_inset_contact
}

fn state_title_box(title: Node<'_, '_>, label: &LayoutLabel) -> Rect {
    let terminal = title.children().find(Node::is_element).unwrap();
    let bounds = if terminal.has_tag_name("foreignObject") {
        svg_box(terminal)
    } else {
        const NATIVE_FIXTURE_BASELINE: f64 = 14.0;
        assert!(terminal.has_tag_name("text"));
        let rows: Vec<_> = terminal.children().filter(Node::is_element).collect();
        let [row] = rows.as_slice() else {
            panic!("expected one native State title row");
        };
        assert!(row.has_tag_name("tspan"));
        svg_group_transform(*row).transform_rect_bbox(Rect::from_origin_size(
            (
                svg_number(*row, "x"),
                svg_number(*row, "y") - NATIVE_FIXTURE_BASELINE,
            ),
            (label.width, label.height),
        ))
    };
    checked_label_box(bounds, label)
}

fn checked_label_box(bounds: Rect, label: &LayoutLabel) -> Rect {
    assert_same_bounds(
        bounds,
        Rect::from_center_size((label.x, label.y), (label.width, label.height)),
    );
    bounds
}

fn terminal_contour(node: roxmltree::Node<'_, '_>) -> BezPath {
    if let Some(circle) = node.children().find(|child| child.has_tag_name("circle")) {
        let coordinate = |name| {
            circle
                .attribute(name)
                .unwrap_or("0")
                .parse::<f64>()
                .unwrap()
        };
        let local = Circle::new(
            (coordinate("cx"), coordinate("cy")),
            circle.attribute("r").unwrap().parse().unwrap(),
        );
        return svg_group_transform(circle) * local.to_path(FLATNESS);
    }
    let outer = node
        .children()
        .find(|child| child.attribute("class") == Some("outer-path"))
        .unwrap()
        .children()
        .find(|child| child.has_tag_name("path"))
        .unwrap();
    svg_group_transform(outer) * BezPath::from_svg(outer.attribute("d").unwrap()).unwrap()
}
