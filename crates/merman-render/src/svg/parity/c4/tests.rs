use crate::environment::RenderEnvironment;
use crate::model::{C4DiagramLayout, LayoutPoint};
use crate::{LayoutAdaptation, LayoutOptions, family};
use base64::Engine as _;
use kurbo::{BezPath, ParamCurve, ParamCurveNearest, Point};
use merman_core::{MermaidEngine, ParseOptions};
use merman_model::OperationControl;
use merman_model::families::c4::C4Text;
use merman_model::presentation::{PresentationDiagram, RenderSemanticModel};
use roxmltree::{Document, Node};

fn render(source: &str, options: &LayoutOptions) -> (C4DiagramLayout, family::RenderedFamilySvg) {
    let parsed = MermaidEngine::new()
        .parse_diagram_for_render_model_sync(source, ParseOptions::strict())
        .unwrap()
        .unwrap()
        .into_presentation();
    render_model(&parsed, options)
}

fn render_model(
    parsed: &PresentationDiagram,
    options: &LayoutOptions,
) -> (C4DiagramLayout, family::RenderedFamilySvg) {
    let session = RenderEnvironment::deterministic().begin_session().unwrap();
    let artifact = family::prepare(parsed, options, session).unwrap();
    let layout =
        serde_json::from_value(artifact.layout_json().unwrap()["layout"]["C4Diagram"].clone())
            .unwrap();
    let svg = artifact
        .render_svg(&Default::default(), &Default::default())
        .unwrap();
    (layout, svg)
}

fn element<'a, 'input>(doc: &'a Document<'input>, kind: &str, id: &str) -> Node<'a, 'input> {
    doc.descendants()
        .find(|node| {
            node.attribute("data-et") == Some(kind) && node.attribute("data-id") == Some(id)
        })
        .unwrap()
}

fn points(edge: Node<'_, '_>) -> Vec<Point> {
    let bytes = base64::engine::general_purpose::STANDARD
        .decode(edge.attribute("data-points").unwrap())
        .unwrap();
    serde_json::from_slice::<Vec<LayoutPoint>>(&bytes)
        .unwrap()
        .into_iter()
        .map(|p| Point::new(p.x, p.y))
        .collect()
}

fn path(edge: Node<'_, '_>) -> BezPath {
    BezPath::from_svg(edge.attribute("d").unwrap()).unwrap()
}

fn number(node: Node<'_, '_>, attr: &str) -> f64 {
    node.attribute(attr).unwrap().parse().unwrap()
}

fn check_terminal(edge: Node<'_, '_>, contact: Point, start: bool, marked: bool) {
    let name = if start { "marker-start" } else { "marker-end" };
    let marker = edge.attribute(name);
    assert_eq!(marker.is_some(), marked);
    let path = path(edge);
    let segment = if start {
        path.segments().next()
    } else {
        path.segments().last()
    }
    .unwrap();
    let direction = (segment.end() - segment.start()).normalize();
    let (end, normal, marker_tip_x) = if start {
        (segment.start(), -direction, 0.0)
    } else {
        (segment.end(), direction, 10.0)
    };
    assert!(direction.x.abs() < 1e-8 || direction.y.abs() < 1e-8);
    let tip = if let Some(marker) = marker {
        let id = marker
            .strip_prefix("url(#")
            .unwrap()
            .strip_suffix(')')
            .unwrap();
        let marker = edge
            .document()
            .descendants()
            .find(|node| node.attribute("id") == Some(id))
            .unwrap();
        assert_eq!(marker.attribute("markerUnits"), Some("userSpaceOnUse"));
        end + normal * (number(marker, "refX") - marker_tip_x).abs()
    } else {
        end
    };
    assert!(
        tip.distance(contact) < 1e-8,
        "{name} tip {tip:?} differs from contact {contact:?}"
    );
}

