#![cfg(all(
    feature = "layout-shoal",
    any(feature = "diagram-er", feature = "diagram-requirement")
))]

use kurbo::{BezPath, ParamCurve, PathSeg, Point, Rect, Shape};
use merman_core::{MermaidEngine, ParseOptions};
use merman_render::environment::RenderEnvironment;
use merman_render::svg::SvgRenderOptions;
use merman_render::theme::{Appearance, BuiltinPreset, Color, NativeTheme, ThemeCompiler};
use merman_render::{LayoutOptions, family};
use roxmltree::{Document, Node};

fn render(body: &str, look: &str, seed: u32, appearance: Appearance) -> String {
    let source = format!(
        "---\nconfig:\n  layout: shoal\n  look: {look}\n  htmlLabels: false\n  handDrawnSeed: {seed}\n---\n{body}"
    );
    let parsed = MermaidEngine::new()
        .parse_diagram_for_render_model_sync(&source, ParseOptions::strict())
        .unwrap()
        .unwrap()
        .into_presentation();
    let theme = ThemeCompiler::new()
        .compile_preset(BuiltinPreset::Studio, Some(appearance))
        .unwrap();
    let session = RenderEnvironment::deterministic()
        .begin_session_with_native_theme(NativeTheme::new(theme))
        .unwrap()
        .with_canvas_background(Color::TRANSPARENT);
    family::prepare(&parsed, &LayoutOptions::headless_svg_defaults(), session)
        .unwrap()
        .render_svg(
            &SvgRenderOptions {
                diagram_id: Some("hand-drawn-test".to_owned()),
                viewbox_padding: 0.0,
            },
            &Default::default(),
        )
        .unwrap()
        .svg()
        .to_owned()
}

fn geometry(node: Node<'_, '_>) -> BezPath {
    BezPath::from_svg(node.attribute("d").unwrap()).unwrap()
}

fn assert_wobbles(path: &BezPath) {
    assert!(path.segments().any(|segment| {
        let from = segment.start();
        let axis = segment.end() - from;
        matches!(segment, PathSeg::Cubic(_))
            && axis.cross(segment.eval(0.5) - from).abs() > axis.hypot() * 0.03
    }));
}

fn assert_inside_viewport(node: Node<'_, '_>, path: &BezPath) {
    let translate = node
        .attribute("transform")
        .unwrap()
        .trim_start_matches("translate(")
        .trim_end_matches(')');
    let (x, y) = translate.split_once(',').unwrap();
    let center = Point::new(x.trim().parse().unwrap(), y.trim().parse().unwrap());
    let [left, top, width, height]: [f64; 4] = node
        .document()
        .root_element()
        .attribute("viewBox")
        .unwrap()
        .split_whitespace()
        .map(|value| value.parse().unwrap())
        .collect::<Vec<_>>()
        .try_into()
        .unwrap();
    let viewport = Rect::new(left, top, left + width, top + height);
    let ink = path.bounding_box() + center.to_vec2();
    assert_eq!(ink.intersect(viewport), ink);
}

#[cfg(feature = "diagram-er")]
#[test]
fn er_shoal_hand_draws_table_and_entity_frames_with_seeded_dividers() {
    let source = "erDiagram\ndirection LR\n\
        用户 {\n uuid 用户ID PK\n varchar 姓名\n}\n\
        用户 |o--o{ 订单 : 创建订单\n";
    let render_seed = |seed| render(source, "handDrawn", seed, Appearance::Light);
    let svg = render_seed(73);
    assert_eq!(svg, render_seed(73));
    let alternate_svg = render_seed(109);
    let document = Document::parse(&svg).unwrap();
    let alternate = Document::parse(&alternate_svg).unwrap();
    for (entity, shape_class) in [
        ("entity-用户-0", "er-native-outline"),
        ("entity-订单-1", "basic label-container"),
    ] {
        let find_frame = |document: &Document<'_>| {
            let node = document
                .descendants()
                .find(|node| node.attribute("data-id") == Some(entity))
                .unwrap();
            let path = node
                .descendants()
                .find(|child| child.attribute("class") == Some(shape_class))
                .unwrap();
            assert_inside_viewport(node, &geometry(path));
            geometry(path)
        };
        let frame = find_frame(&document);
        assert_wobbles(&frame);
        assert_ne!(frame, find_frame(&alternate));
    }
    let divider = document
        .descendants()
        .find(|node| node.attribute("class") == Some("divider"))
        .unwrap();
    assert_wobbles(&geometry(divider.first_element_child().unwrap()));
    let clip_id = divider
        .attribute("clip-path")
        .unwrap()
        .trim_start_matches("url(#")
        .trim_end_matches(')');
    let clip = document
        .descendants()
        .find(|node| node.attribute("id") == Some(clip_id))
        .unwrap();
    assert_eq!(
        clip.first_element_child().unwrap().attribute("d"),
        document
            .descendants()
            .find(|node| node.attribute("class") == Some("er-native-outline"))
            .unwrap()
            .attribute("d"),
    );
    let edge = document
        .descendants()
        .find(|node| node.attribute("data-et") == Some("edge"))
        .unwrap();
    assert_eq!(
        edge.attribute("mask"),
        Some("url(#hand-drawn-test_er-marker-hole-0)")
    );
    assert_eq!(
        edge.attribute("marker-end"),
        Some("url(#hand-drawn-test_er-zeroOrMoreEnd)")
    );
}

