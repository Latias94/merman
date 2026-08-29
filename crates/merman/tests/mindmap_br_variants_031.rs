#[cfg(all(feature = "svg", feature = "layout-cytoscape"))]
#[test]
fn mindmap_br_variants_031_preserves_multiline_label_geometry() {
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

    let doc = roxmltree::Document::parse(svg).expect("valid svg xml");
    let node_1 = doc
        .descendants()
        .find(|n| n.has_tag_name("g") && n.attribute("id") == Some(NODE_1_ID))
        .expect("prefixed node_1 should exist in svg output");

    let node_1_rect = node_1
        .children()
        .find(|n| n.has_tag_name("rect"))
        .expect("node_1 should have a <rect>");
    let node_1_fo = node_1
        .descendants()
        .find(|n| n.has_tag_name("foreignObject"))
        .expect("node_1 should contain a <foreignObject>");

    let rect_w: f64 = node_1_rect
        .attribute("width")
        .expect("node rect width")
        .parse()
        .expect("node rect width f64");
    let rect_h: f64 = node_1_rect
        .attribute("height")
        .expect("node rect height")
        .parse()
        .expect("node rect height f64");
    let rect_x: f64 = node_1_rect
        .attribute("x")
        .expect("node rect x")
        .parse()
        .expect("node rect x f64");
    let rect_y: f64 = node_1_rect
        .attribute("y")
        .expect("node rect y")
        .parse()
        .expect("node rect y f64");
    let fo_h: f64 = node_1_fo
        .attribute("height")
        .expect("foreignObject height")
        .parse()
        .expect("foreignObject height f64");
    let fo_w: f64 = node_1_fo
        .attribute("width")
        .expect("foreignObject width")
        .parse()
        .expect("foreignObject width f64");

    let label_rows = node_1_fo
        .descendants()
        .filter(|node| node.is_text())
        .filter_map(|node| node.text())
        .map(str::trim)
        .filter(|text| !text.is_empty())
        .collect::<Vec<_>>();
    assert_eq!(label_rows, ["line 1", "line 2"]);

    assert!(fo_w.is_finite() && fo_w > 0.0, "foreignObject width={fo_w}");
    assert!(
        fo_h.is_finite() && fo_h > 24.0,
        "two label rows should be taller than one text row: {fo_h}"
    );
    let vertical_padding = rect_h - fo_h;
    assert!(
        vertical_padding > 0.0,
        "mindmap node should retain positive label padding: rect={rect_h}, label={fo_h}"
    );
    assert!(
        ((rect_w - fo_w) - 2.0 * vertical_padding).abs() < 1e-9,
        "rect-shaped mindmap nodes should use twice as much total horizontal as vertical padding: rect=({rect_w}, {rect_h}), label=({fo_w}, {fo_h})"
    );
    assert!(
        (rect_x + rect_w / 2.0).abs() < 1e-9 && (rect_y + rect_h / 2.0).abs() < 1e-9,
        "mindmap node geometry should remain centered: x={rect_x}, y={rect_y}, width={rect_w}, height={rect_h}"
    );
}
