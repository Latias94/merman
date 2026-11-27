#![cfg(all(feature = "svg", feature = "diagram-flowchart"))]

use merman::theme::{BuiltinPreset, Color, ThemeCompiler, ThemeSelection};
use merman::{OperationControl, RenderRequest, Renderer, SvgRequest};

#[test]
fn editable_preset_reproduces_the_canvas_and_honors_background_overrides() {
    let renderer = Renderer::new();
    let source = "flowchart LR\nA[Explore] --> B[Record]";
    let preset = BuiltinPreset::Blueprint;
    let imported = ThemeSelection::compiled(
        ThemeCompiler::new()
            .compile_json(preset.json().as_bytes(), None)
            .unwrap(),
    );
    let render = |request| {
        renderer
            .render_svg_data(RenderRequest::svg(source, OperationControl::new(), request))
            .unwrap()
            .unwrap()
    };
    let direct =
        render(SvgRequest::default().with_native_theme(ThemeSelection::preset(preset, None)));
    let request = SvgRequest::default().with_native_theme(imported);
    assert_eq!(render(request.clone()), direct);

    for (override_color, fill) in [
        (None, Some("#0E3558")),
        (Some(Color::rgb(18, 52, 86)), Some("#123456")),
        (Some(Color::TRANSPARENT), None),
    ] {
        let options = match override_color {
            Some(color) => request.clone().with_background(color),
            None => request.clone(),
        };
        let svg = render(options);
        let document = roxmltree::Document::parse(&svg).unwrap();
        let canvas = document
            .descendants()
            .find(|node| node.attribute("class") == Some("merman-theme-canvas"))
            .expect("native canvas group");
        assert_eq!(
            canvas
                .first_element_child()
                .filter(|node| node.has_tag_name("rect"))
                .and_then(|node| node.attribute("fill")),
            fill
        );
        assert_eq!(
            canvas
                .descendants()
                .any(|node| node.has_tag_name("pattern")),
            fill.is_some()
        );
    }
}
