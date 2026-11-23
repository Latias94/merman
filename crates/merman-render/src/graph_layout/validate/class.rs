use super::{
    Body, CONTACT_TOLERANCE, CrossAxis, Edge, End, Endpoint, FLATNESS, Label, LabelOwner,
    PointMarker, Scene, Shaft, contour_gap, enters_rect, start_tangent, validate,
};
use crate::{
    LayoutOptions, RenderCapability,
    environment::RenderEnvironment,
    family,
    model::{ClassDiagramLayout, LayoutLabel, LayoutPoint},
    svg::SvgRenderOptions,
};
use base64::{Engine as _, engine::general_purpose::STANDARD};
use kurbo::{Affine, BezPath, ParamCurve, PathEl, PathSeg, Point, Rect, Shape, Vec2};
use merman_core::{MermaidEngine, ParseOptions};
use merman_model::{families::class::ClassDiagram, presentation::RenderSemanticModel};
use roxmltree::{Document, Node};
use std::collections::HashMap;

const DIAGRAM_ID: &str = "class-ddlt";
const CONFIG: &str = "\
---
config:
  layout: shoal
  look: classic
  htmlLabels: true
  class:
    hierarchicalNamespaces: true
---
";

const NAMESPACES: &str = r#"
  namespace Trading {
    namespace Bookkeeping {
      class Account {
        +post()
      }
      class Ledger
    }
  }
  namespace Publishing {
    class Journal
  }
  Account "1" *-- "many" Ledger : postings
  Ledger --|> Journal : records
"#;

const PARALLEL: &str = r#"
  class Client {
    +send()
  }
  class Server {
    +reply()
  }
  Client "1" o--|> "0..*" Server : requests
  Client -- Server : responses
"#;

#[test]
fn nested_namespaces_and_uml_relations() {
    check(NAMESPACES, "TB", 3, 3, 3);
}

#[test]
fn parallel_uml_relations_in_all_four_directions() {
    for direction in ["TB", "BT", "LR", "RL"] {
        check(PARALLEL, direction, 2, 0, 0);
    }
}

fn check(source: &str, direction: &str, nodes: usize, groups: usize, expected_stitches: usize) {
    let source = format!("{CONFIG}classDiagram\n  direction {direction}\n{source}");
    let parsed = MermaidEngine::new()
        .parse_diagram_for_render_model_sync(&source, ParseOptions::strict())
        .expect("parse Class fixture")
        .expect("Class diagram")
        .into_presentation();
    let RenderSemanticModel::Class(model) = parsed.model() else {
        panic!("expected Class semantics");
    };
    let session = RenderEnvironment::deterministic().begin_session().unwrap();
    let artifact = family::prepare(&parsed, &LayoutOptions::headless_svg_defaults(), session)
        .expect("prepare Class with Shoal");
    let geometry = artifact.layout_geometry_json().expect("Class geometry");
    let layout: ClassDiagramLayout =
        serde_json::from_value(geometry["layout"]["ClassDiagramV2"].clone()).unwrap();
    let rendered = artifact
        .render_svg(
            &SvgRenderOptions {
                diagram_id: Some(DIAGRAM_ID.into()),
                ..Default::default()
            },
            &Default::default(),
        )
        .expect("paint Class with Shoal");
    assert!(
        rendered
            .required_capabilities()
            .contains(&RenderCapability::LayoutShoal)
    );
    assert_eq!(
        (
            layout.nodes.len(),
            layout.clusters.len(),
            layout.edges.len()
        ),
        (nodes, groups, 2)
    );

    let document = Document::parse(rendered.svg()).unwrap();
    let svg = document.root_element();
    let root = only(svg.descendants().filter(|node| group(*node, "root")));
    let view_box: svgtypes::ViewBox = attr(svg, "viewBox").parse().unwrap();
    let frame = Affine::translate((-view_box.x, -view_box.y));
    assert_eq!(transform(root, frame), frame);

    let body_ids = layout
        .clusters
        .iter()
        .map(|cluster| cluster.id.as_str())
        .chain(layout.nodes.iter().map(|node| node.id.as_str()))
        .enumerate()
        .map(|(index, id)| (id, index))
        .collect();
    let mut scene = capture_bodies(model, &layout, root, &body_ids, frame);
    let stitches = capture_relations(
        model, &layout, root, &body_ids, frame, direction, &mut scene,
    );
    assert_eq!(stitches, expected_stitches);
    assert_eq!(
        scene.labels.len(),
        groups + 4,
        "{direction}: titles and relation labels"
    );
    assert_ne!(scene.edges[0].d, scene.edges[1].d, "{direction}");
    let report = validate(&scene);
    assert!(report.hard.is_empty(), "{direction}: {:#?}", report.hard);
    assert!(
        report.label_overlaps.is_empty() && report.label_node_overlaps.is_empty(),
        "{direction}: {report:#?}"
    );
    assert_eq!(
        (
            report.contacts_checked,
            report.normals_checked,
            report.markers_checked
        ),
        (4, 4, 2)
    );
}

