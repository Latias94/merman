#![cfg(all(feature = "diagram-block", feature = "layout-shoal"))]

use kurbo::{BezPath, ParamCurve, Point, Rect, Shape, Vec2};
use merman_core::{MermaidEngine, ParseOptions};
use merman_render::environment::RenderEnvironment;
use merman_render::svg::SvgRenderOptions;
use merman_render::theme::{Color, NativeTheme, ThemeCompiler};
use merman_render::{LayoutOptions, family};
use roxmltree::{Document, Node};
use serde_json::{Value, json};

const BLOCK: &str = r#"block
  columns 4
  A space:2 B
  C space:2 D
  A -- "sync" <--> B
  C --> D
"#;

fn render(body: &str, config: Value, mode: Option<&str>, canvas: Color) -> String {
    let source = format!("---\nconfig: {config}\n---\n{body}");
    let parsed = MermaidEngine::new()
        .parse_diagram_for_render_model_sync(&source, ParseOptions::strict())
        .unwrap()
        .unwrap();
    let environment = RenderEnvironment::deterministic();
    let session = if let Some(mode) = mode {
        let theme = json!({
            "schema_version": 2,
            "extends": "paper",
            "form": { "edge_label_bg": mode },
        });
        let theme = ThemeCompiler::new()
            .compile_json(theme.to_string().as_bytes(), None)
            .unwrap();
        environment.begin_session_with_native_theme(NativeTheme::new(theme))
    } else {
        environment.begin_session()
    }
    .unwrap()
    .with_canvas_background(canvas);
    family::prepare(
        parsed.presentation(),
        &LayoutOptions::headless_svg_defaults(),
        session,
    )
    .unwrap()
    .render_svg(&SvgRenderOptions::default(), &Default::default())
    .unwrap()
    .svg()
    .to_owned()
}

fn edge<'a, 'input>(document: &'a Document<'input>, suffix: &str) -> Node<'a, 'input> {
    document
        .descendants()
        .find(|node| {
            node.attribute("data-edge") == Some("true")
                && node
                    .attribute("data-id")
                    .is_some_and(|id| id.ends_with(suffix))
        })
        .expect("Block edge")
}

fn definition<'a, 'input>(node: Node<'a, 'input>, attribute: &str) -> Node<'a, 'input> {
    let id = node
        .attribute(attribute)
        .unwrap_or_else(|| panic!("missing {attribute} on {:?}", node.attribute("id")))
        .strip_prefix("url(#")
        .unwrap()
        .strip_suffix(')')
        .unwrap();
    let mut matches = node
        .document()
        .descendants()
        .filter(|candidate| candidate.attribute("id") == Some(id));
    let definition = matches.next().expect("referenced definition");
    assert!(matches.next().is_none(), "duplicate definition {id}");
    definition
}

fn translation(node: Node<'_, '_>) -> Vec2 {
    let (x, y) = node
        .attribute("transform")
        .unwrap()
        .strip_prefix("translate(")
        .unwrap()
        .strip_suffix(')')
        .unwrap()
        .split_once(',')
        .unwrap();
    Vec2::new(x.trim().parse().unwrap(), y.trim().parse().unwrap())
}

fn caption_bounds(shaft: Node<'_, '_>, html: bool) -> Rect {
    let label = shaft
        .document()
        .descendants()
        .find(|node| {
            node.attribute("class") == Some("label")
                && node.attribute("data-id") == shaft.attribute("data-id")
        })
        .expect("label for this shaft");
    let origin = Point::ORIGIN + translation(label.parent_element().unwrap()) + translation(label);
    let box_node = label.first_element_child().unwrap();
    assert!(box_node.has_tag_name(if html { "foreignObject" } else { "rect" }));
    let number = |attr| box_node.attribute(attr).unwrap_or("0").parse().unwrap();
    Rect::from_origin_size(
        origin + Vec2::new(number("x"), number("y")),
        (number("width"), number("height")),
    )
}

