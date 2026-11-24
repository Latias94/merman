use super::*;
use kurbo::{BezPath, ParamCurve, PathEl, Point, Rect, Shape, Vec2};
use roxmltree::{Document, Node};
use serde_json::Value;

fn render(source: &str, shoal: bool) -> (Value, String) {
    let source = if shoal {
        format!("---\nconfig:\n  layout: shoal\n---\n{source}")
    } else {
        source.to_owned()
    };
    let diagram = MermaidEngine::new()
        .parse_diagram_for_render_model_sync(&source, ParseOptions::strict())
        .unwrap()
        .unwrap()
        .into_presentation();
    let session = RenderEnvironment::deterministic().begin_session().unwrap();
    let artifact = crate::family::prepare(&diagram, &crate::LayoutOptions::default(), session)
        .expect("prepare Block geometry");
    let layout =
        artifact.layout_geometry_json().unwrap().as_value()["layout"]["BlockDiagram"].clone();
    let options = SvgRenderOptions {
        diagram_id: Some("block-route".into()),
        ..Default::default()
    };
    let svg = artifact.render_svg(&options, &Default::default()).unwrap();
    (layout, svg.svg().to_owned())
}

fn edge<'a, 'i>(doc: &'a Document<'i>, suffix: &str) -> Node<'a, 'i> {
    doc.descendants()
        .find(|n| {
            n.attribute("data-et") == Some("edge")
                && n.attribute("data-id")
                    .is_some_and(|id| id.ends_with(suffix))
        })
        .expect("painted edge")
}

fn axis_path(node: Node<'_, '_>) -> Vec<Point> {
    let path = BezPath::from_svg(node.attribute("d").unwrap()).unwrap();
    let points: Vec<_> = path
        .elements()
        .iter()
        .enumerate()
        .map(|(index, element)| match element {
            PathEl::MoveTo(p) if index == 0 => *p,
            PathEl::LineTo(p) if index > 0 => *p,
            _ => panic!("expected one orthogonal subpath, got {path:?}"),
        })
        .collect();
    assert!(points.len() >= 2);
    assert!(
        points
            .windows(2)
            .all(|p| p[0] != p[1] && (p[0].x == p[1].x || p[0].y == p[1].y))
    );
    points
}

fn hits(points: &[Point], r: Rect) -> bool {
    points
        .windows(2)
        .map(|p| Rect::from_points(p[0], p[1]))
        .any(|leg| leg.x0 < r.x1 && leg.x1 > r.x0 && leg.y0 < r.y1 && leg.y1 > r.y0)
}

fn svg_rect(doc: &Document<'_>, id: &str) -> Rect {
    let group = doc
        .descendants()
        .find(|n| n.attribute("id") == Some(id))
        .unwrap();
    let rect = group.children().find(|n| n.has_tag_name("rect")).unwrap();
    let origin = translate(group);
    Rect::from_origin_size(
        origin + Vec2::new(number(rect, "x"), number(rect, "y")),
        (number(rect, "width"), number(rect, "height")),
    )
}

fn number(node: Node<'_, '_>, name: &str) -> f64 {
    node.attribute(name).unwrap().parse().unwrap()
}

fn translate(node: Node<'_, '_>) -> Point {
    let (x, y) = node
        .attribute("transform")
        .unwrap()
        .strip_prefix("translate(")
        .unwrap()
        .strip_suffix(')')
        .unwrap()
        .split_once(',')
        .unwrap();
    Point::new(x.trim().parse().unwrap(), y.trim().parse().unwrap())
}

#[test]
fn enabled_block_grid_routes_around_a_drawn_box_through_its_composite_shell() {
    let source =
        "block\n  columns 3\n  block:zone:2\n    columns 3\n    A space X\n  end\n  B\n  A --> B\n";
    let (placed, legacy) = render(source, false);
    let (routed, svg) = render(source, true);
    for field in ["nodes", "shapeGeometries", "bounds"] {
        assert_eq!(placed[field], routed[field], "fixed grid {field}");
    }
    assert_eq!(render(source, true), (routed.clone(), svg.clone()));

    let doc = Document::parse(&svg).unwrap();
    let legacy = Document::parse(&legacy).unwrap();
    let blocker = svg_rect(&doc, "block-route-X");
    let shell = svg_rect(&doc, "block-route-zone");
    let old = BezPath::from_svg(edge(&legacy, "-A-B").attribute("d").unwrap()).unwrap();
    assert!(old.segments().any(|s| blocker.contains(s.eval(0.5))));
    let arrow = edge(&doc, "-A-B");
    let path = axis_path(arrow);
    assert!(!hits(&path, blocker.inflate(0.5, 0.5)));
    assert!(shell.contains(path[0]) && !shell.contains(*path.last().unwrap()));
    let shell_group = |svg: &str, doc: &Document<'_>| {
        let g = doc
            .descendants()
            .find(|n| n.attribute("id") == Some("block-route-zone"))
            .unwrap();
        svg[g.range()].to_owned()
    };
    assert_eq!(
        shell_group(&svg, &doc),
        shell_group(legacy.input_text(), &legacy)
    );

    let points = base64::engine::general_purpose::STANDARD
        .decode(arrow.attribute("data-points").unwrap())
        .unwrap();
    let points: Vec<LayoutPoint> = serde_json::from_slice(&points).unwrap();
    let published: Vec<LayoutPoint> =
        serde_json::from_value(routed["edges"][0]["points"].clone()).unwrap();
    assert_eq!(points.len(), published.len());
    for (point, published) in points.iter().zip(published) {
        assert!((point.x - published.x).abs() < 1e-10 && (point.y - published.y).abs() < 1e-10);
    }
    let target = svg_rect(&doc, "block-route-B");
    let contact = points.last().unwrap();
    assert!((target.x0 - contact.x).abs() < 1e-3);
    let tip = path.last().unwrap();
    // The pointEnd tip is at x=10, refX=5, in an 8px-wide 10-unit viewport.
    assert!((contact.x - tip.x - 4.0).abs() < 1e-10 && contact.y == tip.y);
}