fn capture_bodies<'a>(
    model: &ClassDiagram,
    layout: &ClassDiagramLayout,
    root: Node<'a, '_>,
    body_ids: &HashMap<&str, usize>,
    frame: Affine,
) -> Scene<'a> {
    let mut scene = Scene::default();
    for cluster in &layout.clusters {
        let semantic = model.namespace(&cluster.id).expect("namespace identity");
        let element = with_id(root, "id", &format!("{DIAGRAM_ID}-{}", cluster.id));
        assert!(group(element, "cluster"));
        let bounds = svg_rect(
            only(element.children().filter(|node| node.has_tag_name("rect"))),
            frame,
        );
        compare_bounds(
            bounds,
            layout_box(cluster.x, cluster.y, cluster.width, cluster.height, frame),
            &cluster.id,
        );
        let mut body = Body::group(bounds);
        body.parent = semantic.namespace.parent.as_deref().map(|id| body_ids[id]);
        scene.bodies.push(body);
        let title = only(
            element
                .children()
                .filter(|node| group(*node, "cluster-label")),
        );
        scene.labels.push(Label {
            owner: LabelOwner::Body(body_ids[cluster.id.as_str()]),
            bounds: check_label(
                html_label(title),
                &cluster.title,
                &cluster.title_label,
                frame,
            ),
        });
    }
    for node in &layout.nodes {
        let semantic = model.class(&node.id).expect("class identity");
        let element = with_id(root, "id", &format!("{DIAGRAM_ID}-{}", semantic.dom_id));
        assert!(group(element, "node"));
        let outer = only(
            element
                .children()
                .filter(|child| group(*child, "outer-path")),
        );
        let fill = only(outer.children().filter(|child| {
            child.has_tag_name("path") && child.attribute("stroke") == Some("none")
        }));
        let mut outline = BezPath::from_svg(attr(fill, "d")).expect("Class fill outline");
        if !matches!(outline.elements().last(), Some(PathEl::ClosePath)) {
            outline.close_path();
        }
        outline.apply_affine(transform(fill, frame));
        let bounds = outline.bounding_box();
        compare_bounds(
            bounds,
            layout_box(node.x, node.y, node.width, node.height, frame),
            &node.id,
        );
        let mut body = Body::node(bounds);
        body.contour = Some(outline);
        body.parent = semantic.parent.map(|id| body_ids[id]);
        scene.bodies.push(body);
    }
    for body in &scene.bodies {
        if let Some(parent) = body.parent {
            assert!(
                scene.bodies[parent]
                    .bounds
                    .inflate(-FLATNESS, -FLATNESS)
                    .contains_rect(body.bounds),
                "Class body {:?} outside namespace {:?}",
                body.bounds,
                scene.bodies[parent].bounds,
            );
        }
    }
    scene
}