fn assert_cutout(shaft: Node<'_, '_>, path: &BezPath, label: Rect) -> BezPath {
    assert!(
        path.segments().any(|segment| (0..=16)
            .any(|sample| { label.contains(segment.eval(f64::from(sample) / 16.0)) })),
        "the fixture must have a shaft that crosses its caption"
    );
    let clip = definition(shaft, "clip-path");
    assert!(clip.has_tag_name("clipPath"));
    let contour = clip.first_element_child().unwrap();
    assert_eq!(contour.attribute("clip-rule"), Some("evenodd"));
    let contour = BezPath::from_svg(contour.attribute("d").unwrap()).unwrap();
    let inside = label.inflate(-0.1, -0.1);
    for point in [
        inside.center(),
        Point::new(inside.x0, inside.y0),
        Point::new(inside.x1, inside.y0),
        Point::new(inside.x0, inside.y1),
        Point::new(inside.x1, inside.y1),
    ] {
        assert_eq!(contour.winding(point) % 2, 0, "visible ink at {point:?}");
    }
    for x in [label.x0 - 0.5, label.x1 + 0.5] {
        assert_ne!(contour.winding(Point::new(x, label.center().y)) % 2, 0);
    }
    contour
}

#[test]
fn shoal_block_cuts_transparent_captions_on_the_owning_shaft() {
    let opaque = Color::rgb(250, 249, 248);
    for html in [false, true] {
        for (mode, canvas, paint) in [
            (Some("auto"), Color::TRANSPARENT, None),
            (Some("canvas"), Color::TRANSPARENT, None),
            (Some("transparent"), opaque, None),
            (None, opaque, Some("rgba(12, 23, 34, 0)")),
            (None, Color::TRANSPARENT, None),
        ] {
            let mut config = json!({
                "layout": "shoal",
                "look": if html { "handDrawn" } else { "classic" },
                "htmlLabels": html,
            });
            if let Some(paint) = paint {
                config["themeVariables"] = json!({ "edgeLabelBackground": paint });
            }
            let svg = render(BLOCK, config, mode, canvas);
            let document = Document::parse(&svg).unwrap();
            let shaft = edge(&document, "-A-B");
            let path = BezPath::from_svg(shaft.attribute("d").unwrap()).unwrap();
            let contour = assert_cutout(shaft, &path, caption_bounds(shaft, html));
            assert_eq!(edge(&document, "-C-D").attribute("clip-path"), None);
            for (attribute, contact) in [
                ("marker-start", path.segments().next().unwrap().start()),
                ("marker-end", path.segments().last().unwrap().end()),
            ] {
                assert!(definition(shaft, attribute).has_tag_name("marker"));
                for offset in [Vec2::new(-6.0, -6.0), Vec2::new(6.0, 6.0)] {
                    assert_ne!(contour.winding(contact + offset) % 2, 0);
                }
            }
            let css = document
                .descendants()
                .find(|node| node.has_tag_name("style"))
                .and_then(|node| node.text())
                .unwrap();
            assert!(css.contains(".edgeLabel{background-color:transparent;"));
            assert!(css.contains(".labelBkg{background-color:transparent}"));
            assert!(css.contains("background-color:transparent;fill:transparent;"));
        }
    }
}