#[cfg(feature = "diagram-er")]
#[test]
fn er_shoal_bt_insets_crows_foot_tips_without_moving_the_marker_reference() {
    for (direction, inset) in [("BT", 2.25), ("TB", 0.0)] {
        let body = format!(
            "erDiagram\ndirection {direction}\nA }}o--|{{ B : owns\nB }}|..o{{ C : routes\n"
        );
        let svg = render(&body, "classic", 73, Appearance::Dark);
        let document = Document::parse(&svg).unwrap();
        for (card, width) in [("oneOrMore", 27.0), ("zeroOrMore", 39.0)] {
            for (terminal, anchor) in [("Start", 0.0), ("End", width)] {
                let id = format!("hand-drawn-test_er-{card}{terminal}");
                let marker = document
                    .descendants()
                    .find(|node| node.attribute("id") == Some(id.as_str()))
                    .unwrap();
                let reference = marker.attribute("refX").unwrap().parse::<f64>().unwrap();
                assert_eq!(reference, anchor);
                let d = geometry(marker.last_element_child().unwrap());
                let curves = d
                    .segments()
                    .filter(|segment| matches!(segment, PathSeg::Quad(_)))
                    .collect::<Vec<_>>();
                let [first, second] = curves.as_slice() else {
                    panic!("open crow's foot: {id}")
                };
                assert_eq!((first.start().x - anchor).abs(), inset);
                assert_eq!((second.end().x - anchor).abs(), inset);
                assert_eq!((first.end().x - anchor).abs(), 18.0);
                assert_eq!(first.end(), second.start());
            }
        }
    }
}

#[cfg(feature = "diagram-requirement")]
#[test]
fn requirement_shoal_hand_draws_boxes_dividers_and_shafts_at_the_same_contacts() {
    let source = "requirementDiagram\ndirection LR\n\
        requirement 资金安全 {\n id: SEC-1\n text: 资金对账必须实时完成\n risk: high\n verifymethod: analysis\n}\n\
        element 清算引擎 {\n type: 核心服务\n}\n清算引擎 - verifies -> 资金安全\n";
    let svg = render(source, "handDrawn", 73, Appearance::Light);
    let document = Document::parse(&svg).unwrap();
    for node in document
        .descendants()
        .filter(|node| node.attribute("data-et") == Some("node"))
    {
        let frame = node
            .children()
            .find(|child| child.attribute("class") == Some("basic label-container outer-path"))
            .unwrap();
        let fill = frame.first_element_child().unwrap();
        let stroke = frame.last_element_child().unwrap();
        assert_eq!(fill.attribute("d"), stroke.attribute("d"));
        let path = geometry(stroke);
        assert_wobbles(&path);
        assert_inside_viewport(node, &path);
        let divider = node
            .children()
            .find(|child| child.attribute("class") == Some("divider"))
            .unwrap();
        let divider = geometry(divider.first_element_child().unwrap());
        assert_wobbles(&divider);
        let left = divider.segments().next().unwrap().start();
        let right = divider.segments().next().unwrap().end();
        for contact in [left, right] {
            assert!(path.segments().any(|side| side.start() == contact));
        }
    }
    let classic_svg = render(source, "classic", 73, Appearance::Light);
    let classic = Document::parse(&classic_svg).unwrap();
    let find_edge = |document: &Document<'_>| {
        let edge = document
            .descendants()
            .find(|node| node.attribute("data-et") == Some("edge"))
            .unwrap();
        (
            geometry(edge),
            edge.attribute("marker-end").unwrap().to_owned(),
        )
    };
    let (shaft, marker) = find_edge(&document);
    let (classic_shaft, classic_marker) = find_edge(&classic);
    assert_eq!(marker, classic_marker);
    assert_wobbles(&shaft);
    assert_eq!(
        shaft.segments().next().unwrap().start(),
        classic_shaft.segments().next().unwrap().start()
    );
    assert_eq!(
        shaft.segments().last().unwrap().end(),
        classic_shaft.segments().last().unwrap().end()
    );
}