fn capture_relations<'a>(
    model: &ClassDiagram,
    layout: &ClassDiagramLayout,
    root: Node<'a, '_>,
    body_ids: &HashMap<&str, usize>,
    frame: Affine,
    direction: &str,
    scene: &mut Scene<'a>,
) -> usize {
    let forward = match direction {
        "TB" => Vec2::new(0., 1.),
        "BT" => Vec2::new(0., -1.),
        "LR" => Vec2::new(1., 0.),
        "RL" => Vec2::new(-1., 0.),
        _ => unreachable!(),
    };
    let axis = if forward.x == 0. {
        CrossAxis::X
    } else {
        CrossAxis::Y
    };
    let shafts = only(root.children().filter(|node| group(*node, "edgePaths")));
    let labels = only(root.children().filter(|node| group(*node, "edgeLabels")));
    assert_eq!(
        shafts
            .children()
            .filter(|node| node.attribute("data-et") == Some("edge"))
            .count(),
        layout.edges.len(),
    );
    let mut terminals = root
        .descendants()
        .filter(|node| group(*node, "edgeTerminals"))
        .collect::<Vec<_>>();
    let mut stitches = 0;
    let relations = model.relations();
    assert_eq!(relations.len(), layout.edges.len());
    for (index, relation) in relations.enumerate() {
        let edge = layout
            .edges
            .iter()
            .find(|edge| edge.id == relation.id)
            .unwrap();
        assert_eq!(
            (edge.from.as_str(), edge.to.as_str()),
            (relation.id1, relation.id2)
        );
        let path_id = format!("id_{}_{}_{}", relation.id1, relation.id2, index + 1);
        let path = with_id(shafts, "data-id", &path_id);
        assert!(path.has_tag_name("path") && path.attribute("data-et") == Some("edge"));
        let (from, to) = (body_ids[edge.from.as_str()], body_ids[edge.to.as_str()]);
        assert!(
            (scene.bodies[to].bounds.center() - scene.bodies[from].bounds.center()).dot(forward)
                > 0.
        );
        let mut painted = Edge::new(from, to, attr(path, "d"), axis);
        painted.transform = transform(path, frame);
        let points: Vec<LayoutPoint> =
            serde_json::from_slice(&STANDARD.decode(attr(path, "data-points")).unwrap()).unwrap();
        let point = |p: &LayoutPoint| frame * Point::new(p.x, p.y);
        assert_eq!(points.len(), edge.points.len(), "{path_id}");
        assert!(points.len() >= 2, "{path_id}");
        for (actual, solved) in points.iter().zip(&edge.points) {
            let actual = painted.transform * Point::new(actual.x, actual.y);
            assert!(
                actual.distance(point(solved)) <= FLATNESS,
                "{path_id}: {actual:?} vs {solved:?}"
            );
        }
        let shaft = Shaft::read(&painted).unwrap_or_else(|error| panic!("{path_id}: {error}"));
        for (end, endpoint, kind, contact) in [
            (
                End::Start,
                &mut painted.start,
                relation.relation.type1,
                point(&edge.points[0]),
            ),
            (
                End::End,
                &mut painted.end,
                relation.relation.type2,
                point(edge.points.last().unwrap()),
            ),
        ] {
            let body = &scene.bodies[endpoint.body];
            endpoint.outward_normal = Some(port_normal(body.bounds, contact));
            read_marker(path, end, kind, endpoint);
            let (segment, _) = shaft.segment_from(end, 0);
            let tip = segment.start() - start_tangent(segment) * endpoint.setback;
            assert!(
                tip.distance(contact) <= CONTACT_TOLERANCE,
                "{path_id} {end:?}: painted tip {tip:?}, layout {contact:?}"
            );
        }
        let edge_stitches = check_stitches(scene, &painted, &shaft, &edge.points, frame, &path_id);
        if edge_stitches > 0 {
            assert!(
                shaft
                    .segments
                    .iter()
                    .any(|(segment, _)| matches!(segment, PathSeg::Cubic(_))),
                "{path_id}: cross-namespace fixture should exercise curved SVG",
            );
        }
        stitches += edge_stitches;
        scene.edges.push(painted);
        let label = with_id(labels, "data-id", &path_id);
        assert!(group(label, "label"));
        scene.labels.push(Label {
            owner: LabelOwner::Edge(index),
            bounds: check_label(
                html_label(label),
                &relation.title,
                edge.label.as_ref().expect("solved relation label"),
                frame,
            ),
        });
        for (text, solved) in [
            (
                relation.relation_title_1.as_deref(),
                edge.start_label_right.as_ref(),
            ),
            (
                relation.relation_title_2.as_deref(),
                edge.end_label_left.as_ref(),
            ),
        ] {
            let text = text.filter(|text| !text.is_empty() && *text != "none");
            assert_eq!(text.is_some(), solved.is_some(), "{path_id}");
            if let Some((text, solved)) = text.zip(solved) {
                let position = terminals
                    .iter()
                    .position(|group| label_text(*group) == text)
                    .unwrap_or_else(|| panic!("{path_id} missing cardinality {text}"));
                let html = html_label(terminals.remove(position));
                let mut bounds = check_label(html, text, solved, frame);
                let visible_width = css_width(html);
                assert!(
                    (visible_width - solved.width).abs() <= FLATNESS,
                    "{path_id}: cardinality {text} width {visible_width} differs from {}",
                    solved.width,
                );
                bounds.x1 = bounds.x0 + visible_width;
                scene.labels.push(Label {
                    owner: LabelOwner::Edge(index),
                    bounds,
                });
            }
        }
    }
    assert!(terminals.is_empty());
    stitches
}

