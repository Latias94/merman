use super::{
    Body, CrossAxis, Edge, FLATNESS, Label, LabelOwner, PointMarker, Scene, svg_group_transform,
    validate,
};
use crate::environment::RenderEnvironment;
use crate::model::{LayoutPoint, StateDiagramLayout};
use crate::{LayoutOptions, family};
use kurbo::{BezPath, Circle, Point, Rect, Shape, Vec2};
use merman_core::{MermaidEngine, ParseOptions};
use merman_model::presentation::RenderSemanticModel;
use roxmltree::Document;
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
    Ready --> Running : begin
    Running --> [*]
  }}
  Processing --> Completed : done
"
        );
        check_state_geometry(&source, look);
    }
}

fn check_state_geometry(source: &str, look: &str) {
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
    let mut scene = Scene::default();
    let mut bodies = HashMap::new();
    for cluster in &layout.clusters {
        let owner = scene.bodies.len();
        scene.bodies.push(Body::group(Rect::from_center_size(
            (cluster.x, cluster.y),
            (cluster.width, cluster.height),
        )));
        bodies.insert(cluster.id.as_str(), owner);
        if cluster.title_label.width > 0.0 && cluster.title_label.height > 0.0 {
            let label = &cluster.title_label;
            let label_group = doc
                .descendants()
                .find(|node| {
                    node.attribute("data-et") == Some("cluster")
                        && node.attribute("data-id") == Some(cluster.id.as_str())
                })
                .unwrap()
                .descendants()
                .find(|node| node.attribute("class") == Some("cluster-label"))
                .unwrap();
            let center = svg_group_transform(label_group)
                * Point::new(label.width * 0.5, label.height * 0.5);
            assert!(center.distance(Point::new(label.x, label.y)) <= FLATNESS);
            scene.labels.push(Label {
                owner: LabelOwner::Body(owner),
                bounds: Rect::from_center_size(center, (label.width, label.height)),
            });
        }
    }
    let mut circle_count = 0;
    for node in layout.nodes.iter().filter(|node| !node.is_cluster) {
        let owner = scene.bodies.len();
        let mut body = Body::node(Rect::from_center_size(
            (node.x, node.y),
            (node.width, node.height),
        ));
        let semantic = model
            .nodes
            .iter()
            .find(|source| source.id == node.id)
            .unwrap();
        if matches!(semantic.shape.as_str(), "stateStart" | "stateEnd") {
            let painted = doc
                .descendants()
                .find(|element| {
                    element.attribute("data-et") == Some("node")
                        && element.attribute("data-id") == Some(node.id.as_str())
                })
                .unwrap();
            body.contour = Some(terminal_contour(painted));
            let center = body.contour.as_ref().unwrap().bounding_box().center();
            assert!(center.distance(Point::new(node.x, node.y)) <= FLATNESS);
            circle_count += 1;
        }
        scene.bodies.push(body);
        bodies.insert(node.id.as_str(), owner);
    }
    assert_eq!(circle_count, 3);
    for node in &model.nodes {
        if let Some(&owner) = bodies.get(node.id.as_str()) {
            scene.bodies[owner].parent = node
                .parent_id
                .as_deref()
                .and_then(|id| bodies.get(id))
                .copied();
        }
    }

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
        edge.start.outward_normal = Some(face_normal(scene.bodies[from].bounds, &route.points[0]));
        edge.end.outward_normal = Some(face_normal(
            scene.bodies[to].bounds,
            route.points.last().unwrap(),
        ));
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
            let width: f64 = background.attribute("width").unwrap().parse().unwrap();
            let height: f64 = background.attribute("height").unwrap().parse().unwrap();
            let center = svg_group_transform(background) * Point::new(width / 2.0, height / 2.0);
            assert!(center.distance(Point::new(label.x, label.y)) <= FLATNESS);
            assert!((width - label.width).abs() <= FLATNESS);
            assert!((height - label.height).abs() <= FLATNESS);
            scene.labels.push(Label {
                owner: LabelOwner::Edge(occurrence),
                bounds: Rect::from_center_size(center, (width, height)),
            });
        }
    }
    let report = validate(&scene);
    assert_eq!(report.contacts_checked, 2 * layout.edges.len());
    assert_eq!(report.normals_checked, 2 * layout.edges.len());
    assert_eq!(report.markers_checked, layout.edges.len());
    assert!(report.hard.is_empty(), "{look}: {:#?}", report.hard);
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

fn face_normal(bounds: Rect, port: &LayoutPoint) -> Vec2 {
    let port = Point::new(port.x, port.y);
    [
        ((port.x - bounds.x0).abs(), Vec2::new(-1.0, 0.0)),
        ((port.x - bounds.x1).abs(), Vec2::new(1.0, 0.0)),
        ((port.y - bounds.y0).abs(), Vec2::new(0.0, -1.0)),
        ((port.y - bounds.y1).abs(), Vec2::new(0.0, 1.0)),
    ]
    .into_iter()
    .min_by(|(distance, _), (other, _)| distance.total_cmp(other))
    .unwrap()
    .1
}
