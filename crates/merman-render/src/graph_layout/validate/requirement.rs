use super::{
    Body, CrossAxis, Edge, Label, LabelOwner, PointMarker, Scene, assert_same_bounds,
    rect_face_normal, svg_box, svg_group_transform, svg_label_text, svg_marker_scale, svg_number,
    validate,
};
use crate::environment::RenderEnvironment;
use crate::model::RequirementDiagramLayout;
use crate::svg::SvgRenderOptions;
use crate::{LayoutOptions, family};
use kurbo::{BezPath, ParamCurve, Point, Rect, Shape};
use merman_core::{MermaidEngine, ParseOptions};
use merman_model::presentation::RenderSemanticModel;
use roxmltree::Document;
use std::collections::HashMap;

#[test]
fn requirement_shoal_preserves_relation_labels_and_both_marker_kinds() {
    for look in ["classic", "neo"] {
        let source = format!(
            "\
---
config:
  layout: shoal
  look: {look}
  htmlLabels: true
---
requirementDiagram
  direction LR
  requirement safety {{
    id: R1
    text: Stop safely
    risk: low
    verifymethod: test
  }}
  requirement rules {{
    id: R2
    text: Apply limits
    risk: low
    verifymethod: analysis
  }}
  element brake {{
    type: hardware
  }}
  brake - satisfies -> safety
  safety - contains -> rules
  safety - refines -> rules
  safety - refines -> safety
"
        );
        check_requirement_geometry(&source, look);
    }
}