fn attr<'a>(node: Node<'a, '_>, name: &str) -> &'a str {
    node.attribute(name)
        .unwrap_or_else(|| panic!("missing {name} on {:?}", node.tag_name()))
}

fn only<'a, 'input>(mut nodes: impl Iterator<Item = Node<'a, 'input>>) -> Node<'a, 'input> {
    let node = nodes.next().expect("missing painted Class element");
    assert!(nodes.next().is_none(), "multiple matches for {node:?}");
    node
}

fn with_id<'a, 'input>(root: Node<'a, 'input>, key: &str, id: &str) -> Node<'a, 'input> {
    only(
        root.descendants()
            .filter(|node| node.attribute(key) == Some(id)),
    )
}

fn has_class(node: Node<'_, '_>, name: &str) -> bool {
    node.attribute("class")
        .is_some_and(|classes| classes.split_ascii_whitespace().any(|class| class == name))
}

fn group(node: Node<'_, '_>, name: &str) -> bool {
    node.has_tag_name("g") && has_class(node, name)
}

fn transform(node: Node<'_, '_>, frame: Affine) -> Affine {
    frame * super::svg_group_transform(node)
}

fn number(node: Node<'_, '_>, name: &str) -> f64 {
    attr(node, name).parse().unwrap()
}

fn svg_rect(node: Node<'_, '_>, frame: Affine) -> Rect {
    let origin = |name| {
        node.attribute(name)
            .map_or(0., |value| value.parse::<f64>().unwrap())
    };
    transform(node, frame).transform_rect_bbox(Rect::from_origin_size(
        (origin("x"), origin("y")),
        (number(node, "width"), number(node, "height")),
    ))
}

fn label_text(node: Node<'_, '_>) -> String {
    node.descendants()
        .filter(|child| child.is_text())
        .filter_map(|child| child.text())
        .collect()
}

fn html_label<'a, 'input>(owner: Node<'a, 'input>) -> Node<'a, 'input> {
    only(
        owner
            .descendants()
            .filter(|node| node.has_tag_name("foreignObject")),
    )
}

fn check_label(element: Node<'_, '_>, text: &str, layout: &LayoutLabel, frame: Affine) -> Rect {
    assert_eq!(label_text(element), text);
    let actual = svg_rect(element, frame);
    compare_bounds(
        actual,
        layout_box(layout.x, layout.y, layout.width, layout.height, frame),
        text,
    );
    actual
}

fn css_width(html: Node<'_, '_>) -> f64 {
    attr(html, "style")
        .split(';')
        .filter_map(|declaration| declaration.trim().split_once(':'))
        .find_map(|(key, value)| (key == "width").then_some(value))
        .and_then(|value| value.trim().strip_suffix("px"))
        .expect("Class terminal CSS width in SVG pixels")
        .parse()
        .unwrap()
}

fn compare_bounds(actual: Rect, expected: Rect, id: &str) {
    assert!(
        actual.is_finite()
            && expected.is_finite()
            && (actual.origin() - expected.origin()).hypot() <= FLATNESS
            && (actual.max_x() - expected.max_x()).abs() <= FLATNESS
            && (actual.max_y() - expected.max_y()).abs() <= FLATNESS,
        "{id}: painted {actual:?}, layout {expected:?}",
    );
}

fn layout_box(x: f64, y: f64, width: f64, height: f64, frame: Affine) -> Rect {
    frame.transform_rect_bbox(Rect::from_center_size((x, y), (width, height)))
}

fn port_normal(bounds: Rect, contact: Point) -> Vec2 {
    let (distance, normal) = [
        (contact.x - bounds.x0, Vec2::new(-1., 0.)),
        (contact.x - bounds.x1, Vec2::new(1., 0.)),
        (contact.y - bounds.y0, Vec2::new(0., -1.)),
        (contact.y - bounds.y1, Vec2::new(0., 1.)),
    ]
    .into_iter()
    .min_by(|(a, _), (b, _)| a.abs().total_cmp(&b.abs()))
    .unwrap();
    assert!(
        distance.abs() <= CONTACT_TOLERANCE,
        "{contact:?} off {bounds:?}"
    );
    normal
}

