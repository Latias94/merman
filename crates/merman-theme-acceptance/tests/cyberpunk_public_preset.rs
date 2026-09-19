use merman::svg::{DiagramThemeCompiler, ThemePreset};
use merman::{
    Engine, MermaidConfig, OperationControl, RenderOutput, RenderRequest, Renderer,
    TargetAdmissionReason, TargetAdmissionStatus,
};

fn decode_png(bytes: &[u8]) -> (u32, u32, Vec<u8>) {
    let mut reader = png::Decoder::new(std::io::Cursor::new(bytes))
        .read_info()
        .unwrap();
    let mut pixels = vec![0; reader.output_buffer_size().unwrap()];
    let info = reader.next_frame(&mut pixels).unwrap();
    assert_eq!(info.color_type, png::ColorType::Rgba);
    pixels.truncate(info.buffer_size());
    (info.width, info.height, pixels)
}

fn rasterize(svg: &str) -> (u32, u32, Vec<u8>) {
    let session = merman_render::environment::RenderEnvironment::deterministic()
        .begin_session()
        .unwrap();
    let compatible = merman_render::svg::finalize_resvg_svg(svg, &session).unwrap();
    let bytes = merman_export::svg_to_png(&compatible, &Default::default()).unwrap();
    decode_png(&bytes)
}

fn override_paint(svg: &str, node: roxmltree::Node<'_, '_>, declaration: &str) -> String {
    let mut changed = svg.to_owned();
    if declaration == "marker-end:none" {
        changed.replace_range(
            node.attribute_node("marker-end").unwrap().range_value(),
            "none",
        );
    } else if let Some(style) = node.attribute_node("style") {
        changed.insert_str(
            style.range_value().end,
            &format!(";{declaration}!important"),
        );
    } else {
        let offset = node.range().start + 1 + node.tag_name().name().len();
        changed.insert_str(offset, &format!(" style=\"{declaration}!important\""));
    }
    changed
}

fn assert_pixel_contribution(
    original: &(u32, u32, Vec<u8>),
    svg: &str,
    node: roxmltree::Node<'_, '_>,
    declaration: &str,
    label: &str,
) {
    let changed = rasterize(&override_paint(svg, node, declaration));
    assert_eq!((original.0, original.1), (changed.0, changed.1), "{label}");
    assert!(
        original
            .2
            .chunks_exact(4)
            .zip(changed.2.chunks_exact(4))
            .any(|(before, after)| before != after),
        "{label} must contribute actual PNG pixels"
    );
}