#[test]
fn c4_shoal_preserves_three_nested_stitches_and_relationship_ink() {
    for look in ["classic", "neo"] {
        let source = format!(
            r#"---
config:
  layout: shoal
  look: {look}
---
C4Container
Person(user, "Customer")
Enterprise_Boundary(ent, "Enterprise") {{
  System_Boundary(sys, "Payments") {{
    Container_Boundary(cb, "Storage") {{
      ContainerDb(db, "Ledger", "PostgreSQL")
    }}
  }}
}}
BiRel(user, db, "Reads<br/>and writes", "TLS")
Rel_Back(db, sys, "Belongs to")
UpdateRelStyle(user, db, $textColor="navy", $lineColor="blue", $offsetX="12", $offsetY="-8")
"#
        );
        let options = LayoutOptions::headless_svg_defaults()
            .with_container_size(320.0, 600.0)
            .with_adaptation(LayoutAdaptation::FitWidth)
            .with_shoal_layout_frame(None);
        let (layout, svg) = render(&source, &options);
        let frame = serde_json::to_value(svg.shoal_layout_frame().unwrap().as_ref()).unwrap();
        assert_eq!(frame["groups"], serde_json::json!(["ent", "sys", "cb"]));
        assert_eq!(frame["nodes"]["db"]["parent"], 2);
        let doc = Document::parse(svg.svg()).unwrap();
        let forward = element(&doc, "edge", "c4-rel-0");
        let backward = element(&doc, "edge", "c4-rel-1");
        assert_eq!(forward.attribute("stroke"), Some("blue"));
        let knots = points(forward);
        let forward_path = path(forward);
        let mut previous = 0;
        for alias in ["ent", "sys", "cb"] {
            let cluster = element(&doc, "cluster", alias);
            let frame = cluster
                .children()
                .find(|child| child.has_tag_name("rect"))
                .unwrap();
            let left = number(frame, "x");
            let top = number(frame, "y");
            let height = number(frame, "height");
            let (index, port) = knots
                .iter()
                .enumerate()
                .skip(previous + 1)
                .find(|(_, port)| {
                    (port.x - left).abs() < 1e-8 && port.y > top && port.y < top + height
                })
                .unwrap();
            previous = index;
            assert!(
                forward_path
                    .segments()
                    .any(|segment| segment.nearest(*port, 1e-6).distance_sq < 1e-12)
            );
        }
        assert!(
            !doc.descendants()
                .any(|node| node.attribute("data-et") == Some("cluster")
                    && node.attribute("data-id") == Some("global"))
        );
        check_terminal(forward, knots[0], true, true);
        check_terminal(forward, *knots.last().unwrap(), false, true);
        let reverse_knots = points(backward);
        check_terminal(backward, reverse_knots[0], true, true);
        check_terminal(backward, *reverse_knots.last().unwrap(), false, false);
        assert_eq!(layout.rels[0].from, "user");
        assert_eq!(layout.rels[1].to, "sys");
        let label = doc
            .descendants()
            .find(|node| {
                node.attribute("class") == Some("label")
                    && node.attribute("data-id") == Some("c4-rel-0")
            })
            .unwrap();
        let rows: Vec<_> = label
            .descendants()
            .filter(|node| node.has_tag_name("tspan"))
            .filter_map(|node| node.text())
            .collect();
        assert_eq!(rows, ["Reads", "and writes", "[TLS]"]);
    }
}

#[test]
fn c4_shoal_uses_the_invisible_root_only_for_a_relation_that_names_it() {
    let base = "---\nconfig:\n  layout: shoal\n---\nC4Context\nSystem(a, \"A\")\nRel(global, a, \"From root\")\n";
    for suffix in ["", "System(global, \"Visible alias\")\n"] {
        let (layout, svg) = render(
            &format!("{base}{suffix}"),
            &LayoutOptions::headless_svg_defaults(),
        );
        let doc = Document::parse(svg.svg()).unwrap();
        let edge = element(&doc, "edge", "c4-rel-0");
        let start = points(edge)[0];
        if suffix.is_empty() {
            let root = layout
                .boundaries
                .iter()
                .find(|boundary| boundary.alias == "global")
                .unwrap();
            assert_eq!(start.x, root.x);
            assert!(start.y > root.y && start.y < root.y + root.height);
        } else {
            assert_eq!(
                element(&doc, "node", "global").attribute("data-id"),
                Some("global")
            );
            let node = layout
                .shapes
                .iter()
                .find(|shape| shape.alias == "global")
                .unwrap();
            assert!((start.x - node.x - node.width).abs() < 1e-8);
        }
    }
}

#[test]
fn c4_shoal_keeps_typed_parallel_occurrences_and_dynamic_numbering() {
    let source = "---\nconfig:\n  layout: shoal\n---\nC4Dynamic\nSystem(a, \"A\")\nSystem(b, \"B\")\nRel(a, b, \"Request\")\n";
    let (metadata, mut model, context) = MermaidEngine::new()
        .parse_diagram_for_render_model_sync(source, ParseOptions::strict())
        .unwrap()
        .unwrap()
        .into_presentation()
        .into_parts();
    let RenderSemanticModel::C4(c4) = &mut model else {
        panic!("expected C4 model");
    };
    c4.rels[0].line_color = Some("navy".into());
    let mut parallel = c4.rels[0].clone();
    parallel.label = C4Text::String("Acknowledgment".into());
    parallel.rel_type = "rel_b".into();
    parallel.line_color = Some("red".into());
    c4.rels.push(parallel);
    let parsed = PresentationDiagram::try_from_frozen(
        metadata,
        model,
        context,
        &Default::default(),
        &OperationControl::new(),
    )
    .unwrap()
    .unwrap();
    let (layout, svg) = render_model(&parsed, &LayoutOptions::headless_svg_defaults());
    assert_eq!(layout.rels[0].label.text, "1: Request");
    assert_eq!(layout.rels[1].label.text, "2: Acknowledgment");
    let doc = Document::parse(svg.svg()).unwrap();
    let first = element(&doc, "edge", "c4-rel-0");
    let second = element(&doc, "edge", "c4-rel-1");
    assert_eq!(first.attribute("stroke"), Some("navy"));
    assert_eq!(second.attribute("stroke"), Some("red"));
    assert_ne!(points(first), points(second));
    assert!(first.attribute("marker-end").is_some());
    assert!(second.attribute("marker-start").is_some());
    assert!(second.attribute("marker-end").is_none());
}
