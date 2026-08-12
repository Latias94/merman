#[cfg(feature = "svg")]
#[test]
fn c4_render_svg_sync_uses_typed_render_path() {
    let input = r#"
C4Context
title Typed C4
Person(customer, "Customer", "Uses the system")
System(system, "Internet Banking", "Core system")
Rel(customer, system, "Uses", "HTTPS")
"#;

    let output = merman::Renderer::new()
        .with_parse_options(merman::ParseOptions::strict())
        .render(merman::RenderRequest::svg(
            input,
            merman::OperationControl::new(),
            merman::SvgRequest {
                options: merman::svg::SvgRenderOptions {
                    diagram_id: Some("typed_c4".to_string()),
                    ..Default::default()
                },
                ..Default::default()
            },
        ))
        .expect("render svg");
    let merman::RenderOutput::Svg(Some(svg)) = output else {
        panic!("diagram not detected");
    };

    assert!(svg.svg().contains("typed_c4"));
    assert!(svg.svg().contains("c4"));
}
