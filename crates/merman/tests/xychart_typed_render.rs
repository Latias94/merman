#[cfg(feature = "svg")]
#[test]
fn xychart_renderer_uses_typed_render_path() {
    let input = r#"
xychart
title "Typed XYChart"
x-axis [A, B]
y-axis 1 --> 3
bar [1, 2]
"#;

    let output = merman::Renderer::new()
        .with_parse_options(merman::ParseOptions::strict())
        .render(merman::RenderRequest::svg(
            input,
            merman::OperationControl::new(),
            merman::SvgRequest {
                options: merman::svg::SvgRenderOptions {
                    diagram_id: Some("typed_xychart".to_string()),
                    ..Default::default()
                },
                ..Default::default()
            },
        ))
        .expect("render svg");
    let merman::RenderOutput::Svg(Some(svg)) = output else {
        panic!("diagram not detected");
    };

    assert!(svg.svg().contains("typed_xychart"));
    assert!(svg.svg().contains("xychart"));
}

#[cfg(feature = "svg")]
#[test]
fn xychart_renderer_renders_line_labels_and_axis_rotation() {
    let input = r#"%%{init: {"xyChart": {"xAxis": {"labelRotation": 45}}}}%%
xychart
x-axis [Alpha, Beta]
y-axis 0 --> 10
line [2 "low", 8 "high"]
"#;

    let output = merman::Renderer::new()
        .with_parse_options(merman::ParseOptions::strict())
        .render(merman::RenderRequest::svg(
            input,
            merman::OperationControl::new(),
            merman::SvgRequest {
                options: merman::svg::SvgRenderOptions {
                    diagram_id: Some("typed_xychart_labels".to_string()),
                    ..Default::default()
                },
                ..Default::default()
            },
        ))
        .expect("render svg");
    let merman::RenderOutput::Svg(Some(svg)) = output else {
        panic!("diagram not detected");
    };

    assert!(svg.svg().contains(">low<"));
    assert!(svg.svg().contains(">high<"));
    assert!(svg.svg().contains("rotate(45)"));
}

#[cfg(all(feature = "png", feature = "diagram-xychart"))]
#[test]
fn native_xychart_bar_elevation_survives_export() {
    use merman::theme::{BuiltinPreset, ThemeSelection};
    use merman::{OperationControl, RenderOutput, RenderRequest, Renderer, SvgRequest};

    let source = "xychart\nx-axis [A, B]\ny-axis 0 --> 10\nbar [3, 7]\nline [2, 6]\n";
    let RenderOutput::Document(Some(document)) = Renderer::new()
        .render(RenderRequest::document(
            source,
            OperationControl::new(),
            SvgRequest::default()
                .with_native_theme(ThemeSelection::preset(BuiltinPreset::Studio, None)),
        ))
        .unwrap()
    else {
        panic!("expected a rendered XYChart");
    };
    let svg = roxmltree::Document::parse(document.svg()).unwrap();
    let blurred_bars = svg
        .descendants()
        .filter(|node| node.attribute("data-merman-theme-depth") == Some("node"))
        .filter(|node| {
            node.descendants()
                .any(|child| child.has_tag_name("feGaussianBlur"))
        })
        .count();
    assert_eq!(blurred_bars, 2);

    let png = document
        .export_png(&Default::default(), OperationControl::new())
        .unwrap();
    assert!(png.bytes().starts_with(b"\x89PNG\r\n\x1a\n"));
    assert_eq!(png.export_report().conversion().filter_primitives, 2);
    assert_eq!(png.theme_outcome(), document.theme_outcome());

    #[cfg(feature = "pdf")]
    {
        let pdf = document
            .export_pdf(&Default::default(), OperationControl::new())
            .unwrap();
        assert!(pdf.bytes().starts_with(b"%PDF-"));
        assert_eq!(pdf.plan().filtered_groups, 2);
        assert!(pdf.plan().effective_image_pixels > 0);
        assert_eq!(pdf.theme_outcome(), document.theme_outcome());
    }
}