#[test]
fn shoal_block_s61_separates_shapes_routes_and_transparent_captions() {
    use kurbo::Line;
    use merman_layout_shoal::Side;
    use merman_render::model::{BlockDiagramLayout, LayoutPoint};
    use merman_render::theme::{Appearance, BuiltinPreset};

    let source = r#"---
config:
  layout: shoal
  look: handDrawn
  htmlLabels: false
---
block-beta
  columns 4
  A(("Start Circle")) space B{"Decision<br/>Diamond"} C(["Stadium"])
  space:4
  D{{"Hexagon"}} E[/"Trapezoid"\] F[("Primary<br/>Cylinder")] G((("Double<br/>Circle")))
  A -- "fwd" --> B
  B -- "rev" --> A
  B -- "p1" --> C
  B -- "p2" --> C
  A -- "fan1" --> F
  D -- "fan2" --> F
  E -- "fan3" --> F
  F -- "sync" --> G
  C -- "loop" --> G
"#;
    let parsed = MermaidEngine::new()
        .parse_diagram_for_render_model_sync(source, ParseOptions::strict())
        .unwrap()
        .unwrap();
    let theme = ThemeCompiler::new()
        .compile_preset(BuiltinPreset::Studio, Some(Appearance::Dark))
        .unwrap();
    let session = RenderEnvironment::deterministic()
        .begin_session_with_native_theme(NativeTheme::new(theme))
        .unwrap()
        .with_canvas_background(Color::TRANSPARENT);
    let artifact = family::prepare(
        parsed.presentation(),
        &LayoutOptions::headless_svg_defaults(),
        session,
    )
    .unwrap();
    let layout: BlockDiagramLayout = serde_json::from_value(
        artifact.layout_geometry_json().unwrap()["layout"]["BlockDiagram"].clone(),
    )
    .unwrap();
    let svg = artifact
        .render_svg(&SvgRenderOptions::default(), &Default::default())
        .unwrap();
    let document = Document::parse(svg.svg()).unwrap();
    let view_box: Vec<f64> = document
        .root_element()
        .attribute("viewBox")
        .unwrap()
        .split_ascii_whitespace()
        .map(|number| number.parse().unwrap())
        .collect();
    let viewport = Rect::from_origin_size((view_box[0], view_box[1]), (view_box[2], view_box[3]));
    let [a, b, c, d, e, f, g] = layout.nodes.as_slice() else {
        panic!("expected the seven S61 blocks");
    };
    assert_eq!([a.x, b.x, c.x], [d.x, f.x, g.x]);
    assert_eq!([a.y, b.y, c.y], [a.y; 3]);
    assert_eq!([d.y, e.y, f.y, g.y], [d.y; 4]);
    let outlines: Vec<_> = layout
        .shape_geometries
        .iter()
        .map(|shape| {
            let (x0, y0, x1, y1) = shape.rendered_extents();
            let outline = Rect::new(x0, y0, x1, y1);
            let slot = shape.allocated;
            let slot = Rect::from_center_size((slot.x, slot.y), (slot.width, slot.height));
            assert!(
                slot.inflate(0.01, 0.01).contains_rect(outline),
                "{} exceeds its slot",
                shape.id
            );
            assert!(viewport.contains_rect(outline));
            (shape.id.as_str(), outline)
        })
        .collect();
    for (i, (id, outline)) in outlines.iter().enumerate() {
        for (other, bounds) in &outlines[..i] {
            assert_eq!(
                outline.intersect(*bounds).area(),
                0.0,
                "{id} overlaps {other}"
            );
        }
    }

    assert_eq!(layout.edges.len(), 9);
    let captions: Vec<_> = layout
        .edges
        .iter()
        .map(|link| caption_bounds(edge(&document, &link.id), false))
        .collect();
    let point = |p: &LayoutPoint| Point::new(p.x, p.y);
    for (i, (link, &caption)) in layout.edges.iter().zip(&captions).enumerate() {
        let shaft = edge(&document, &link.id);
        let path = BezPath::from_svg(shaft.attribute("d").unwrap()).unwrap();
        let enters = |rect: Rect| {
            let interior = rect.inflate(-0.01, -0.01);
            path.segments().any(|seg| {
                interior.contains(seg.start())
                    || interior.path_segments(0.0).any(|edge| {
                        let border = Line::new(edge.start(), edge.end());
                        !seg.intersect_line(border).is_empty()
                    })
            })
        };
        assert!(viewport.contains_rect(path.bounding_box()));
        assert!(viewport.contains_rect(caption));
        assert!(enters(caption), "{} shaft misses its caption", link.id);
        assert!(definition(shaft, "clip-path").has_tag_name("clipPath"));
        assert!(definition(shaft, "marker-end").has_tag_name("marker"));
        for (id, outline) in &outlines {
            assert_eq!(
                caption.intersect(*outline).area(),
                0.0,
                "{} caption overlaps {id}",
                link.id
            );
            if *id != link.from && *id != link.to {
                assert!(!enters(*outline), "{} crosses {id}", link.id);
            }
        }
        for (j, &peer) in captions.iter().enumerate().filter(|(j, _)| *j != i) {
            assert_eq!(
                caption.intersect(peer).area(),
                0.0,
                "{} caption overlaps its peer",
                link.id
            );
            assert!(
                !enters(peer),
                "{} crosses caption {}",
                link.id,
                layout.edges[j].id
            );
        }
        let points = &link.points;
        let last = points.len() - 1;
        for (id, contact, next) in [(&link.from, 0, 1), (&link.to, last, last - 1)] {
            let at = point(&points[contact]);
            let offset = point(&points[next]) - at;
            let (side, across) = match (offset.x, offset.y) {
                (0.0, y) if y < 0.0 => (Side::Top, at.x),
                (0.0, y) if y > 0.0 => (Side::Bottom, at.x),
                (x, 0.0) if x < 0.0 => (Side::Left, at.y),
                (x, 0.0) if x > 0.0 => (Side::Right, at.y),
                _ => panic!("{} has no straight terminal lead at {id}", link.id),
            };
            let shape = layout
                .shape_geometries
                .iter()
                .find(|shape| &shape.id == id)
                .unwrap();
            let expected = shape.face_contact(side, across).unwrap();
            assert!(
                (point(&expected) - at).hypot() < 1e-6,
                "{} misses the contour of {id}",
                link.id
            );
        }
    }
    for (left, right, reverse) in [(0, 1, true), (2, 3, false)] {
        let ends = |i: usize| {
            let points = &layout.edges[i].points;
            [point(&points[0]), point(points.last().unwrap())]
        };
        let mut peer = ends(right);
        if reverse {
            peer.reverse();
        }
        for (a, b) in ends(left).into_iter().zip(peer) {
            assert!(
                (a - b).hypot() >= 8.0,
                "coincident contacts at {a:?} and {b:?}"
            );
        }
    }
}