#[test]
fn enabled_block_grid_reserves_the_drawn_marker_footprint() {
    let source = "block\n  columns 5\n  space:2 C space:2\n  space:5\n  A space X space B\n  C --x X\n  A --> B\n";
    let (_, marked) = render(source, true);
    let (_, unmarked) = render(&source.replace("--x", "---"), true);
    let marked = Document::parse(&marked).unwrap();
    let unmarked = Document::parse(&unmarked).unwrap();
    let terminal = edge(&marked, "-C-X");
    let reference = terminal
        .attribute("marker-end")
        .unwrap()
        .strip_prefix("url(#")
        .unwrap()
        .strip_suffix(')')
        .unwrap();
    let marker = marked
        .descendants()
        .find(|n| n.attribute("id") == Some(reference))
        .unwrap();
    assert_eq!(marker.attribute("markerUnits"), Some("userSpaceOnUse"));
    assert_eq!(marker.attribute("viewBox"), Some("0 0 11 11"));
    assert_eq!(number(marker, "markerWidth"), 11.0);
    assert_eq!(number(marker, "markerHeight"), 11.0);
    assert_eq!(marker.attribute("orient"), Some("auto"));
    let node = marker.first_element_child().unwrap();
    assert!(
        node.attribute("style")
            .unwrap()
            .contains("stroke-width: 2;")
    );
    let ink = BezPath::from_svg(node.attribute("d").unwrap()).unwrap();
    let shaft = axis_path(terminal);
    let [before, tip] = shaft[shaft.len() - 2..] else {
        unreachable!()
    };
    let along = (tip - before).normalize();
    let across = Vec2::new(-along.y, along.x);
    let rx = number(marker, "refX");
    let ry = number(marker, "refY");
    let world = |p: Point| tip + along * (p.x - rx) + across * (p.y - ry);
    let local = ink.bounding_box().inflate(1.0, 1.0);
    let world_box = Rect::from_points(
        world(Point::new(local.x0, local.y0)),
        world(Point::new(local.x1, local.y1)),
    );
    let clear = axis_path(edge(&marked, "-A-B"));
    let previous = axis_path(edge(&unmarked, "-A-B"));
    assert!(!hits(&clear, world_box));
    assert!(hits(&previous, world_box));
    assert!(
        ink.elements().iter().any(|element| {
            let PathEl::LineTo(p) = element else {
                return false;
            };
            let p = world(*p);
            hits(&previous, Rect::from_points(p, p).inflate(0.1, 0.1))
        }),
        "without the marker obstacle an actual cross stroke lies on the other shaft"
    );
}

#[test]
fn enabled_block_grid_keeps_a_label_attached_and_routes_the_other_edge_around_it() {
    let source = "block\n  columns 5\n  space:2 C space:2\n  A space:3 B\n  space:2 D space:2\n  C --\"Route label\"--> D\n  A --> B\n";
    let (placed, legacy) = render(source, false);
    let (routed, svg) = render(source, true);
    assert_eq!(placed["edges"][0]["label"], routed["edges"][0]["label"]);
    let doc = Document::parse(&svg).unwrap();
    let legacy = Document::parse(&legacy).unwrap();
    let label = doc
        .descendants()
        .find(|n| n.attribute("data-id") == Some("1-C-D"))
        .unwrap();
    let box_node = label.first_element_child().unwrap();
    assert!(box_node.has_tag_name("foreignObject"));
    let center = translate(label.parent_element().unwrap());
    let bounds = Rect::from_center_size(
        center,
        (number(box_node, "width"), number(box_node, "height")),
    );
    let old = BezPath::from_svg(edge(&legacy, "-A-B").attribute("d").unwrap()).unwrap();
    assert!(old.segments().any(|s| bounds.contains(s.eval(0.5))));
    assert!(!hits(&axis_path(edge(&doc, "-A-B")), bounds));
    assert!(hits(&axis_path(edge(&doc, "-C-D")), bounds));
}

#[test]
fn enabled_block_grid_falls_back_when_an_anchored_label_is_blocked() {
    let source = "block\n  columns 5\n  A space X space B\n  A --\"Over X\"--> B\n";
    assert_eq!(render(source, true), render(source, false));
}
