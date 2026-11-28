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

fn relation_label<'a, 'input>(doc: &'a Document<'input>, id: &str) -> Node<'a, 'input> {
    doc.descendants()
        .find(|node| {
            node.attribute("class") == Some("label") && node.attribute("data-id") == Some(id)
        })
        .unwrap()
}

fn relation_label_center(label: Node<'_, '_>) -> Point {
    let at: svgtypes::Transform = label.attribute("transform").unwrap().parse().unwrap();
    let mut rows = label.children().filter(|node| node.has_tag_name("text"));
    let baseline =
        |row: Node<'_, '_>| number(row, "y") + number(row.first_element_child().unwrap(), "dy");
    let first = baseline(rows.next().unwrap());
    let last = rows.next_back().map_or(first, baseline);
    Point::new(at.e, at.f + first.midpoint(last))
}

fn shaft_distance(path: &BezPath, center: Point) -> f64 {
    path.segments()
        .map(|segment| segment.nearest(center, 1e-3).distance_sq.sqrt())
        .fold(f64::INFINITY, f64::min)
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

fn resolve_marker<'a, 'input>(edge: Node<'a, 'input>, url: &str) -> Node<'a, 'input> {
    let id = url
        .strip_prefix("url(#")
        .unwrap()
        .strip_suffix(')')
        .unwrap();
    edge.document()
        .descendants()
        .find(|node| node.has_tag_name("marker") && node.attribute("id") == Some(id))
        .unwrap()
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
        let marker = resolve_marker(edge, marker);
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
fn c4_shoal_dark_theme_keeps_arrow_fill_in_sync_with_each_relation() {
    let source = r#"---
config:
  layout: shoal
  theme: dark
---
C4Context
Enterprise_Boundary(group, "Boundary") {
  System(a, "First")
  System(b, "Second")
}
System(c, "Third")
BiRel(a, b, "Uses")
Rel_Back(b, c, "Returns")
UpdateRelStyle(b, c, $textColor="gold", $lineColor="orange")
"#;
    let (_, svg) = render(source, &LayoutOptions::headless_svg_defaults());
    let doc = Document::parse(svg.svg()).unwrap();
    for (idx, stroke, text, markers) in [
        (0, "lightgrey", "#ccc", &["marker-start", "marker-end"][..]),
        (1, "orange", "gold", &["marker-start"][..]),
    ] {
        let id = format!("c4-rel-{idx}");
        let edge = element(&doc, "edge", &id);
        assert_eq!(edge.attribute("stroke"), Some(stroke));
        for marker in markers {
            let path = resolve_marker(edge, edge.attribute(*marker).unwrap())
                .first_element_child()
                .unwrap();
            assert_eq!(path.attribute("fill"), Some(stroke));
        }
        assert_eq!(
            relation_label(&doc, &id)
                .descendants()
                .find(|node| node.has_tag_name("text"))
                .unwrap()
                .attribute("fill"),
            Some(text)
        );
    }
    let title = doc
        .descendants()
        .find(|node| node.has_tag_name("tspan") && node.text() == Some("Boundary"))
        .unwrap();
    assert_eq!(
        title.parent_element().unwrap().attribute("fill"),
        Some("#ccc")
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
fn c4_shoal_wraps_and_seats_the_nested_audit_captions() {
    let source = r#"C4Context
Person(u, "Operations Engineer", "Monitors fleet health")
Enterprise_Boundary(ent, "Global Cloud Corp") {
  System_Boundary(reg, "US-East Region") {
    System(api, "Control Plane API", "Coordinates cluster rollouts")
    SystemDb(db, "State Store", "Consistent Raft metadata")
  }
  System(obs, "Observability Hub", "Metrics, traces, and alerts")
}
BiRel(u, api, "Submits deployment plans and inspects live rollout progress", "HTTPS / gRPC")
Rel(api, db, "Reads and writes strongly consistent cluster state snapshots", "Raft<br/>TLS")
Rel(api, obs, "Streams structured telemetry", "OTLP")
"#;
    let options = LayoutOptions::headless_svg_defaults();
    let authored = format!("---\nconfig:\n  layout: shoal\n---\n{source}");
    let (layout, svg) = render(&authored, &options);
    let (_, again) = render(&authored, &options);
    assert_eq!(svg.svg(), again.svg());
    let doc = Document::parse(svg.svg()).unwrap();
    let mut ink = Vec::new();
    for (i, rel) in layout.rels.iter().enumerate() {
        if i < 2 {
            assert!(rel.label.line_count > 1);
            assert!(rel.label.width <= 190.0);
        }
        let id = format!("c4-rel-{i}");
        let label = relation_label(&doc, &id);
        let transform: svgtypes::Transform = label.attribute("transform").unwrap().parse().unwrap();
        let rows: Vec<_> = label
            .children()
            .filter(|node| node.has_tag_name("text"))
            .collect();
        let technology = rel.techn.as_ref().unwrap();
        let (message_rows, technology_rows) = rows.split_at(rel.label.line_count);
        assert_eq!(technology_rows.len(), technology.line_count);
        let y = |text: Node<'_, '_>| {
            number(text, "y") + number(text.first_element_child().unwrap(), "dy")
        };
        let last_message = y(*message_rows.last().unwrap());
        let first_technology = y(technology_rows[0]);
        assert!(first_technology - last_message > 12.0);
        assert!(
            technology_rows
                .iter()
                .all(|text| text.attribute("font-style") == Some("italic"))
        );
        let half_width = rel.label.width.max(technology.width) / 2.0;
        let label_ink = Rect::new(
            transform.e - half_width,
            transform.f + y(rows[0]) - 6.0,
            transform.e + half_width,
            transform.f + y(*rows.last().unwrap()) + 6.0,
        );
        for &other in &ink {
            assert_eq!(label_ink.intersect(other).area(), 0.0);
        }
        ink.push(label_ink);
        let shaft = path(element(&doc, "edge", &id));
        assert!(shaft.segments().any(
            |segment| segment.nearest(label_ink.center(), 1e-6).distance_sq <= 120.0_f64.powi(2)
        ));
    }

    let (native, svg) = render(source, &options);
    assert_eq!(native.rels[0].label.line_count, 1);
    assert!(native.rels[0].label.width > 190.0);
    assert!(!svg.svg().contains("-label-cutout"));
    assert!(!svg.svg().contains("clip-path="));
}

#[test]
fn c4_shoal_cuts_out_only_the_shaft_behind_an_on_route_caption() {
    let source = r#"---
config:
  layout: shoal
  c4:
    c4ShapeMargin: 90
  themeVariables:
    useGradient: true
---
C4Context
System_Boundary(app, "Application") {
  System(c4-rel-0-label-cutout, "API")
  System(gradient, "Database")
}
BiRel(c4-rel-0-label-cutout, gradient, "Reads widely distributed state", "TLS")
"#;
    let options = LayoutOptions::headless_svg_defaults()
        .with_container_size(310.0, 600.0)
        .with_adaptation(LayoutAdaptation::FitWidth);
    let (_, svg) = render(source, &options);
    let verify = |svg: &str| {
        let doc = Document::parse(svg).unwrap();
        let root_id = doc.root_element().attribute("id").unwrap();
        let gradient = format!("{root_id}-gradient");
        let mut definitions = doc
            .descendants()
            .filter(|node| node.attribute("id") == Some(gradient.as_str()));
        assert!(definitions.next().unwrap().has_tag_name("linearGradient"));
        assert!(definitions.next().is_none());
        assert!(svg.contains(&format!("url(#{gradient})")));
        let shaft = element(&doc, "edge", "c4-rel-0");
        let reference = shaft
            .attribute("clip-path")
            .expect("the caption crosses the shaft");
        let id = reference
            .strip_prefix("url(#")
            .unwrap()
            .strip_suffix(')')
            .unwrap();
        let clip = doc
            .descendants()
            .find(|node| node.attribute("id") == Some(id))
            .unwrap();
        assert!(clip.has_tag_name("clipPath"));
        let alias = element(&doc, "node", "c4-rel-0-label-cutout");
        assert_ne!(alias.attribute("id"), Some(id));
        let cutout = clip.first_element_child().unwrap();
        assert_eq!(cutout.attribute("clip-rule"), Some("evenodd"));
        let cutout = BezPath::from_svg(cutout.attribute("d").unwrap()).unwrap();
        let data = points(shaft);
        check_terminal(shaft, data[0], true, true);
        check_terminal(shaft, *data.last().unwrap(), false, true);
        let axis = path(shaft).segments().next().unwrap();
        assert!(cutout.contains(axis.start()));
        assert!(cutout.contains(path(shaft).segments().last().unwrap().end()));
        let label = relation_label(&doc, "c4-rel-0");
        let at: svgtypes::Transform = label.attribute("transform").unwrap().parse().unwrap();
        assert!(!cutout.contains(Point::new(at.e, at.f)));
        assert!(path(shaft).segments().any(|segment| {
            let p = segment.eval(segment.nearest(Point::new(at.e, at.f), 1e-6).t);
            !cutout.contains(p)
        }));
        assert_eq!(
            doc.descendants()
                .filter(|node| node.attribute("clip-path") == Some(reference))
                .count(),
            1
        );
    };
    verify(svg.svg());
    let finalized = svg
        .finalize_resvg(&crate::svg::SvgPipeline::resvg_safe())
        .unwrap();
    verify(finalized.svg().as_str());
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

#[test]
fn c4_shoal_hand_drawn_cylinders_dark_boundaries_and_multiline_wrap() {
    let source = r#"---
config:
  layout: shoal
  look: handDrawn
  theme: dark
---
C4Context
System_Boundary(b0, "Core Platform") {
  SystemDb(db, "Primary Ledger", "Stores immutable journal")
  SystemQueue(q, "Event Bus", "Streams async updates")
}
Rel(db, q, "Publishes<br/>replicated change capture events across regional zones", "Kafka/TLS")
"#;
    let options = LayoutOptions::headless_svg_defaults();
    let (layout, svg) = render(source, &options);
    let doc = Document::parse(svg.svg()).unwrap();

    // B3: handDrawn C4 cylinders emit a split body + separate open rough-stroke rim path.
    for alias in ["db", "q"] {
        let node = element(&doc, "node", alias);
        let rim = node.descendants().find(|el| {
            el.has_tag_name("path")
                && el.attribute("class") == Some("rough-stroke")
                && el
                    .attribute("style")
                    .is_some_and(|s| s.contains("fill:none !important"))
        });
        assert!(
            rim.is_some(),
            "C4 handDrawn {alias} must emit a separate stroke-only rim path"
        );
    }

    // M1: dark Mermaid theme under Shoal must not fall back to invisible #444444 boundary stroke.
    let boundary_outline = element(&doc, "cluster", "b0")
        .children()
        .find(|el| {
            el.has_tag_name("path")
                && el.attribute("class") == Some("c4-boundary-outline")
                && el.attribute("stroke-dasharray").is_some()
                && el.attribute("stroke").is_some()
        })
        .expect("C4 boundary outline");
    assert_ne!(
        boundary_outline.attribute("stroke"),
        Some("#444444"),
        "dark Shoal C4 boundary must use light theme lineColor instead of #444444"
    );

    // M3: `<br/>`-separated long second line wraps independently (> 2 lines total) and stays in bounds.
    assert!(
        layout.rels[0].label.line_count > 2,
        "expected wrapped <br/> relation caption to have > 2 lines, got {}",
        layout.rels[0].label.line_count
    );
    let vb = doc.root_element().attribute("viewBox").unwrap();
    let parts: Vec<f64> = vb
        .split_whitespace()
        .map(|s| s.parse::<f64>().unwrap())
        .collect();
    let label = relation_label(&doc, "c4-rel-0");
    let at: svgtypes::Transform = label.attribute("transform").unwrap().parse().unwrap();
    assert!(
        at.e >= parts[0]
            && at.e <= parts[0] + parts[2]
            && at.f >= parts[1]
            && at.f <= parts[1] + parts[3],
        "relation label ({}, {}) must lie inside viewBox {:?}",
        at.e,
        at.f,
        parts
    );
}

#[test]
fn c4_shoal_keeps_dynamic_captions_local_and_clips_only_their_own_shafts() {
    let source = r#"---
config:
  layout: shoal
---
C4Dynamic
Person(shopper, "Shopper", "Mobile user")
Container_Boundary(b1, "Checkout Services") {
  Container(bff, "Mobile BFF", "TypeScript")
  Container(auth, "Auth Service", "Go")
  Container(pay, "Payment Service", "Rust")
  ContainerDb(db, "Orders DB", "PostgreSQL")
}
Rel(shopper, bff, "Submit checkout with 3DS device fingerprint", "HTTPS/JSON")
Rel(bff, auth, "Validate session token and step-up claim", "gRPC/mTLS")
Rel(auth, bff, "Return signed identity claims", "gRPC/mTLS")
Rel(bff, pay, "Authorize payment intent with idempotency key", "gRPC/mTLS")
Rel(pay, bff, "Return payment confirmation receipt", "gRPC/mTLS")
Rel(pay, db, "Mark authorization captured", "SQL/TLS")
Rel(bff, shopper, "Display order confirmation screen", "HTTPS/JSON")
"#;
    let (_, svg) = render(source, &LayoutOptions::headless_svg_defaults());
    let doc = Document::parse(svg.svg()).unwrap();
    for idx in 0..7 {
        let id = format!("c4-rel-{idx}");
        let edge = element(&doc, "edge", &id);
        let label = relation_label(&doc, &id);
        let at: svgtypes::Transform = label.attribute("transform").unwrap().parse().unwrap();
        let center = Point::new(at.e, at.f);
        let edge_path = path(edge);
        let min_dist = edge_path
            .segments()
            .map(|seg| seg.nearest(center, 1e-3).distance_sq.sqrt())
            .fold(f64::INFINITY, f64::min);
        assert!(
            min_dist <= 260.0,
            "{id} caption is {min_dist:.1}px away from its own edge"
        );
        if let Some(reference) = edge.attribute("clip-path") {
            let clip_id = reference
                .strip_prefix("url(#")
                .unwrap()
                .strip_suffix(')')
                .unwrap();
            let clip = doc
                .descendants()
                .find(|node| node.attribute("id") == Some(clip_id))
                .unwrap();
            let cutout = path(clip.first_element_child().unwrap());
            let contours = cutout
                .elements()
                .iter()
                .filter(|element| matches!(element, kurbo::PathEl::MoveTo(_)))
                .count();
            assert_eq!(
                contours, 2,
                "{id} must have an outer contour and one caption hole"
            );
            assert!(
                !cutout.contains(center),
                "{id} hole must belong to its own caption"
            );
        }
    }
}

#[test]
fn c4_shoal_dense_captions_are_closest_to_their_own_shaft() {
    let source = r#"---
config:
  layout: shoal
---
C4Context
System(a, "Left Top", "Source")
System(b, "Mid Top", "Relay")
System(c, "Right Top", "Target")
System(d, "Left Bot", "Peer 1")
System(e, "Mid Bot", "Peer 2")
System(f, "Right Bot", "Peer 3")
Rel(a, c, "Long top span edge across center", "HTTPS")
Rel(a, f, "Long cross diagonal alpha alpha alpha", "gRPC")
Rel(c, d, "Long cross diagonal beta beta beta", "gRPC")
Rel(b, e, "Center vertical edge with wide caption text", "SQL")
Rel(a, e, "Offset crossover one with wide caption", "REST")
Rel(c, e, "Offset crossover two with wide caption", "REST")
Rel(d, f, "Bottom span edge across center", "Kafka")
UpdateRelStyle(a, f, $offsetX="-10", $offsetY="-5")
UpdateRelStyle(c, d, $offsetX="10", $offsetY="5")
"#;
    let (_, svg) = render(source, &LayoutOptions::default());
    let doc = Document::parse(svg.svg()).unwrap();
    let paths: Vec<_> = (0..7)
        .map(|idx| path(element(&doc, "edge", &format!("c4-rel-{idx}"))))
        .collect();
    for (idx, own) in paths.iter().enumerate() {
        let label = relation_label(&doc, &format!("c4-rel-{idx}"));
        let center = relation_label_center(label);
        let own_distance = shaft_distance(own, center);
        assert!(
            own_distance < 100.0,
            "caption {idx} is {own_distance}px away"
        );
        for (other, route) in paths.iter().enumerate() {
            let other_distance = shaft_distance(route, center);
            assert!(
                other == idx || own_distance < other_distance,
                "caption {idx} is closer to shaft {other} ({other_distance:.1}px) than to its own ({own_distance:.1}px)",
            );
        }
    }
}

#[test]
fn c4_shoal_crossing_shafts_clear_foreign_marker_runways() {
    let source = r#"---
config:
  layout: shoal
  htmlLabels: false
---
C4Container
Container(n1, "Node 1")
Container(n2, "Node 2")
Container(n3, "Node 3")
Container(n4, "Node 4")
Container(b1, "Bottom 1")
Container(b2, "Bottom 2")
Container(b3, "Bottom 3")
Container(b4, "Bottom 4")
Rel(n1, n4, "Call Call Call Call Call Call Call Call")
Rel(n1, b4, "Call Call Call Call Call Call Call Call")
Rel(n4, b1, "Call Call Call Call Call Call Call Call")
Rel(n2, b2, "Call Call Call Call Call Call Call Call")
Rel(n3, b3, "Call Call Call Call Call Call Call Call")
"#;
    let (_, svg) = render(source, &LayoutOptions::headless_svg_defaults());
    let doc = Document::parse(svg.svg()).unwrap();
    let edges: Vec<_> = (0..5)
        .map(|idx| element(&doc, "edge", &format!("c4-rel-{idx}")))
        .collect();
    let routes: Vec<_> = edges.iter().map(|&edge| points(edge)).collect();
    for (owner, &edge) in edges.iter().enumerate() {
        let route = &routes[owner];
        let last = route.len() - 1;
        for (attr, [tip, neighbor]) in [("marker-start", [0, 1]), ("marker-end", [last, last - 1])]
        {
            if edge.attribute(attr).is_none() {
                continue;
            }
            let tip = route[tip];
            let axis = (route[neighbor] - tip).normalize();
            let half_width = Vec2::new(axis.y.abs(), axis.x.abs()) * 5.5;
            let back = tip + axis * (11.0 + merman_layout_shoal::MIN_MARKER_LEAD);
            let runway = Rect::from_points(tip, back).inflate(half_width.x, half_width.y);
            for (other, route) in routes
                .iter()
                .enumerate()
                .filter(|(other, _)| *other != owner)
            {
                assert!(
                    route
                        .windows(2)
                        .all(|leg| !bounds_touch(Rect::from_points(leg[0], leg[1]), runway)),
                    "shaft {other} enters the marker runway of relation {owner}"
                );
            }
        }
    }
}

#[test]
fn c4_shoal_keeps_reciprocal_branches_nested_across_boundary() {
    let source = r#"---
config:
  layout: shoal
  htmlLabels: false
---
C4Dynamic
Container_Boundary(store, "Storefront") {
    Component(spa, "Browser SPA", "React", "Customer storefront")
    Component(api, "Checkout API", "Rust", "Validates basket")
    Component(inv, "Inventory Svc", "Rust", "Reserves stock")
    ComponentDb(db, "Orders DB", "PostgreSQL", "Stores orders")
}
System_Ext(psp, "Stripe PSP", "External payment gateway")
Rel(spa, api, "POST /checkout", "HTTPS/JSON")
Rel(api, inv, "Reserve SKUs", "gRPC")
Rel(inv, db, "Lock stock rows", "SQL")
Rel(api, psp, "Charge card", "HTTPS")
Rel(psp, api, "Payment authorized", "JSON")
Rel(api, spa, "Return confirmation", "HTTP 201")
"#;
    let (_, svg) = render(source, &LayoutOptions::headless_svg_defaults());
    let doc = Document::parse(svg.svg()).unwrap();
    let outbound = points(element(&doc, "edge", "c4-rel-3"));
    let incoming = points(element(&doc, "edge", "c4-rel-4"));
    assert!(outbound[0].x < incoming.last().unwrap().x);
    assert!(outbound.last().unwrap().x < incoming[0].x);
    for leg in outbound.windows(2) {
        let a = Rect::from_points(leg[0], leg[1]);
        assert!(
            incoming
                .windows(2)
                .all(|leg| !bounds_touch(a, Rect::from_points(leg[0], leg[1]))),
            "the reciprocal shafts touch or cross"
        );
    }
}

#[test]
fn c4_shoal_keeps_the_detour_column_through_the_boundary() {
    let source = r#"---
config:
  layout: shoal
  htmlLabels: false
---
C4Context
Person(customer, "Retail Customer", "Uses mobile and web banking")
Enterprise_Boundary(bank, "Acme Bank") {
    System(core, "Core Banking", "Manages accounts and transfers")
    SystemDb(vault, "Customer Vault", "PII and KYC store")
}
System_Ext(swift, "SWIFT Network", "International wire rail")
Rel(customer, core, "Initiates wire transfer", "HTTPS")
BiRel(core, vault, "Verifies KYC status", "gRPC")
Rel(core, swift, "Dispatches MT103 message", "ISO 20022")
"#;
    let (_, svg) = render(source, &LayoutOptions::headless_svg_defaults());
    let doc = Document::parse(svg.svg()).unwrap();
    let points = points(element(&doc, "edge", "c4-rel-2"));
    assert!(points[0].x > points.last().unwrap().x);
    assert!(points.windows(2).all(|leg| leg[0].x >= leg[1].x));
}

fn bounds_touch(a: Rect, b: Rect) -> bool {
    a.x0 <= b.x1 && b.x0 <= a.x1 && a.y0 <= b.y1 && b.y0 <= a.y1
}

#[test]
fn c4_shoal_view_box_contains_the_wide_title() {
    let title = "An exceptionally wide title for a one-card diagram ".repeat(8);
    let source = format!(
        "---\nconfig:\n  layout: shoal\n  c4:\n    diagramMarginX: 0\n---\nC4Context\ntitle {title}\nSystem(a, \"A\")\n"
    );
    let (_, svg) = render(&source, &LayoutOptions::default());
    let doc = Document::parse(svg.svg()).unwrap();
    let view_box: svgtypes::ViewBox = doc
        .root_element()
        .attribute("viewBox")
        .unwrap()
        .parse()
        .unwrap();
    let title = doc
        .root_element()
        .children()
        .find(|node| node.has_tag_name("text"))
        .unwrap();
    let center = number(title, "x");
    let minimum_width = 5.0 * title.text().unwrap().len() as f64;
    assert!(view_box.x <= center - minimum_width / 2.0);
    assert!(view_box.x + view_box.w >= center + minimum_width / 2.0);
}

#[test]
fn c4_shoal_replays_saved_order_by_alias_after_reordering_cards() {
    let render_order = |source, previous| {
        render(
            source,
            &LayoutOptions::default().with_shoal_layout_frame(previous),
        )
    };
    let before =
        "---\nconfig:\n  layout: shoal\n---\nC4Context\nSystem(a, \"A\")\nSystem(b, \"B\")\n";
    let after = "---\nconfig:\n  layout: shoal\n---\nC4Context\nSystem(b, \"B\")\nSystem(a, \"Updated A\")\n";
    let (_, first) = render_order(before, None);
    let mut previous = serde_json::to_value(first.shoal_layout_frame().unwrap().as_ref()).unwrap();
    let saved = std::sync::Arc::new(serde_json::from_value(previous.clone()).unwrap());
    previous["family"] = "architecture".into();
    let foreign_frame = std::sync::Arc::new(serde_json::from_value(previous).unwrap());
    let (default, _) = render_order(after, None);
    let (replayed, _) = render_order(after, Some(saved));
    let (foreign, _) = render_order(after, Some(foreign_frame));
    let position = |shape: &crate::model::C4ShapeLayout| (shape.y, shape.x);
    assert!(position(&default.shapes[0]) < position(&default.shapes[1]));
    assert!(position(&replayed.shapes[1]) < position(&replayed.shapes[0]));
    assert_eq!(position(&foreign.shapes[0]), position(&default.shapes[0]));
    assert_eq!(position(&foreign.shapes[1]), position(&default.shapes[1]));
}