#[test]
fn shoal_block_routes_both_markers_clear_of_an_opaque_caption() {
    let svg = render(
        "block\n  A B\n  A -- \"sync\" <--> B\n",
        json!({ "layout": "shoal", "htmlLabels": false, "block": { "padding": 12 } }),
        Some("auto"),
        Color::rgb(255, 255, 255),
    );
    let document = Document::parse(&svg).unwrap();
    let shaft = edge(&document, "-A-B");
    assert!(shaft.attribute("clip-path").is_none());
    let label = caption_bounds(shaft, false);
    let path = BezPath::from_svg(shaft.attribute("d").unwrap()).unwrap();
    let start = path.segments().next().unwrap();
    let end = path.segments().last().unwrap();
    for (attribute, contact, toward_edge) in [
        ("marker-start", start.start(), start.end()),
        ("marker-end", end.end(), end.start()),
    ] {
        assert!(definition(shaft, attribute).has_tag_name("marker"));
        let outward = (toward_edge - contact).normalize();
        let across = Vec2::new(-outward.y, outward.x);
        let ink = [
            contact + outward * 4.4 + across * 4.0,
            contact + outward * 4.4 - across * 4.0,
            contact - outward * 4.0 + across * 4.0,
            contact - outward * 4.0 - across * 4.0,
        ]
        .into_iter()
        .fold(Rect::from_points(contact, contact), |bounds, point| {
            bounds.union_pt(point)
        });
        assert_eq!(
            label.intersect(ink).area(),
            0.0,
            "{attribute}: {ink:?} overlaps {label:?}"
        );
    }
}

#[test]
fn shoal_block_preserves_markers_when_a_caption_covers_their_contacts() {
    let svg = render(
        "block\n  A B\n  A -- \"a caption wider than the gap\" <--> B\n",
        json!({ "layout": "shoal", "htmlLabels": false, "block": { "padding": 0 } }),
        Some("transparent"),
        Color::TRANSPARENT,
    );
    let document = Document::parse(&svg).unwrap();
    let shaft = edge(&document, "-A-B");
    let label = caption_bounds(shaft, false);
    let path = BezPath::from_svg(shaft.attribute("d").unwrap()).unwrap();
    assert_cutout(shaft, &path, label);
    assert!(label.contains(path.segments().next().unwrap().start()));
    assert!(label.contains(path.segments().last().unwrap().end()));
    for attribute in ["marker-start", "marker-end"] {
        let marker_carrier = document
            .descendants()
            .find(|node| node.has_tag_name("path") && node.attribute(attribute).is_some())
            .expect("preserved endpoint marker");
        assert_eq!(marker_carrier.attribute("clip-path"), None);
        assert_eq!(marker_carrier.attribute("d"), shaft.attribute("d"));
        assert_eq!(marker_carrier.attribute("stroke"), Some("none"));
        assert!(definition(marker_carrier, attribute).has_tag_name("marker"));
    }
}

#[test]
fn block_painted_captions_and_legacy_routes_keep_their_shafts() {
    let opaque = Color::rgb(250, 249, 248);
    for (layout, mode, canvas, paint) in [
        ("shoal", Some("auto"), opaque, None),
        ("shoal", Some("pill"), Color::TRANSPARENT, None),
        ("shoal", None, Color::TRANSPARENT, Some("#123456")),
        #[cfg(feature = "layout-dagre")]
        ("dagre", Some("transparent"), Color::TRANSPARENT, None),
    ] {
        let mut config = json!({ "layout": layout });
        if let Some(paint) = paint {
            config["themeVariables"] = json!({ "edgeLabelBackground": paint });
        }
        let svg = render(BLOCK, config, mode, canvas);
        let document = Document::parse(&svg).unwrap();
        let shaft = edge(&document, "-A-B");
        assert_eq!(shaft.attribute("clip-path"), None);
        assert!(definition(shaft, "marker-start").has_tag_name("marker"));
        assert!(definition(shaft, "marker-end").has_tag_name("marker"));
    }
}