fn assert_complete_scene(source: &str, expected_labels: &[&str], effects: usize, arrows: usize) {
    let compiler = DiagramThemeCompiler::new();
    let saved =
        serde_json::to_vec(&compiler.export_preset(ThemePreset::Cyberpunk).unwrap()).unwrap();
    let original_theme = compiler.compile_preset(ThemePreset::Cyberpunk).unwrap();
    let imported_theme = DiagramThemeCompiler::new()
        .compile_recipe(serde_json::from_slice(&saved).unwrap())
        .unwrap();
    assert_eq!(
        original_theme.recipe_fingerprint(),
        imported_theme.recipe_fingerprint()
    );
    let renderer = Renderer::new().with_engine(Engine::new().with_site_config(
        MermaidConfig::from_value(serde_json::json!({"htmlLabels": false})),
    ));
    let mut outputs = Vec::new();
    for theme in [original_theme, imported_theme] {
        let RenderOutput::Document(Some(document)) = renderer
            .render(
                RenderRequest::document(source, OperationControl::new(), Default::default())
                    .with_theme(theme),
            )
            .unwrap()
        else {
            panic!("complete public scene must produce a document")
        };
        let png = document
            .export_png(&Default::default(), OperationControl::new())
            .unwrap();
        assert_eq!(
            png.admission().status(),
            TargetAdmissionStatus::HostDependent
        );
        assert_eq!(
            png.admission().reasons(),
            &[TargetAdmissionReason::SystemOrHostFontDependency]
        );
        assert_eq!(
            png.export_report()
                .native_filter_receipt()
                .unwrap()
                .reference_count(),
            effects as u32
        );
        outputs.push((document.svg().to_owned(), decode_png(png.bytes())));
    }
    assert_eq!(outputs[0].0, outputs[1].0);
    assert!(
        outputs[0].1 == outputs[1].1,
        "recipe export/import must preserve every PNG pixel"
    );
    let (svg, original) = &outputs[0];
    assert!(
        *original == rasterize(svg),
        "the mutation export path must reproduce the public facade PNG exactly"
    );
    let xml = roxmltree::Document::parse(svg).unwrap();
    assert!(
        !xml.descendants()
            .any(|node| node.has_tag_name("foreignObject"))
    );
    let filtered = xml
        .descendants()
        .filter(|node| node.has_attribute("filter"))
        .collect::<Vec<_>>();
    assert_eq!(filtered.len(), effects);
    let mut glyph_svg = svg.clone();
    for terminal in filtered.into_iter().rev() {
        glyph_svg = override_paint(&glyph_svg, terminal, "filter:none");
    }
    let glyph_pixels = rasterize(&glyph_svg);
    let glyph_xml = roxmltree::Document::parse(&glyph_svg).unwrap();
    let labels = glyph_xml
        .descendants()
        .filter(|node| node.has_tag_name("text"))
        .map(|node| {
            let text = node
                .descendants()
                .filter(|child| child.is_text())
                .filter_map(|child| child.text())
                .collect::<String>();
            (node, text.trim().to_owned())
        })
        .filter(|(_, text)| !text.is_empty())
        .collect::<Vec<_>>();
    let mut actual_labels = labels
        .iter()
        .map(|(_, text)| text.as_str())
        .collect::<Vec<_>>();
    let mut expected_labels = expected_labels.to_vec();
    actual_labels.sort_unstable();
    expected_labels.sort_unstable();
    assert_eq!(actual_labels, expected_labels);
    for (node, text) in labels {
        assert_pixel_contribution(&glyph_pixels, &glyph_svg, node, "visibility:hidden", &text);
    }
    for (attribute, declaration, count) in [
        ("filter", "filter:none", effects),
        ("marker-end", "marker-end:none", arrows),
        ("data-merman-theme-canvas-layer", "visibility:hidden", 3),
    ] {
        let terminals = xml
            .descendants()
            .filter(|node| node.has_attribute(attribute))
            .collect::<Vec<_>>();
        assert_eq!(terminals.len(), count, "{attribute}");
        for (index, terminal) in terminals.into_iter().enumerate() {
            assert_pixel_contribution(
                original,
                svg,
                terminal,
                declaration,
                &format!("{attribute} terminal {index}"),
            );
        }
    }
}

#[test]
fn complete_flowchart_public_recipe_paints_labels_glows_arrows_and_canvas() {
    assert_complete_scene(
        include_str!("../../merman-theme-fixtures/fixtures/public-cyberpunk/flowchart.mmd"),
        &[
            "Browse Products",
            "Item in Stock?",
            "Add to Cart",
            "Out of Stock",
            "Checkout",
            "Yes",
            "No",
        ],
        13,
        3,
    );
}

#[test]
fn complete_sequence_public_recipe_paints_labels_glows_arrows_and_canvas() {
    assert_complete_scene(
        include_str!("../../merman-theme-fixtures/fixtures/public-cyberpunk/sequence.mmd"),
        &[
            "Client",
            "Client",
            "API",
            "API",
            "Request data",
            "Return data",
            "Verify token",
            "loop",
            "[Each request]",
        ],
        21,
        2,
    );
}

#[test]
fn complete_xychart_public_recipe_paints_labels_glows_and_canvas() {
    assert_complete_scene(
        include_str!("../../merman-theme-fixtures/fixtures/public-cyberpunk/xychart.mmd"),
        &[
            "Request Volume",
            "Window",
            "Requests",
            "A",
            "B",
            "C",
            "D",
            "0",
            "1",
            "2",
            "3",
            "4",
            "5",
            "6",
            "7",
            "8",
            "9",
            "10",
        ],
        28,
        0,
    );
}
