use crate::{
    environment::RenderEnvironment, model::LayoutPoint, svg::SvgRenderOptions, theme::NativeTheme,
};
use base64::Engine as _;
use kurbo::{BezPath, ParamCurve, PathEl, PathSeg, Point, Rect, Shape, Vec2};
use merman_core::{MermaidEngine, ParseOptions};
use merman_theme_contract::v2::{Appearance, BuiltinPreset, ThemeCompiler};
use roxmltree::{Document, Node};
use serde_json::Value;
use svgtypes::{PathParser, PathSegment};

fn render(source: &str, shoal: bool) -> (Value, String) {
    let source = if shoal {
        format!("---\nconfig:\n  layout: shoal\n---\n{source}")
    } else {
        source.to_owned()
    };
    render_with_studio(&source, None)
}

fn render_with_studio(source: &str, appearance: Option<Appearance>) -> (Value, String) {
    render_with_preset(source, appearance.map(|mode| (BuiltinPreset::Studio, mode)))
}

fn render_with_preset(
    source: &str,
    preset: Option<(BuiltinPreset, Appearance)>,
) -> (Value, String) {
    let diagram = MermaidEngine::new()
        .parse_diagram_for_render_model_sync(source, ParseOptions::strict())
        .unwrap()
        .unwrap()
        .into_presentation();
    let environment = RenderEnvironment::deterministic();
    let session = if let Some((preset, appearance)) = preset {
        let theme = ThemeCompiler::new()
            .compile_preset(preset, Some(appearance))
            .unwrap();
        environment.begin_session_with_native_theme(NativeTheme::new(theme))
    } else {
        environment.begin_session()
    }
    .unwrap();
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

fn node<'a, 'i>(doc: &'a Document<'i>, id: &str) -> Node<'a, 'i> {
    let legacy_id = format!("{}-{id}", doc.root_element().attribute("id").unwrap());
    doc.descendants()
        .find(|n| {
            (n.attribute("data-et") == Some("node") && n.attribute("data-id") == Some(id))
                || n.attribute("id") == Some(legacy_id.as_str())
        })
        .unwrap()
}

fn svg_rect(doc: &Document<'_>, id: &str) -> Rect {
    let group = node(doc, id);
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

fn paths_cross(path: &BezPath, other: &BezPath) -> bool {
    let mut flattened = BezPath::new();
    kurbo::flatten(other.elements().iter().copied(), 0.01, |element| {
        flattened.push(element);
    });
    flattened.segments().any(|segment| {
        let PathSeg::Line(line) = segment else {
            unreachable!("flattening yields lines")
        };
        path.segments()
            .any(|curve| !curve.intersect_line(line).is_empty())
    })
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
    let blocker = svg_rect(&doc, "X");
    let shell = svg_rect(&doc, "zone");
    let old = BezPath::from_svg(edge(&legacy, "-A-B").attribute("d").unwrap()).unwrap();
    assert!(old.segments().any(|s| blocker.contains(s.eval(0.5))));
    let arrow = edge(&doc, "-A-B");
    let path = axis_path(arrow);
    assert!(!hits(&path, blocker.inflate(0.5, 0.5)));
    assert!(shell.contains(path[0]) && !shell.contains(*path.last().unwrap()));
    let shell_content = |doc: &Document<'_>| {
        node(doc, "zone")
            .children()
            .filter(Node::is_element)
            .map(|child| doc.input_text()[child.range()].to_owned())
            .collect::<Vec<_>>()
    };
    assert_eq!(shell_content(&doc), shell_content(&legacy));

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
    let target = svg_rect(&doc, "B");
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
        .find(|n| n.attribute("class") == Some("label") && n.attribute("data-id") == Some("1-C-D"))
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
fn enabled_block_grid_reseats_a_blocked_caption_on_its_own_clear_route() {
    let source = "block\n  columns 5\n  A space X space B\n  A --\"Over X\"--> B\n";
    let (layout, svg) = render(source, true);
    let (legacy_layout, legacy_svg) = render(source, false);
    for field in ["nodes", "shapeGeometries", "bounds"] {
        assert_eq!(layout[field], legacy_layout[field], "fixed grid {field}");
    }
    let doc = Document::parse(&svg).unwrap();
    let legacy = Document::parse(&legacy_svg).unwrap();
    let blocker = svg_rect(&doc, "X");
    let caption = |layout: &Value| {
        let label = &layout["edges"][0]["label"];
        let read = |key: &str| label[key].as_f64().unwrap();
        Rect::from_center_size((read("x"), read("y")), (read("width"), read("height")))
    };
    assert!(blocker.contains(caption(&legacy_layout).center()));
    let caption = caption(&layout);
    let path = edge(&doc, "-A-B");
    let shaft = axis_path(path);
    assert!(!hits(&shaft, blocker));
    assert!(hits(&shaft, caption));
    assert_eq!(caption.intersect(blocker).area(), 0.0);
    assert!(svg.contains("Over X"));
    assert_eq!(
        path.attribute("marker-end"),
        edge(&legacy, "-A-B").attribute("marker-end")
    );
}

#[test]
fn enabled_block_grid_recovers_wide_parallel_captions_without_crossing_a_block() {
    let source = r#"---
config:
  layout: shoal
  htmlLabels: false
---
block
  columns 3
  A X B
  A -- "WWWWWWWWW1" --> B
  A -- "WWWWWWWWW2" --> B
  A -- "WWWWWWWWW3" --> B
"#;
    let (layout, svg) =
        render_with_preset(source, Some((BuiltinPreset::Monograph, Appearance::Light)));
    let doc = Document::parse(&svg).unwrap();
    let blocker = svg_rect(&doc, "X");
    for (index, id) in ["1-A-B", "2-A-B", "3-A-B"].into_iter().enumerate() {
        let shaft = edge(&doc, id);
        let path = axis_path(shaft);
        assert!(!hits(&path, blocker), "{id} crosses X");
        let caption = doc
            .descendants()
            .find(|node| {
                node.attribute("class") == Some("label") && node.attribute("data-id") == Some(id)
            })
            .unwrap();
        let center = translate(caption.parent_element().unwrap());
        let label = &layout["edges"][index]["label"];
        let expected = Point::new(label["x"].as_f64().unwrap(), label["y"].as_f64().unwrap());
        assert!((center - expected).hypot() < 1e-6);
        assert!(
            hits(&path, Rect::from_center_size(center, (0.02, 0.02))),
            "{id} misses its published caption"
        );
        let data = base64::engine::general_purpose::STANDARD
            .decode(shaft.attribute("data-points").unwrap())
            .unwrap();
        let points: Vec<LayoutPoint> = serde_json::from_slice(&data).unwrap();
        let published: Vec<LayoutPoint> =
            serde_json::from_value(layout["edges"][index]["points"].clone()).unwrap();
        assert_eq!(points.len(), published.len());
        for (point, published) in points.iter().zip(published) {
            assert!((point.x - published.x).abs() < 1e-10 && (point.y - published.y).abs() < 1e-10);
        }
        assert!(shaft.attribute("marker-end").is_some());
    }
}

#[test]
fn enabled_block_grid_rolls_back_captions_with_their_clear_grid_routes() {
    let source = r#"---
config:
  layout: shoal
  htmlLabels: false
---
block
  columns 2
  A B
  A -- "WWW1" --> B
  A -- "WWW2" --> B
  A -- "WWW3" --> B
  A -- "WWW4" --> B
  A -- "WWW5" --> B
"#;
    let preset = Some((BuiltinPreset::Monograph, Appearance::Light));
    let (layout, svg) = render_with_preset(source, preset);
    let (grid, grid_svg) =
        render_with_preset(&source.replace("layout: shoal", "layout: dagre"), preset);
    let doc = Document::parse(&svg).unwrap();
    let grid_doc = Document::parse(&grid_svg).unwrap();
    assert_ne!(layout["edges"][0]["label"], grid["edges"][0]["label"]);
    for (index, id) in [(3, "4-A-B"), (4, "5-A-B")] {
        assert_eq!(
            edge(&doc, id).attribute("d"),
            edge(&grid_doc, id).attribute("d"),
            "{id} should keep its safe grid shaft"
        );
        assert_eq!(
            layout["edges"][index]["label"],
            grid["edges"][index]["label"]
        );
        assert_eq!(
            layout["edges"][index]["points"],
            grid["edges"][index]["points"]
        );
    }
}

#[test]
fn enabled_block_grid_spreads_nested_storage_branches_and_caps_free_bends() {
    let source = r#"---
config:
  layout: shoal
  htmlLabels: true
---
block
  columns 4
  Header["🌐 分布式流式计算平台总览 Architecture Overview"]:4
  space:4
  block:ingest:2
    columns 2
    InTitle["数据接入层"]:2
    Kafka[("Kafka 集群")]
    Pulsar[("Pulsar 队列")]
  end
  arr1<["实时投递"]>(right)
  Compute["⚡ Flink 流式计算引擎<br/>Exactly-Once 状态快照"]
  space:4
  block:storage:4
    columns 3
    OLTP[("PostgreSQL 主库")]
    OLAP[("ClickHouse 实时数仓")]
    Archive[("S3 冷归档")]
  end
  Compute --> Archive
  Compute --> OLTP
  Compute --> OLAP
  Kafka --> Compute
"#;
    let (_, svg) = render_with_studio(source, Some(Appearance::Light));
    let doc = Document::parse(&svg).unwrap();
    let bottom = svg_rect(&doc, "Compute").y1;
    let mut radii = Vec::new();
    let paths = ["-Compute-OLTP", "-Compute-OLAP", "-Compute-Archive"].map(|id| {
        let d = edge(&doc, id).attribute("d").unwrap();
        for segment in PathParser::from(d) {
            if let PathSegment::EllipticalArc { rx, ry, .. } = segment.unwrap() {
                assert_eq!(rx, ry);
                radii.push(rx);
            }
        }
        BezPath::from_svg(d).unwrap()
    });
    let ports = paths
        .each_ref()
        .map(|path| path.segments().next().unwrap().start());
    assert!(ports.iter().all(|port| (port.y - bottom).abs() < 1e-6));
    assert!(ports.windows(2).all(|pair| pair[1].x - pair[0].x >= 8.0));
    for (index, path) in paths.iter().enumerate() {
        assert!(paths[..index].iter().all(|other| !paths_cross(path, other)));
    }
    assert!(
        radii.contains(&24.0),
        "the long turn reaches the radius cap"
    );
    assert!(radii.iter().all(|&radius| radius <= 24.0));
}

#[test]
fn enabled_block_grid_uses_other_faces_when_a_face_cannot_fit_parallel_markers() {
    let source = format!(
        "---\nconfig:\n  layout: shoal\n---\nblock\n  columns 2\n  A B\n{}",
        "  A --> B\n".repeat(5)
    );
    let (_, svg) = render_with_studio(&source, Some(Appearance::Light));
    let doc = Document::parse(&svg).unwrap();
    let contacts = ["1-A-B", "2-A-B", "3-A-B", "4-A-B", "5-A-B"].map(|id| {
        let d = edge(&doc, id).attribute("d").unwrap();
        BezPath::from_svg(d)
            .unwrap()
            .segments()
            .next()
            .unwrap()
            .start()
    });
    assert!(contacts.iter().any(|port| *port != contacts[0]));
}

#[test]
fn enabled_block_grid_keeps_sketch_fanout_clear_of_peer_labels() {
    let source = r#"---
config:
  layout: shoal
  look: handDrawn
  htmlLabels: false
---
block
  columns 6
  A["Gateway"]:2 space B["Router"]:2 space
  space:6
  block:workers:4
    columns 2
    W1["Worker A"] W2["Worker B"]
    W3["Worker C"] W4["Worker D"]
  end
  DB[("Shared DB")]:2
  A -- "route" --> B
  B -- "fanout 1" --> W1
  B -- "fanout 2" --> W2
  W1 --> DB
  W2 --> DB
  W3 --> DB
  W4 --> DB
  style B fill:#123456,opacity:0.5
"#;
    let (_, svg) = render_with_studio(source, Some(Appearance::Dark));
    let (_, legacy) = render_with_studio(
        &source.replace("layout: shoal", "layout: dagre"),
        Some(Appearance::Dark),
    );
    let doc = Document::parse(&svg).unwrap();
    let legacy = Document::parse(&legacy).unwrap();
    let ids = ["1-B-W1", "1-B-W2"];
    let label_bounds = |doc: &Document<'_>| {
        ids.map(|id| {
            let group = doc
                .descendants()
                .find(|n| {
                    n.attribute("class") == Some("label") && n.attribute("data-id") == Some(id)
                })
                .unwrap();
            let bg = group.first_element_child().unwrap();
            let origin = translate(group.parent_element().unwrap()) + translate(group).to_vec2();
            Rect::from_origin_size(
                origin + Vec2::new(number(bg, "x"), number(bg, "y")),
                (number(bg, "width"), number(bg, "height")),
            )
        })
    };
    let labels = label_bounds(&doc);
    assert_eq!(labels, label_bounds(&legacy));
    let paths = ids.map(|id| BezPath::from_svg(edge(&doc, id).attribute("d").unwrap()).unwrap());
    assert!(!paths_cross(&paths[0], &paths[1]));
    for (index, path) in paths.iter().enumerate() {
        let first = path.segments().next().unwrap();
        let last = path.segments().last().unwrap();
        for runway in [first, last] {
            let start = runway.start();
            assert!(runway.end().y - start.y >= merman_render_core::route_paint::LEAD - 1e-6);
            for t in [0.25, 0.5, 0.75, 1.0] {
                assert!((runway.eval(t).x - start.x).abs() < 1e-6);
            }
        }
        assert!(paths_cross(path, &labels[index].to_path(0.01)));
        assert!(!paths_cross(
            path,
            &labels[1 - index].inflate(0.5, 0.5).to_path(0.01)
        ));
        assert!(path.segments().any(|s| matches!(s, PathSeg::Cubic(_))));
    }

    let router = node(&doc, "B");
    let shape = router.children().find(|n| n.has_tag_name("path")).unwrap();
    assert!(shape.attribute("d").unwrap().ends_with('Z'));
    let style = shape.attribute("style").unwrap();
    assert!(style.contains("fill:#123456;") && style.contains("opacity:0.5;"));
    assert!(router.descendants().any(|n| n.text() == Some("Router")));
    assert!(
        node(&legacy, "B")
            .children()
            .any(|n| n.has_tag_name("rect"))
    );
}
