use super::{
    Body, CrossAxis, Edge, End, FLATNESS, Label, LabelOwner, PointMarker, Scene, Shaft,
    assert_same_bounds, contour_gap, rect_face_normal, svg_box, svg_group_transform,
    svg_marker_scale, svg_number, validate,
};
use crate::environment::RenderEnvironment;
use crate::model::LayoutPoint;
use crate::usecase::UsecaseDiagramLayout;
use crate::{LayoutOptions, family};
use base64::Engine as _;
use kurbo::{BezPath, Ellipse, ParamCurveNearest, Point, Rect, Shape, Vec2};
use merman_core::{MermaidEngine, ParseOptions};
use merman_model::families::usecase::UsecaseNodeKind;
use merman_model::presentation::RenderSemanticModel;
use roxmltree::{Document, Node};
use std::collections::HashMap;

#[test]
fn usecase_shoal_keeps_actor_oval_contours_and_boundary_stitches() {
    for look in ["classic", "neo"] {
        let source = format!(
            "\
---
config:
  layout: shoal
  look: {look}
  htmlLabels: true
---
usecase-beta
direction LR
actor Buyer(\"Customer\")
systemBoundary Auth(Authentication)
  SignIn(\"Sign in\")
  Verify(\"Verify code\")
end
Buyer --> SignIn
SignIn ..> : include Verify
"
        );
        check_usecase_geometry(&source, look);
    }
}

fn check_usecase_geometry(source: &str, look: &str) {
    let parsed = MermaidEngine::new()
        .parse_diagram_for_render_model_sync(source, ParseOptions::strict())
        .unwrap()
        .unwrap()
        .into_presentation();
    let RenderSemanticModel::Usecase(model) = parsed.model() else {
        panic!("expected Usecase semantics");
    };
    let artifact = family::prepare(
        &parsed,
        &LayoutOptions::headless_svg_defaults(),
        RenderEnvironment::deterministic().begin_session().unwrap(),
    )
    .unwrap();
    let layout: UsecaseDiagramLayout =
        serde_json::from_value(artifact.layout_json().unwrap()["layout"]["UsecaseDiagram"].clone())
            .unwrap();
    let svg = artifact
        .render_svg(&Default::default(), &Default::default())
        .unwrap();
    let document = Document::parse(svg.svg()).unwrap();
    let painted_item = |id: &str, role: &str| {
        document
            .descendants()
            .find(|item| {
                item.attribute("data-et") == Some(role) && item.attribute("data-id") == Some(id)
            })
            .unwrap_or_else(|| panic!("missing Usecase {role} `{id}`"))
    };
    let mut scene = Scene::default();
    let mut bodies = HashMap::new();
    for node in &layout.nodes {
        let measured = Rect::from_center_size((node.x, node.y), (node.width, node.height));
        let owner = scene.bodies.len();
        let painted = painted_item(&node.id, if node.is_cluster { "cluster" } else { "node" });
        let body = if node.is_cluster {
            let painted_frame = painted
                .descendants()
                .find(|item| {
                    item.attribute("class").is_some_and(|classes| {
                        classes
                            .split_ascii_whitespace()
                            .any(|name| name == "boundary-body")
                    })
                })
                .unwrap();
            assert_same_bounds(svg_box(painted_frame), measured);
            Body::group(measured)
        } else {
            let semantic = model.nodes.iter().find(|item| item.id == node.id).unwrap();
            let mut body = Body::node(measured);
            match semantic.kind {
                UsecaseNodeKind::Actor => {
                    let glyph = painted
                        .descendants()
                        .find(|item| item.attribute("class") == Some("usecase-actor-stick"))
                        .unwrap();
                    body.contour = Some(
                        svg_group_transform(glyph)
                            * BezPath::from_svg(glyph.attribute("d").unwrap()).unwrap(),
                    );
                }
                UsecaseNodeKind::UseCaseEllipse => {
                    let glyph = painted
                        .descendants()
                        .find(|item| item.has_tag_name("ellipse"))
                        .unwrap();
                    let geometry = Ellipse::new(
                        (svg_number(glyph, "cx"), svg_number(glyph, "cy")),
                        (svg_number(glyph, "rx"), svg_number(glyph, "ry")),
                        0.0,
                    );
                    let contour = svg_group_transform(glyph) * geometry.to_path(FLATNESS);
                    assert_same_bounds(contour.bounding_box(), measured);
                    body.contour = Some(contour);
                }
                _ => panic!("unexpected Usecase contour {}", node.id),
            }
            body
        };
        scene.bodies.push(body);
        bodies.insert(node.id.as_str(), owner);
        for label in painted
            .descendants()
            .filter(|item| item.has_tag_name("foreignObject"))
        {
            scene.labels.push(Label {
                owner: LabelOwner::Body(owner),
                bounds: svg_box(label),
            });
        }
    }
    for boundary in &model.boundaries {
        let parent = bodies[boundary.id.as_str()];
        for member in &boundary.members {
            scene.bodies[bodies[member.as_str()]].parent = Some(parent);
        }
    }
    assert_eq!(scene.bodies.len(), 4);
    let mut stitches = 0;
    let mut markers = 0;
    for (occurrence, route) in layout.edges.iter().enumerate() {
        let path = painted_item(&route.id, "edge");
        let from = bodies[route.from.as_str()];
        let to = bodies[route.to.as_str()];
        let mut edge = Edge::new(from, to, path.attribute("d").unwrap(), CrossAxis::Y);
        edge.transform = svg_group_transform(path);
        let data = base64::engine::general_purpose::STANDARD
            .decode(path.attribute("data-points").unwrap())
            .unwrap();
        let vertices: Vec<LayoutPoint> = serde_json::from_slice(&data).unwrap();
        let [first, .., last] = route.points.as_slice() else {
            panic!("missing Usecase route terminals for {}", route.id);
        };
        for (endpoint, contact, end, reference) in [
            (&mut edge.start, first, End::Start, "marker-start"),
            (&mut edge.end, last, End::End, "marker-end"),
        ] {
            let bounds = scene.bodies[endpoint.body].bounds;
            let contact = Point::new(contact.x, contact.y);
            endpoint.outward_normal = Some(rect_face_normal(bounds, contact));
            if let Some(id) = path.attribute(reference) {
                let id = id.strip_prefix("url(#").unwrap().strip_suffix(')').unwrap();
                let marker = document
                    .descendants()
                    .find(|item| item.has_tag_name("marker") && item.attribute("id") == Some(id))
                    .unwrap();
                let (trim, opening) = usecase_marker(marker, end);
                endpoint.setback = trim;
                endpoint.marker = Some(opening);
                markers += 1;
            }
        }
        if scene.bodies[from].parent != scene.bodies[to].parent {
            let group = scene.bodies[to]
                .parent
                .expect("the oval is within its system");
            let stitch = vertices[1..vertices.len() - 1]
                .iter()
                .map(|point| edge.transform * Point::new(point.x, point.y))
                .find(|&point| contour_gap(&scene.bodies[group], point) <= FLATNESS)
                .expect("a route knot is shared at the painted system border");
            let shaft = Shaft::read(&edge).unwrap();
            assert!(shaft.segments.iter().any(|(segment, _)| {
                segment.nearest(stitch, FLATNESS).distance_sq <= FLATNESS * FLATNESS
            }));
            stitches += 1;
        }
        scene.edges.push(edge);
        if let Some(measured) = &route.label {
            let label = document
                .descendants()
                .find(|item| {
                    item.attribute("class") == Some("label")
                        && item.attribute("data-id") == Some(route.id.as_str())
                })
                .unwrap();
            let bounds = svg_box(
                label
                    .descendants()
                    .find(|item| item.has_tag_name("foreignObject"))
                    .unwrap(),
            );
            assert_same_bounds(
                bounds,
                Rect::from_center_size((measured.x, measured.y), (measured.width, measured.height)),
            );
            scene.labels.push(Label {
                owner: LabelOwner::Edge(occurrence),
                bounds,
            });
        }
    }
    assert_eq!(scene.edges.len(), 2);
    assert_eq!(markers, 2);
    assert_eq!(stitches, 1);
    let report = validate(&scene);
    assert_eq!(report.contacts_checked, 4);
    assert_eq!(report.normals_checked, 4);
    assert_eq!(report.markers_checked, markers);
    assert!(report.hard.is_empty(), "{look}: {:#?}", report.hard);
}

fn usecase_marker(marker: Node<'_, '_>, end: End) -> (f64, PointMarker) {
    assert_eq!(marker.attribute("markerUnits"), Some("userSpaceOnUse"));
    let glyph = marker
        .descendants()
        .find(|item| item.has_tag_name("path"))
        .unwrap();
    let mut outline = BezPath::from_svg(glyph.attribute("d").unwrap())
        .unwrap()
        .bounding_box();
    if marker.attribute("overflow") == Some("visible") {
        let stroke_width = glyph
            .attribute("style")
            .unwrap()
            .split(';')
            .filter_map(|declaration| declaration.trim().strip_prefix("stroke-width:"))
            .next_back()
            .unwrap()
            .trim()
            .parse::<f64>()
            .unwrap();
        outline = outline.inflate(stroke_width / 2.0, stroke_width / 2.0);
    }
    let scale = svg_marker_scale(marker);
    let reference = Vec2::new(svg_number(marker, "refX"), svg_number(marker, "refY"));
    let (trim, depth) = match end {
        End::Start => (reference.x - outline.x0, outline.x1 - reference.x),
        End::End => (outline.x1 - reference.x, reference.x - outline.x0),
    };
    (
        trim.max(0.0) * scale,
        PointMarker {
            depth: depth * scale,
            half_width: (outline.y1 - reference.y).max(reference.y - outline.y0) * scale,
        },
    )
}
