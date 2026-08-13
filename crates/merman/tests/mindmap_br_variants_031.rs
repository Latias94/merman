#[cfg(all(feature = "svg", feature = "layout-cytoscape"))]
#[test]
fn mindmap_br_variants_031_matches_upstream_node_geometry() {
    const DIAGRAM_ID: &str = "stress_mindmap_br_variants_031";
    const NODE_1_ID: &str = "stress_mindmap_br_variants_031-node_1";

    let input = include_str!("../../../fixtures/mindmap/stress_mindmap_br_variants_031.mmd");

    let output = merman::Renderer::new()
        .render(
            merman::RenderRequest::svg(
                input,
                merman::OperationControl::new(),
                merman::SvgRequest {
                    options: merman::svg::SvgRenderOptions {
                        diagram_id: Some(DIAGRAM_ID.to_string()),
                        ..Default::default()
                    },
                    ..Default::default()
                },
            )
            .with_parse_options(merman::ParseOptions::strict()),
        )
        .expect("render svg");
    let merman::RenderOutput::Svg(Some(svg)) = output else {
        panic!("diagram detected");
    };
    let svg = svg.svg();
    assert!(
        !svg.contains(r#"style="undefined"#),
        "mindmap edge paths should not leak invalid style tokens"
    );

    let doc = roxmltree::Document::parse(&svg).expect("valid svg xml");
    let node_1 = doc
        .descendants()
        .find(|n| n.has_tag_name("g") && n.attribute("id") == Some(NODE_1_ID))
        .expect("prefixed node_1 should exist in svg output");

    // Upstream Mermaid 11.15 renders the 2-line label with a 48px foreignObject height and
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
