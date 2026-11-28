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