fn read_marker(path: Node<'_, '_>, end: End, kind: i32, endpoint: &mut Endpoint) {
    let attribute = match end {
        End::Start => "marker-start",
        End::End => "marker-end",
    };
    let (kind, vertices) = match kind {
        0 => ("aggregation", 4),
        1 => ("extension", 3),
        2 => ("composition", 4),
        -1 => {
            assert!(path.attribute(attribute).is_none());
            return;
        }
        other => panic!("unexpected UML marker {other}"),
    };
    let id = attr(path, attribute)
        .strip_prefix("url(#")
        .and_then(|reference| reference.strip_suffix(')'))
        .expect("local UML marker reference");
    let marker = with_id(path.document().root_element(), "id", id);
    assert!(marker.has_tag_name("marker") && has_class(marker, kind));
    assert_eq!(attr(marker, "markerUnits"), "userSpaceOnUse");
    assert_eq!(attr(marker, "orient"), "auto");
    assert!(marker.attribute("viewBox").is_none());
    let glyph = only(marker.children().filter(Node::is_element));
    assert!(glyph.has_tag_name("path") && glyph.attribute("transform").is_none());
    let outline = BezPath::from_svg(attr(glyph, "d")).unwrap();
    let bounds = outline.bounding_box();
    let reference = Point::new(number(marker, "refX"), number(marker, "refY"));
    let (tip_x, base_x) = match end {
        End::Start => (bounds.x0, bounds.x1),
        End::End => (bounds.x1, bounds.x0),
    };
    let segments: Vec<_> = outline.segments().collect();
    assert_eq!(segments.len(), vertices, "{id}");
    assert!(
        segments
            .iter()
            .all(|segment| matches!(segment, PathSeg::Line(_)))
    );
    let tips: Vec<_> = segments
        .iter()
        .map(|segment| segment.start())
        .filter(|vertex| (vertex.x - tip_x).abs() <= f64::EPSILON)
        .collect();
    assert_eq!(
        tips,
        [Point::new(tip_x, reference.y)],
        "{id}: point toward its class"
    );
    assert!(
        (reference.x - base_x).abs() <= f64::EPSILON,
        "{id}: reference behind tip"
    );
    assert!(bounds.x0 >= 0. && bounds.x1 <= number(marker, "markerWidth"));
    assert!(bounds.y0 >= 0. && bounds.y1 <= number(marker, "markerHeight"));
    endpoint.setback = (reference.x - tip_x).abs();
    endpoint.marker = Some(PointMarker {
        depth: endpoint.setback,
        half_width: (reference.y - bounds.y0).min(bounds.y1 - reference.y),
    });
}

fn check_stitches(
    scene: &Scene<'_>,
    edge: &Edge<'_>,
    shaft: &Shaft,
    solved: &[LayoutPoint],
    frame: Affine,
    id: &str,
) -> usize {
    let ancestors = |body: usize| {
        std::iter::successors(scene.bodies[body].parent, |&parent| {
            scene.bodies[parent].parent
        })
        .collect::<Vec<_>>()
    };
    let (from, to) = (ancestors(edge.start.body), ancestors(edge.end.body));
    let crossings = from
        .iter()
        .filter(|group| !to.contains(group))
        .map(|&group| (group, true))
        .chain(
            to.iter()
                .rev()
                .filter(|group| !from.contains(group))
                .map(|&group| (group, false)),
        );
    let mut count = 0;
    let mut remaining = 1;
    for (group, exiting) in crossings {
        let body = &scene.bodies[group];
        let knot = solved[remaining..solved.len() - 1]
            .iter()
            .position(|point| contour_gap(body, frame * Point::new(point.x, point.y)) < FLATNESS)
            .unwrap_or_else(|| panic!("{id}: missing layout stitch on {:?}", body.bounds));
        remaining += knot;
        let contact = frame * Point::new(solved[remaining].x, solved[remaining].y);
        remaining += 1;
        assert!(
            shaft.lines.windows(2).any(|pair| {
                pair[0].p1.distance(contact) <= FLATNESS
                    && pair[1].p0.distance(contact) <= FLATNESS
                    && enters_rect(pair[0], body.bounds) == exiting
                    && enters_rect(pair[1], body.bounds) != exiting
            }),
            "{id}: missing painted stitch on {:?} at {contact:?}",
            body.bounds
        );
        count += 1;
    }
    count
}
