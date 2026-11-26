use crate::environment::{RenderEnvironment, RenderSession};
use crate::model::{C4DiagramLayout, LayoutPoint};
use crate::theme::{NativeTheme, ThemeCompiler};
use crate::{LayoutAdaptation, LayoutOptions, family};
use base64::Engine as _;
use kurbo::{BezPath, ParamCurve, ParamCurveNearest, Point, Rect, Shape, Vec2};
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
    let session = RenderEnvironment::deterministic().begin_session().unwrap();
    render_model(&parsed, options, session)
}

fn render_model(
    parsed: &PresentationDiagram,
    options: &LayoutOptions,
    session: RenderSession,
) -> (C4DiagramLayout, family::RenderedFamilySvg) {
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

fn rect_outline(rect: Rect, radius: f64) -> BezPath {
    rect.to_rounded_rect(radius).to_path(1e-8)
}

fn svg_rect_outline(node: Node<'_, '_>) -> BezPath {
    let x = number(node, "x");
    let y = number(node, "y");
    rect_outline(
        Rect::new(x, y, x + number(node, "width"), y + number(node, "height")),
        number(node, "rx"),
    )
}

fn touches_outline(outline: &BezPath, point: Point) -> bool {
    outline
        .segments()
        .any(|segment| segment.nearest(point, 1e-8).distance_sq < 1e-14)
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
        let stroke_outset = marker
            .first_element_child()
            .filter(|path| path.attribute("stroke-linejoin") == Some("round"))
            .map_or(0.0, |path| number(path, "stroke-width") / 2.0);
        end + normal * ((number(marker, "refX") - marker_tip_x).abs() + stroke_outset)
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
            let outline = svg_rect_outline(frame);
            let mut contacts = knots
                .iter()
                .enumerate()
                .filter(|(_, port)| touches_outline(&outline, **port));
            let (index, &port) = contacts.next().expect("one boundary stitch");
            assert!(contacts.next().is_none(), "duplicate stitch for {alias}");
            assert!(index > previous && index + 1 < knots.len());
            previous = index;
            assert!(touches_outline(&forward_path, port));
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
        assert!(
            !doc.descendants()
                .any(|node| node.attribute("data-et") == Some("cluster")
                    && node.attribute("data-id") == Some("global"))
        );
        if suffix.is_empty() {
            assert!(
                !doc.descendants()
                    .any(|node| node.attribute("data-et") == Some("node")
                        && node.attribute("data-id") == Some("global"))
            );
            let root = layout
                .boundaries
                .iter()
                .find(|boundary| boundary.alias == "global")
                .unwrap();
            let rect = Rect::new(root.x, root.y, root.x + root.width, root.y + root.height);
            assert!(touches_outline(&rect_outline(rect, 0.0), start));
        } else {
            let outline = element(&doc, "node", "global")
                .descendants()
                .find(|node| node.has_tag_name("rect"))
                .unwrap();
            let node = layout
                .shapes
                .iter()
                .find(|shape| shape.alias == "global")
                .unwrap();
            let translation = Vec2::new(node.x + node.width / 2.0, node.y + node.height / 2.0);
            assert!(touches_outline(
                &svg_rect_outline(outline),
                start - translation
            ));
        }
        check_terminal(edge, start, true, false);
    }
}

#[test]
fn c4_shoal_keeps_large_legacy_label_offsets_near_the_relationship() {
    let source = "---\nconfig:\n  layout: shoal\n---\nC4Context\nSystem(a, \"Service\")\nSystem(b, \"Store\")\nRel(a, b, \"Reads\", \"TLS\")\n";
    let options = LayoutOptions::headless_svg_defaults();
    let (natural, _) = render(source, &options);
    let (layout, svg) = render(
        &format!("{source}UpdateRelStyle(a, b, $offsetX=\"900\", $offsetY=\"-600\")\n"),
        &options,
    );
    assert!(layout.width < natural.width + 30.0);
    assert!(layout.height < natural.height + 30.0);
    let doc = Document::parse(svg.svg()).unwrap();
    let shaft = path(element(&doc, "edge", "c4-rel-0"));
    let label = doc
        .descendants()
        .find(|node| {
            node.attribute("class") == Some("label")
                && node.attribute("data-id") == Some("c4-rel-0")
        })
        .unwrap();
    let transform: svgtypes::Transform = label.attribute("transform").unwrap().parse().unwrap();
    let offset = Point::new(transform.e, transform.f);
    assert!(
        shaft
            .segments()
            .any(|segment| segment.nearest(offset, 1e-6).distance_sq < 80.0_f64.powi(2))
    );
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
    let session = RenderEnvironment::deterministic().begin_session().unwrap();
    let (layout, svg) = render_model(&parsed, &LayoutOptions::headless_svg_defaults(), session);
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

#[test]
fn c4_shoal_paints_native_theme_on_canonical_routes() {
    let source = r#"---
config:
  layout: shoal
---
C4Context
System(a, "First")
System(b, "Second")
BiRel(a, b, "Requests", "TLS")
"#;
    let parsed = MermaidEngine::new()
        .parse_diagram_for_render_model_sync(source, ParseOptions::strict())
        .unwrap()
        .unwrap();
    let theme = ThemeCompiler::new()
        .compile_json(
            br##"{
                "schema_version": 2,
                "extends": "paper",
                "roles": {"stroke.connector": "#123456"},
                "form": {"radius": 18, "connector_width": 3, "edge_style": "orthogonal", "marker_style": "open"}
            }"##,
            None,
        )
        .unwrap();
    let session = RenderEnvironment::deterministic()
        .begin_session_with_native_theme(NativeTheme::new(theme))
        .unwrap();
    let options = LayoutOptions::headless_svg_defaults().with_shoal_layout_frame(None);
    let (_, svg) = render_model(parsed.presentation(), &options, session);
    assert!(svg.shoal_layout_frame().is_some());

    let doc = Document::parse(svg.svg()).unwrap();
    let node = element(&doc, "node", "a");
    let outline = node
        .descendants()
        .find(|node| node.has_tag_name("rect") && node.attribute("rx").is_some())
        .unwrap();
    assert_eq!(number(outline, "rx"), 18.0);
    let edge = element(&doc, "edge", "c4-rel-0");
    assert_eq!(edge.attribute("stroke"), Some("#123456"));
    assert_eq!(number(edge, "stroke-width"), 3.0);
    let knots = points(edge);
    check_terminal(edge, knots[0], true, true);
    check_terminal(edge, *knots.last().unwrap(), false, true);
    let marker_url = edge.attribute("marker-end").unwrap();
    let marker_id = marker_url
        .strip_prefix("url(#")
        .unwrap()
        .strip_suffix(')')
        .unwrap();
    let marker_path = doc
        .descendants()
        .find(|node| node.attribute("id") == Some(marker_id))
        .unwrap()
        .first_element_child()
        .unwrap();
    assert_eq!(marker_path.attribute("d"), Some("M 0 0 L 10 5 L 0 10"));
    assert_eq!(marker_path.attribute("fill"), Some("none"));
}
