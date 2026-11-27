#![cfg(all(feature = "diagram-class", feature = "layout-shoal"))]

use kurbo::{BezPath, PathEl, Rect, Shape};
use merman_core::{MermaidEngine, ParseOptions};
use merman_render::environment::RenderEnvironment;
use merman_render::theme::{Color, NativeTheme, ThemeCompiler};
use merman_render::{LayoutOptions, family};
use roxmltree::{Document, Node};

fn render(body: &str, html: bool, mode: &str, canvas: Color) -> String {
    let source = format!(
        "---\nconfig: {{\"layout\":\"shoal\",\"look\":\"handDrawn\",\"htmlLabels\":{html},\"class\":{{\"htmlLabels\":{html}}}}}\n---\nclassDiagram\ndirection LR\n{body}"
    );
    let parsed = MermaidEngine::new()
        .parse_diagram_for_render_model_sync(&source, ParseOptions::strict())
        .unwrap()
        .unwrap();
    let theme = ThemeCompiler::new()
        .compile_json(
            format!(
                r#"{{"schema_version":2,"extends":"studio","form":{{"edge_label_bg":"{mode}"}}}}"#
            )
            .as_bytes(),
            None,
        )
        .unwrap();
    let session = RenderEnvironment::deterministic()
        .begin_session_with_native_theme(NativeTheme::new(theme).with_background(canvas))
        .unwrap();
    family::prepare(
        parsed.presentation(),
        &LayoutOptions::headless_svg_defaults(),
        session,
    )
    .unwrap()
    .render_svg(&Default::default(), &Default::default())
    .unwrap()
    .svg()
    .to_owned()
}

fn translation(node: Node<'_, '_>) -> (f64, f64) {
    let (x, y) = node
        .attribute("transform")
        .unwrap()
        .strip_prefix("translate(")
        .unwrap()
        .strip_suffix(')')
        .unwrap()
        .split_once(',')
        .unwrap();
    (x.trim().parse().unwrap(), y.trim().parse().unwrap())
}

fn assert_cutout(doc: &Document<'_>, caption: &str, html: bool, expected: bool) {
    let label = doc
        .descendants()
        .find(|node| {
            node.attribute("class") == Some("label")
                && node.attribute("data-id").is_some()
                && node.descendants().any(|text| text.text() == Some(caption))
        })
        .unwrap();
    let host = doc
        .descendants()
        .find(|path| {
            path.has_tag_name("path")
                && path.attribute("data-edge") == Some("true")
                && path.attribute("data-id") == label.attribute("data-id")
        })
        .unwrap();
    if !expected {
        assert!(host.attribute("clip-path").is_none(), "{caption}");
        return;
    }

    let url = host.attribute("clip-path").expect(caption);
    let id = url
        .strip_prefix("url(#")
        .unwrap()
        .strip_suffix(')')
        .unwrap();
    assert_eq!(
        doc.descendants()
            .filter(|node| node.attribute("clip-path") == Some(url))
            .count(),
        1,
        "the window belongs to its host edge only"
    );
    let mut definitions = doc
        .descendants()
        .filter(|node| node.attribute("id") == Some(id));
    let clip = definitions.next().unwrap();
    assert!(clip.has_tag_name("clipPath"));
    assert!(definitions.next().is_none());
    let shape = clip.first_element_child().unwrap();
    assert_eq!(shape.attribute("clip-rule"), Some("evenodd"));
    let path = BezPath::from_svg(shape.attribute("d").unwrap()).unwrap();
    let inner = path
        .elements()
        .iter()
        .rposition(|el| matches!(el, PathEl::MoveTo(_)))
        .unwrap();
    assert!(inner > 0);
    let hole = BezPath::from_vec(path.elements()[inner..].to_vec()).bounding_box();
    let (x, y) = translation(label.parent_element().unwrap());
    let (dx, dy) = translation(label);
    let background = label
        .descendants()
        .find(|node| {
            if html {
                node.has_tag_name("foreignObject")
            } else {
                node.has_tag_name("rect") && node.attribute("class") == Some("background")
            }
        })
        .unwrap();
    let number = |attr| {
        background
            .attribute(attr)
            .unwrap_or("0")
            .parse::<f64>()
            .unwrap()
    };
    let expected = Rect::from_origin_size(
        (x + dx + number("x"), y + dy + number("y")),
        (number("width"), number("height")),
    );
    for (a, b) in [
        (hole.x0, expected.x0),
        (hole.y0, expected.y0),
        (hole.x1, expected.x1),
        (hole.y1, expected.y1),
    ] {
        assert!((a - b).abs() < 1e-6, "{caption}: {hole:?} vs {expected:?}");
    }
    assert!(host.attribute("marker-end").is_some());
}

#[test]
fn transparent_canvas_cuts_only_the_class_relations_without_a_namespace_backplate() {
    let source = "namespace Platform {\nclass Catalog\nclass Service\n}\n\
        Gateway --> Service : routes\nGateway ..> Catalog : healthcheck\n\
        Service --> Catalog : reads\nProbe --> Gateway\n";
    let svg = render(source, false, "auto", Color::TRANSPARENT);
    let doc = Document::parse(&svg).unwrap();
    assert_cutout(&doc, "routes", false, true);
    assert_cutout(&doc, "healthcheck", false, true);
    assert_cutout(&doc, "reads", false, false);
    assert_eq!(
        doc.descendants()
            .filter(|node| node.has_tag_name("clipPath"))
            .count(),
        2
    );
}

#[test]
fn explicit_transparent_class_labels_cut_their_host_on_an_opaque_canvas() {
    let source = "A --> B : connects\n";
    for html in [false, true] {
        let svg = render(source, html, "transparent", Color::rgb(240, 240, 240));
        assert_cutout(&Document::parse(&svg).unwrap(), "connects", html, true);
    }
}
