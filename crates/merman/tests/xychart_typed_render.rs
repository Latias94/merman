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
