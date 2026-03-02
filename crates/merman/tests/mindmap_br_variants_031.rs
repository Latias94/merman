#[cfg(feature = "render")]
#[test]
fn mindmap_br_variants_031_matches_upstream_node_geometry() {
    let input = include_str!("../../../fixtures/mindmap/stress_mindmap_br_variants_031.mmd");

    let engine = merman_core::Engine::new();
    let parse_options = merman_core::ParseOptions {
        suppress_errors: false,
    };

    let layout = merman::render::LayoutOptions {
        viewport_width: 800.0,
        viewport_height: 600.0,
        text_measurer: std::sync::Arc::new(
            merman::render::VendoredFontMetricsTextMeasurer::default(),
        ),
        math_renderer: None,
        use_manatee_layout: true,
    };

    let svg_opts = merman::render::SvgRenderOptions {
        diagram_id: Some("stress_mindmap_br_variants_031".to_string()),
        ..Default::default()
    };

    let svg = merman::render::render_svg_sync(&engine, input, parse_options, &layout, &svg_opts)
        .expect("render svg")
        .expect("diagram detected");

    let doc = roxmltree::Document::parse(&svg).expect("valid svg xml");
    let node_1 = doc
        .descendants()
        .find(|n| n.has_tag_name("g") && n.attribute("id") == Some("node_1"))
        .expect("node_1 should exist in svg output");

    // Upstream Mermaid (11.12.x) renders the 2-line label with a 48px foreignObject height and
    // a 68px outer rect (padding=20).
    let node_1_rect = node_1
        .children()
        .find(|n| n.has_tag_name("rect"))
        .expect("node_1 should have a <rect>");
    assert_eq!(node_1_rect.attribute("height"), Some("68"));
    assert_eq!(node_1_rect.attribute("y"), Some("-34"));

    let node_1_fo = node_1
        .descendants()
        .find(|n| n.has_tag_name("foreignObject"))
        .expect("node_1 should contain a <foreignObject>");
    let fo_h: f64 = node_1_fo
        .attribute("height")
        .expect("foreignObject height")
        .parse()
        .expect("foreignObject height f64");
    assert!((fo_h - 48.0).abs() < 1e-9, "foreignObject height={fo_h}");

    let fo_w: f64 = node_1_fo
        .attribute("width")
        .expect("foreignObject width")
        .parse()
        .expect("foreignObject width f64");
    assert!(
        (39.0..=41.0).contains(&fo_w),
        "expected foreignObject width ~= 40, got {fo_w}"
    );
}
