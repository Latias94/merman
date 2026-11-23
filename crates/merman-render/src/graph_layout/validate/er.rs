use super::{
    Body, CrossAxis, Edge, End, FLATNESS, Label, LabelOwner, PointMarker, Scene,
    assert_same_bounds, rect_face_normal, svg_box, svg_group_transform, svg_number, validate,
};
use crate::environment::RenderEnvironment;
use crate::model::{ErDiagramLayout, LayoutLabel};
use crate::{LayoutOptions, family};
use kurbo::{Affine, BezPath, Point, Rect, Shape};
use merman_core::{MermaidEngine, ParseOptions};
use merman_model::presentation::RenderSemanticModel;
use roxmltree::{Document, Node};
use std::collections::HashMap;

#[test]
fn er_shoal_preserves_table_routes_and_both_cardinality_terminals() {
    for look in ["classic", "neo"] {
        let source = format!(
            "\
---
config:
  layout: shoal
  look: {look}
  htmlLabels: true
---
erDiagram
  subgraph Sales [Sales ledger]
    direction LR
    CUSTOMER {{
      int id PK
      string name
    }}
    CUSTOMER ||--o{{ INVOICE : sends
    CUSTOMER }}o..o| INVOICE : audits
    INVOICE ||--|{{ ITEM : includes
  end
  CARRIER ||--o{{ Sales : serves
"
        );
        assert_eq!(check_er_geometry(&source, look), [true; 4]);
    }
}

#[test]
fn er_shoal_keeps_marker_clearance_at_a_mixed_group_with_a_self_relation() {
    for look in ["classic", "neo"] {
        let source = format!(
            "\
---
config:
  layout: shoal
  look: {look}
  htmlLabels: true
---
erDiagram
  direction BT
  subgraph Group [Group]
    direction LR
    A |{{--o| A : loop
  end
  B ||--o{{ A : connects
"
        );
        check_er_geometry(&source, look);
    }
}

fn check_er_geometry(source: &str, look: &str) -> [bool; 4] {
    let parsed = MermaidEngine::new()
        .parse_diagram_for_render_model_sync(source, ParseOptions::strict())
        .unwrap()
        .unwrap()
        .into_presentation();
    let RenderSemanticModel::Er(model) = parsed.model() else {
        panic!("expected ER semantics");
    };
    let artifact = family::prepare(
        &parsed,
        &LayoutOptions::headless_svg_defaults(),
        RenderEnvironment::deterministic().begin_session().unwrap(),
    )
    .unwrap();
    let layout: ErDiagramLayout =
        serde_json::from_value(artifact.layout_json().unwrap()["layout"]["ErDiagram"].clone())
            .unwrap();
    let svg = artifact
        .render_svg(&Default::default(), &Default::default())
        .unwrap();
    let document = Document::parse(svg.svg()).unwrap();
    let svg_id = document.root_element().attribute("id").unwrap();
    let element = |id: &str| {
        let id = format!("{svg_id}-{id}");
        document
            .descendants()
            .find(|node| node.attribute("id") == Some(id.as_str()))
            .unwrap_or_else(|| panic!("missing painted ER element `{id}`"))
    };
    let mut scene = Scene::default();
    let mut bodies = HashMap::new();
    let mut tables = 0;
    for node in &layout.nodes {
        let painted = element(&node.id);
        let bounds = if let Some(table) = painted
            .children()
            .find(|child| child.attribute("class") == Some("outer-path"))
        {
            let fill = table
                .children()
                .find(|child| child.has_tag_name("path"))
                .unwrap();
            let outline = BezPath::from_svg(fill.attribute("d").unwrap()).unwrap();
            tables += 1;
            (svg_group_transform(fill) * outline).bounding_box()
        } else {
            svg_box(
                painted
                    .children()
                    .find(|child| child.has_tag_name("rect"))
                    .unwrap(),
            )
        };
        assert_same_bounds(
            bounds,
            Rect::from_center_size((node.x, node.y), (node.width, node.height)),
        );
        let body = if node.is_cluster {
            Body::group(bounds)
        } else {
            Body::node(bounds)
        };
        bodies.insert(node.id.as_str(), scene.bodies.len());
        scene.bodies.push(body);
    }
    assert_eq!(
        tables,
        model
            .entities
            .values()
            .filter(|entity| !entity.attributes.is_empty())
            .count()
    );
    assert_eq!(layout.clusters.len(), model.subgraphs.len());
    for group in &model.subgraphs {
        let parent = bodies[group.id.as_str()];
        for member in &group.nodes {
            let id = model
                .entities
                .get(member)
                .map_or(member, |entity| &entity.id);
            scene.bodies[bodies[id.as_str()]].parent = Some(parent);
        }
        let bounds = svg_box(
            element(&group.id)
                .descendants()
                .find(|child| child.has_tag_name("foreignObject"))
                .unwrap(),
        );
        let cluster = layout
            .clusters
            .iter()
            .find(|cluster| cluster.id == group.id)
            .unwrap();
        assert_eq!(cluster.effective_dir, group.dir.as_deref().unwrap());
        assert_same_bounds(bounds, label_box(&cluster.title_label));
        scene.labels.push(Label {
            owner: LabelOwner::Body(parent),
            bounds,
        });
    }

    let mut cardinalities = [false; 4];
    let mut marker_boxes = Vec::new();
    for (occurrence, route) in layout.edges.iter().enumerate() {
        let index: usize = route.id.strip_prefix("er-rel-").unwrap().parse().unwrap();
        let relationship = &model.relationships[index];
        let id = format!(
            "id_{}_{}_{}",
            relationship.entity_a, relationship.entity_b, index
        );
        let path = element(&id);
        assert!(path.has_tag_name("path"));
        assert_eq!(path.attribute("data-look"), Some(look));
        let dashed = relationship.rel_spec.rel_type == "NON_IDENTIFYING";
        assert_eq!(
            path.attribute("class")
                .unwrap()
                .split_whitespace()
                .any(|name| name == "edge-pattern-dashed"),
            dashed
        );
        let from = bodies[route.from.as_str()];
        let to = bodies[route.to.as_str()];
        let [first, .., last] = route.points.as_slice() else {
            panic!("missing ER route terminals for {}", route.id);
        };
        let mut edge = Edge::new(from, to, path.attribute("d").unwrap(), CrossAxis::Y);
        edge.transform = svg_group_transform(path);
        for (end, endpoint, contact, card) in [
            (
                End::Start,
                &mut edge.start,
                first,
                &relationship.rel_spec.card_b,
            ),
            (End::End, &mut edge.end, last, &relationship.rel_spec.card_a),
        ] {
            let contact = Point::new(contact.x, contact.y);
            let normal = rect_face_normal(scene.bodies[endpoint.body].bounds, contact);
            endpoint.outward_normal = Some(normal);
            let (cardinality, suffix) = match card.as_str() {
                "ONLY_ONE" => (0, "onlyOne"),
                "ZERO_OR_ONE" => (1, "zeroOrOne"),
                "ONE_OR_MORE" => (2, "oneOrMore"),
                "ZERO_OR_MORE" => (3, "zeroOrMore"),
                _ => panic!("unexpected ER cardinality `{card}`"),
            };
            cardinalities[cardinality] = true;
            let (attribute, terminal) = match end {
                End::Start => ("marker-start", "Start"),
                End::End => ("marker-end", "End"),
            };
            let marker_id = format!("{svg_id}_er-{suffix}{terminal}");
            assert_eq!(
                path.attribute(attribute),
                Some(format!("url(#{marker_id})").as_str())
            );
            let marker = document
                .descendants()
                .find(|node| {
                    node.has_tag_name("marker") && node.attribute("id") == Some(marker_id.as_str())
                })
                .unwrap();
            let (trim, opening, ink) = marker_footprint(marker, end);
            endpoint.setback = trim;
            endpoint.marker = Some(opening);
            let front = if end == End::Start { normal } else { -normal };
            let anchor = contact + normal * trim;
            let transform = Affine::new([front.x, front.y, -front.y, front.x, anchor.x, anchor.y]);
            marker_boxes.push((occurrence, transform.transform_rect_bbox(ink)));
        }
        scene.edges.push(edge);
        let label = route.label.as_ref().unwrap();
        let terminal = document
            .descendants()
            .find(|node| {
                node.attribute("class") == Some("label")
                    && node.attribute("data-id") == Some(id.as_str())
            })
            .unwrap();
        let bounds = svg_box(
            terminal
                .children()
                .find(|node| node.has_tag_name("foreignObject"))
                .unwrap(),
        );
        assert_same_bounds(bounds, label_box(label));
        scene.labels.push(Label {
            owner: LabelOwner::Edge(occurrence),
            bounds,
        });
    }
    assert_eq!(scene.edges.len(), model.relationships.len());
    let report = validate(&scene);
    assert_eq!(report.contacts_checked, 2 * scene.edges.len());
    assert_eq!(report.normals_checked, 2 * scene.edges.len());
    assert_eq!(report.markers_checked, 2 * scene.edges.len());
    assert!(report.hard.is_empty(), "{look}: {:#?}", report.hard);
    for (owner, marker) in marker_boxes {
        for label in &scene.labels {
            if label.owner != LabelOwner::Edge(owner) {
                let hit = marker.intersect(label.bounds);
                assert!(
                    hit.width() <= FLATNESS || hit.height() <= FLATNESS,
                    "{look}: ER marker on edge {owner} hits {:?}",
                    label.owner
                );
            }
        }
    }
    cardinalities
}

fn label_box(label: &LayoutLabel) -> Rect {
    Rect::from_center_size((label.x, label.y), (label.width, label.height))
}

fn marker_footprint(marker: Node<'_, '_>, end: End) -> (f64, PointMarker, Rect) {
    assert_eq!(marker.attribute("markerUnits"), Some("userSpaceOnUse"));
    let ref_x = svg_number(marker, "refX");
    let ref_y = svg_number(marker, "refY");
    let bounds = marker
        .children()
        .filter_map(|child| {
            if let Some(path) = child.attribute("d") {
                Some(BezPath::from_svg(path).unwrap().bounding_box())
            } else if child.has_tag_name("circle") {
                let radius = svg_number(child, "r");
                Some(Rect::from_center_size(
                    (svg_number(child, "cx"), svg_number(child, "cy")),
                    (2.0 * radius, 2.0 * radius),
                ))
            } else {
                None
            }
        })
        .reduce(|combined, next| combined.union(next))
        .unwrap();
    let (trim, depth) = match end {
        End::Start => (ref_x - bounds.x0, bounds.x1 - ref_x),
        End::End => (bounds.x1 - ref_x, ref_x - bounds.x0),
    };
    (
        trim.max(0.0),
        PointMarker {
            depth,
            half_width: (bounds.y1 - ref_y).max(ref_y - bounds.y0),
        },
        bounds - Point::new(ref_x, ref_y).to_vec2(),
    )
}
