use super::{
    Body, CrossAxis, Edge, End, FLATNESS, Label, LabelOwner, PointMarker, Scene, Shaft,
    assert_same_bounds, contour_gap, rect_face_normal, svg_box, svg_group_transform, svg_has_class,
    svg_label_text, svg_marker_scale, svg_number, validate,
};
use crate::environment::RenderEnvironment;
use crate::model::LayoutPoint;
use crate::usecase::UsecaseDiagramLayout;
use crate::{LayoutOptions, family};
use base64::Engine as _;
use kurbo::{
    BezPath, Ellipse, ParamCurve, ParamCurveNearest, Point, Rect, RoundedRect, Shape, Vec2,
};
use merman_core::{MermaidEngine, ParseOptions};
use merman_model::families::usecase::{
    UsecaseActorType, UsecaseArrowType, UsecaseNodeKind, UsecaseRelationshipType,
};
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
actor Customer(\"Customer\")
actor Reviewer(\"R\")@{{icon:\"fa:user\"}}
systemBoundary Portal(Order portal)@{{type:package}}
  Checkout(\"Submit order\")
  Verify(\"Verify identity\")
  Escalate(\"Escalate order\")
end
systemBoundary Admin(Administration)
  Review(\"Review order\")
  Archive(\"Archive order\")
end
Customer --> Checkout
Checkout ..> : include Verify
Escalate ..> : extend Checkout
Verify --> Review
Review --|> Archive
Reviewer --> Archive
"
        );
        check_usecase_geometry(&source, look, 4);
    }
}

#[test]
fn usecase_shoal_anchors_actor_links_and_loops_in_physical_axes() {
    for (direction, look) in [
        ("TB", "classic"),
        ("BT", "neo"),
        ("LR", "classic"),
        ("RL", "neo"),
    ] {
        for metadata in [
            "",
            "@{type:hollow}",
            "@{type:awesome}",
            r#"@{icon:"fa:user"}"#,
        ] {
            let source = format!(
                "\
---
config:
  layout: shoal
  look: {look}
  htmlLabels: true
  usecase:
    actorFontSize: 22
---
usecase-beta
direction {direction}
systemBoundary Portal(Partner portal)@{{type:package}}
  actor A(\"Partner account\\nwith a wide caption\"){metadata} <<External>>
end
B(\"Case\")
B --> A
A -- retry --> A
A <-- repeat -- A
"
            );
            check_usecase_geometry(&source, &format!("{direction} {look} {metadata}"), 1);
        }
    }
}

#[test]
fn usecase_shoal_hand_drawn_edges_advance_a_repeatable_random_stream() {
    let render = |seed| {
        let source = format!(
            "\
---
config:
  layout: shoal
  look: handDrawn
  handDrawnSeed: {seed}
---
usecase-beta
direction LR
A(\"Case\")
B(\"Case\")
C(\"Case\")
D(\"Case\")
A --> B
C --> D
"
        );
        let parsed = MermaidEngine::new()
            .parse_diagram_for_render_model_sync(&source, ParseOptions::strict())
            .unwrap()
            .unwrap()
            .into_presentation();
        let artifact = family::prepare(
            &parsed,
            &LayoutOptions::headless_svg_defaults(),
            RenderEnvironment::deterministic().begin_session().unwrap(),
        )
        .unwrap();
        artifact
            .render_svg(&Default::default(), &Default::default())
            .unwrap()
    };
    let paths = |svg: &str| {
        Document::parse(svg)
            .unwrap()
            .descendants()
            .filter(|item| item.attribute("data-et") == Some("edge"))
            .map(|edge| {
                svg_group_transform(edge) * BezPath::from_svg(edge.attribute("d").unwrap()).unwrap()
            })
            .collect::<Vec<_>>()
    };
    for seed in [0, 17] {
        let svg = render(seed);
        assert_eq!(svg.svg(), render(seed).svg());
        let paths = paths(svg.svg());
        let [first, second] = paths.as_slice() else {
            panic!("expected two painted Usecase paths");
        };
        let first_start = first.segments().next().unwrap().start();
        let second_start = second.segments().next().unwrap().start();
        let offset = second_start - first_start;
        assert_eq!(first.elements().len(), second.elements().len());
        assert!(
            first
                .segments()
                .zip(second.segments())
                .all(|(a, b)| (a.end() + offset).distance(b.end()) <= FLATNESS),
            "the canonical vertices should be translations of each other"
        );
        assert!(
            first
                .segments()
                .zip(second.segments())
                .any(|(a, b)| (a.eval(0.5) + offset).distance(b.eval(0.5)) > 0.001),
            "the second shaft replayed the first shaft's random offsets for seed {seed}"
        );
    }
    assert_ne!(paths(render(17).svg()), paths(render(23).svg()));
}

fn actor_glyph(painted: Node<'_, '_>, variant: UsecaseActorType) -> (BezPath, f64) {
    let glyph = painted
        .children()
        .find(|&item| svg_has_class(item, "usecase-actor-glyph"))
        .unwrap();
    let mut outline = BezPath::new();
    for part in glyph.children().filter(Node::is_element) {
        let path = match part.tag_name().name() {
            "path" if !svg_has_class(part, "usecase-actor-business-marker") => {
                BezPath::from_svg(part.attribute("d").unwrap()).unwrap()
            }
            "circle" => {
                let radius = svg_number(part, "r");
                Ellipse::new(
                    (svg_number(part, "cx"), svg_number(part, "cy")),
                    (radius, radius),
                    0.0,
                )
                .to_path(FLATNESS)
            }
            "rect" => RoundedRect::from_origin_size(
                (svg_number(part, "x"), svg_number(part, "y")),
                (svg_number(part, "width"), svg_number(part, "height")),
                svg_number(part, "rx"),
            )
            .to_path(FLATNESS),
            _ => continue,
        };
        outline.extend(
            (svg_group_transform(part) * path)
                .elements()
                .iter()
                .copied(),
        );
    }
    let anchor_y = match variant {
        UsecaseActorType::Normal | UsecaseActorType::Hollow => 5.0,
        UsecaseActorType::Awesome => 13.0,
        UsecaseActorType::Icon => -2.0,
    };
    (
        outline,
        (svg_group_transform(glyph) * Point::new(0.0, anchor_y)).y,
    )
}

fn check_usecase_geometry(source: &str, context: &str, expected_stitches: usize) {
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
    let mut actor_anchors = HashMap::new();
    for node in &layout.nodes {
        let measured = Rect::from_center_size((node.x, node.y), (node.width, node.height));
        let owner = scene.bodies.len();
        let painted = painted_item(&node.id, if node.is_cluster { "cluster" } else { "node" });
        let body = if node.is_cluster {
            let boundary = model
                .boundaries
                .iter()
                .find(|boundary| boundary.id == node.id)
                .unwrap();
            let painted_frame = painted
                .children()
                .find(|item| item.has_tag_name("rect") && svg_has_class(*item, "boundary-body"))
                .unwrap();
            let bounds = svg_box(painted_frame);
            assert_same_bounds(bounds, measured);
            let title = painted
                .children()
                .find(|item| svg_has_class(*item, "cluster-label"))
                .unwrap()
                .children()
                .find(|item| item.has_tag_name("foreignObject"))
                .unwrap();
            assert_eq!(svg_label_text(title), boundary.label);
            let title_bounds = svg_box(title);
            assert!(bounds.contains_rect(title_bounds));
            let tab = painted
                .children()
                .find(|item| svg_has_class(*item, "boundary-tab"));
            assert_eq!(tab.is_some(), boundary.package);
            if let Some(tab) = tab {
                assert!(svg_box(tab).contains_rect(title_bounds));
            }
            scene.labels.push(Label {
                owner: LabelOwner::Body(owner),
                bounds: title_bounds,
            });
            Body::group(bounds)
        } else {
            let semantic = model.nodes.iter().find(|item| item.id == node.id).unwrap();
            let mut body = Body::node(measured);
            let labels = painted
                .descendants()
                .filter(|item| item.has_tag_name("foreignObject"))
                .map(svg_box)
                .filter(|bounds| bounds.area() > 0.0);
            match semantic.kind {
                UsecaseNodeKind::Actor => {
                    let (mut outline, anchor_y) = actor_glyph(painted, semantic.actor_type);
                    let glyph_bounds = outline.bounding_box();
                    let mut previous_bottom = glyph_bounds.y1;
                    for label in labels.clone() {
                        assert!(
                            label.y0 >= previous_bottom,
                            "{context}: actor labels overlap"
                        );
                        previous_bottom = label.y1;
                        outline.extend(label.path_elements(FLATNESS));
                    }
                    assert_same_bounds(outline.bounding_box(), measured);
                    actor_anchors.insert(owner, anchor_y);
                    body.contour = Some(outline);
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
            for bounds in labels {
                scene.labels.push(Label {
                    owner: LabelOwner::Body(owner),
                    bounds,
                });
            }
            body
        };
        scene.bodies.push(body);
        bodies.insert(node.id.as_str(), owner);
    }
    for boundary in &model.boundaries {
        let parent = bodies[boundary.id.as_str()];
        for member in &boundary.members {
            scene.bodies[bodies[member.as_str()]].parent = Some(parent);
        }
    }
    assert_eq!(
        scene.bodies.len(),
        model.nodes.len() + model.boundaries.len()
    );
    let mut stitches = 0;
    let mut markers = 0;
    let mut generalizations = 0;
    for (occurrence, route) in layout.edges.iter().enumerate() {
        let path = painted_item(&route.id, "edge");
        let from = bodies[route.from.as_str()];
        let to = bodies[route.to.as_str()];
        let cross_axis = if matches!(model.direction.as_str(), "LR" | "RL") {
            CrossAxis::Y
        } else {
            CrossAxis::X
        };
        let mut edge = Edge::new(from, to, path.attribute("d").unwrap(), cross_axis);
        edge.transform = svg_group_transform(path);
        let shaft = Shaft::read(&edge).unwrap();
        let data = base64::engine::general_purpose::STANDARD
            .decode(path.attribute("data-points").unwrap())
            .unwrap();
        let vertices: Vec<LayoutPoint> = serde_json::from_slice(&data).unwrap();
        let [first, .., last] = route.points.as_slice() else {
            panic!("missing Usecase route terminals for {}", route.id);
        };
        for (endpoint, contact, painted, end, reference) in [
            (
                &mut edge.start,
                first,
                &vertices[0],
                End::Start,
                "marker-start",
            ),
            (
                &mut edge.end,
                last,
                vertices.last().unwrap(),
                End::End,
                "marker-end",
            ),
        ] {
            let bounds = scene.bodies[endpoint.body].bounds;
            let contact = Point::new(contact.x, contact.y);
            assert!(contour_gap(&Body::node(bounds), contact) <= FLATNESS);
            let normal = rect_face_normal(bounds, contact);
            endpoint.outward_normal = Some(normal);
            if let Some(&anchor_y) = actor_anchors.get(&endpoint.body) {
                let painted = edge.transform * Point::new(painted.x, painted.y);
                assert!((painted - contact).cross(normal).abs() <= FLATNESS);
                assert!(contour_gap(&scene.bodies[endpoint.body], painted) <= FLATNESS);
                if normal.x != 0.0 {
                    let center_y = if from == to {
                        first.y.midpoint(last.y)
                    } else {
                        contact.y
                    };
                    assert!(
                        (center_y - anchor_y).abs() <= FLATNESS,
                        "{context}: actor anchor"
                    );
                }
            }
            if let Some(id) = path.attribute(reference) {
                let id = id.strip_prefix("url(#").unwrap().strip_suffix(')').unwrap();
                let marker = document
                    .descendants()
                    .find(|item| item.has_tag_name("marker") && item.attribute("id") == Some(id))
                    .unwrap();
                let (trim, opening) = usecase_marker(marker, end);
                endpoint.setback = trim;
                endpoint.marker = Some(opening);
                if actor_anchors.contains_key(&endpoint.body) {
                    let lead = shaft.sample(end, opening.depth) - shaft.sample(end, 0.0);
                    assert!(
                        lead.dot(normal) + FLATNESS >= opening.depth,
                        "{context}: short actor marker lead on {}",
                        route.id
                    );
                }
                markers += 1;
                generalizations += usize::from(svg_has_class(marker, "extension"));
            }
        }
        if from == to && actor_anchors.contains_key(&from) {
            assert_ne!(
                (first.x, first.y),
                (last.x, last.y),
                "{context}: coincident loop terminals"
            );
        }
        let from_group = scene.bodies[from].parent;
        let to_group = scene.bodies[to].parent;
        if from_group != to_group {
            for group in [from_group, to_group].into_iter().flatten() {
                let stitch = vertices[1..vertices.len() - 1]
                    .iter()
                    .map(|point| edge.transform * Point::new(point.x, point.y))
                    .find(|&point| contour_gap(&scene.bodies[group], point) <= FLATNESS)
                    .unwrap_or_else(|| {
                        panic!(
                            "Usecase edge {} has no route knot at group {group}",
                            route.id
                        )
                    });
                assert!(
                    shaft.segments.iter().any(|(segment, _)| {
                        segment.nearest(stitch, FLATNESS).distance_sq <= FLATNESS * FLATNESS
                    }),
                    "Usecase edge {} misses its route knot at group {group}",
                    route.id
                );
                stitches += 1;
            }
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
    assert_eq!(scene.edges.len(), model.relationships.len());
    assert_eq!(
        markers,
        model
            .relationships
            .iter()
            .filter(|edge| edge.arrow_type != UsecaseArrowType::Line)
            .count()
    );
    assert_eq!(
        generalizations,
        model
            .relationships
            .iter()
            .filter(|edge| edge.relationship_type == UsecaseRelationshipType::Generalization)
            .count()
    );
    assert_eq!(
        scene
            .labels
            .iter()
            .filter(|label| matches!(label.owner, LabelOwner::Edge(_)))
            .count(),
        model
            .relationships
            .iter()
            .filter(|edge| edge.label.is_some())
            .count()
    );
    assert_eq!(stitches, expected_stitches);
    let report = validate(&scene);
    assert_eq!(report.contacts_checked, 2 * scene.edges.len());
    assert_eq!(report.normals_checked, 2 * scene.edges.len());
    assert_eq!(report.markers_checked, markers);
    assert!(report.hard.is_empty(), "{context}: {:#?}", report.hard);
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
