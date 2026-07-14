#[cfg(feature = "render")]
#[test]
fn c4_render_svg_sync_uses_typed_render_path() {
    let engine = merman_core::Engine::new();
    let parse_options = merman_core::ParseOptions::strict();
    let layout = merman::render::LayoutOptions::headless_svg_defaults();
    let svg_opts = merman::render::SvgRenderOptions {
        diagram_id: Some("typed_c4".to_string()),
        ..Default::default()
    };
    let input = r#"
C4Context
title Typed C4
Person(customer, "Customer", "Uses the system")
System(system, "Internet Banking", "Core system")
Rel(customer, system, "Uses", "HTTPS")
"#;

    let svg = merman::render::render_svg_sync(&engine, input, parse_options, &layout, &svg_opts)
        .expect("render svg")
        .expect("diagram detected");

    assert!(svg.contains("typed_c4"));
    assert!(svg.contains("c4"));
}