fn check_requirement_geometry(source: &str, look: &str) {
    let parsed = MermaidEngine::new()
        .parse_diagram_for_render_model_sync(source, ParseOptions::strict())
        .unwrap()
        .unwrap()
        .into_presentation();
    let RenderSemanticModel::Requirement(model) = parsed.model() else {
        panic!("expected Requirement semantics");
    };
    let artifact = family::prepare(
        &parsed,
        &LayoutOptions::headless_svg_defaults(),
        RenderEnvironment::deterministic().begin_session().unwrap(),
    )
    .unwrap();
    let layout: RequirementDiagramLayout = serde_json::from_value(
        artifact.layout_json().unwrap()["layout"]["RequirementDiagram"].clone(),
    )
    .unwrap();
    let diagram_id = "shoal-requirement";
    let svg = artifact
        .render_svg(
            &SvgRenderOptions {
                diagram_id: Some(diagram_id.into()),
                ..Default::default()
            },
            &Default::default(),
        )
        .unwrap();
    let document = Document::parse(svg.svg()).unwrap();
    let mut scene = Scene::default();
    let mut bodies = HashMap::new();
    for node in &layout.nodes {
        let id = format!("{diagram_id}-{}", node.id);
        let painted = document
            .descendants()
            .find(|item| item.attribute("id") == Some(id.as_str()))
            .unwrap();
        assert_eq!(painted.attribute("data-et"), Some("node"));
        assert_eq!(painted.attribute("data-id"), Some(node.id.as_str()));
        let outline = painted
            .children()
            .find(|item| {
                item.attribute("class")
                    .is_some_and(|class| class.split_whitespace().any(|word| word == "outer-path"))
            })
            .unwrap()
            .children()
            .find(|item| item.has_tag_name("path"))
            .unwrap();
        let contour = svg_group_transform(outline)
            * BezPath::from_svg(outline.attribute("d").unwrap()).unwrap();
        let bounds = contour.bounding_box();
        assert_same_bounds(
            bounds,
            Rect::from_origin_size((node.x, node.y), (node.width, node.height)),
        );
        let owner = scene.bodies.len();
        bodies.insert(node.id.as_str(), owner);
        scene.bodies.push(Body::node(bounds));
        let mut title_found = false;
        for label in painted
            .descendants()
            .filter(|item| item.has_tag_name("foreignObject"))
        {
            let text = svg_label_text(label);
            if text.trim().is_empty() {
                continue;
            }
            title_found |= text.trim() == node.id;
            scene.labels.push(Label {
                owner: LabelOwner::Body(owner),
                bounds: svg_box(label),
            });
        }
        assert!(title_found, "missing visible Requirement title {}", node.id);
    }
    assert_eq!(scene.bodies.len(), 3);
    assert_eq!(layout.edges.len(), model.relationships.len());
    for (occurrence, route) in layout.edges.iter().enumerate() {
        let index = route
            .id
            .rsplit_once('-')
            .unwrap()
            .1
            .parse::<usize>()
            .unwrap();
        let relationship = &model.relationships[index];
        let path = document
            .descendants()
            .find(|item| {
                item.has_tag_name("path")
                    && item.attribute("data-et") == Some("edge")
                    && item.attribute("data-id") == Some(route.id.as_str())
            })
            .unwrap();
        let from = bodies[route.from.as_str()];
        let to = bodies[route.to.as_str()];
        let [first, .., last] = route.points.as_slice() else {
            panic!("missing Requirement terminals for {}", route.id);
        };
        let mut edge = Edge::new(from, to, path.attribute("d").unwrap(), CrossAxis::Y);
        edge.transform = svg_group_transform(path);
        edge.start.outward_normal = Some(rect_face_normal(
            scene.bodies[from].bounds,
            Point::new(first.x, first.y),
        ));
        edge.end.outward_normal = Some(rect_face_normal(
            scene.bodies[to].bounds,
            Point::new(last.x, last.y),
        ));
        let contains = relationship.rel_type == "contains";
        let (terminal, opposite, suffix, port) = if contains {
            (
                "marker-start",
                "marker-end",
                "containsStart",
                &mut edge.start,
            )
        } else {
            ("marker-end", "marker-start", "arrowEnd", &mut edge.end)
        };
        let marker_id = format!("{diagram_id}_requirement-requirement_{suffix}");
        assert_eq!(
            path.attribute(terminal),
            Some(format!("url(#{marker_id})").as_str())
        );
        assert!(path.attribute(opposite).is_none());
        let marker = document
            .descendants()
            .find(|item| {
                item.has_tag_name("marker") && item.attribute("id") == Some(marker_id.as_str())
            })
            .unwrap();
        assert_eq!(marker.attribute("markerUnits"), Some("userSpaceOnUse"));
        port.marker = Some(if contains {
            let circle = marker
                .descendants()
                .find(|item| item.has_tag_name("circle"))
                .unwrap();
            let radius = svg_number(circle, "r");
            PointMarker {
                depth: svg_number(circle, "cx") + radius - svg_number(marker, "refX"),
                half_width: radius,
            }
        } else {
            let glyph = marker
                .descendants()
                .find_map(|item| item.attribute("d"))
                .unwrap();
            let outline = BezPath::from_svg(glyph).unwrap();
            let bounds = outline.bounding_box();
            let scale = svg_marker_scale(marker);
            if marker.attribute("viewBox").is_some() {
                let projected_half_stroke = outline
                    .segments()
                    .map(|segment| {
                        let delta = segment.end() - segment.start();
                        delta.y.abs() / delta.hypot()
                    })
                    .fold(0.0_f64, f64::max)
                    * svg_number(marker, "stroke-width")
                    / 2.0;
                port.setback = (bounds.x1 + projected_half_stroke - svg_number(marker, "refX"))
                    .max(0.0)
                    * scale;
            }
            PointMarker {
                depth: (svg_number(marker, "refX") - bounds.x0) * scale,
                half_width: bounds.height() * scale / 2.0,
            }
        });
        scene.edges.push(edge);
        let measured = route.label.as_ref().unwrap();
        let label = document
            .descendants()
            .find(|item| {
                item.attribute("class") == Some("label")
                    && item.attribute("data-id") == Some(route.id.as_str())
            })
            .unwrap();
        let bounds = svg_box(
            label
                .children()
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
    let report = validate(&scene);
    assert_eq!(report.contacts_checked, 8);
    assert_eq!(report.normals_checked, 8);
    assert_eq!(report.markers_checked, 4);
    assert!(report.hard.is_empty(), "{look}: {:#?}", report.hard);
}
