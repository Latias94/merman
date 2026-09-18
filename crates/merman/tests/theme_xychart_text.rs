//! Role typography crosses recipe exchange, layout and public export entry points.

#![cfg(feature = "svg")]

use merman::svg::DiagramThemeCompiler;
use merman::{OperationControl, RenderOutput, RenderRequest, Renderer};

#[test]
fn xy_text_role_recipe_survives_exchange_and_public_rendering() {
    let recipe = serde_json::json!({
        "schema_version": 1,
        "kind": "complete_spec",
        "complete_spec": {"styles": [
            {"kind":"rule","family":"xychart","target":"title","style":{"fill":"#ff0000","typography":{"font_size_px":18,"font_weight":700}}},
            {"kind":"rule","family":"xychart","target":"axis-title","style":{"fill":"#00ff00","typography":{"font_size_px":13,"font_weight":600}}},
            {"kind":"rule","family":"xychart","target":"axis-label","style":{"fill":"#0000ff","typography":{"font_size_px":11,"font_weight":500}}},
            {"kind":"rule","family":"xychart","target":"legend","style":{"fill":"#ff00ff","typography":{"font_size_px":12,"font_weight":600}}}
        ]}
    });
    let recipe: merman::svg::ThemeRecipeV1 = serde_json::from_value(recipe).unwrap();
    let saved = serde_json::to_vec(&recipe).unwrap();
    let source =
        "xychart\ntitle TitleProbe\nx-axis Month [A, B]\ny-axis Count 0 --> 10\nbar Sales [4, 7]";
    let mut svgs = Vec::new();
    #[cfg(all(feature = "png", feature = "pdf"))]
    let mut pixels = Vec::new();
    for recipe in [recipe, serde_json::from_slice(&saved).unwrap()] {
        let theme = DiagramThemeCompiler::new().compile_recipe(recipe).unwrap();
        let renderer = Renderer::new();
        let RenderOutput::Svg(Some(output)) = renderer
            .render(
                RenderRequest::svg(source, OperationControl::new(), Default::default())
                    .with_theme(theme.clone()),
            )
            .unwrap()
        else {
            panic!("expected SVG")
        };
        let xml = roxmltree::Document::parse(output.svg()).unwrap();
        for (text, size, weight) in [
            ("TitleProbe", "18", "700"),
            ("Month", "13", "600"),
            ("A", "11", "500"),
            ("Sales", "12", "600"),
        ] {
            let node = xml
                .descendants()
                .find(|node| node.has_tag_name("text") && node.text() == Some(text))
                .unwrap();
            assert_eq!(node.attribute("font-size"), Some(size));
            assert_eq!(node.attribute("font-weight"), Some(weight));
        }
        svgs.push(output.svg().to_owned());
        #[cfg(all(feature = "png", feature = "pdf"))]
        {
            let (width, height, buffer) = native_role_pixels(&renderer, source, &theme);
            for channel in 0..3 {
                assert!(
                    buffer.chunks_exact(4).any(|pixel| {
                        pixel[channel] > 128
                            && pixel[(channel + 1) % 3] < 64
                            && pixel[(channel + 2) % 3] < 64
                            && pixel[3] > 128
                    }),
                    "role text color channel {channel} must be painted"
                );
            }
            pixels.push((width, height, buffer));
        }
    }
    assert_eq!(svgs[0], svgs[1]);
    #[cfg(all(feature = "png", feature = "pdf"))]
    assert_eq!(pixels[0], pixels[1]);
}

#[test]
fn public_cyberpunk_role_fonts_survive_preset_export_and_native_rendering() {
    use merman::svg::ThemePreset;

    let compiler = DiagramThemeCompiler::new();
    let saved =
        serde_json::to_vec(&compiler.export_preset(ThemePreset::Cyberpunk).unwrap()).unwrap();
    let themes = [
        compiler.compile_preset(ThemePreset::Cyberpunk).unwrap(),
        DiagramThemeCompiler::new()
            .compile_recipe(serde_json::from_slice(&saved).unwrap())
            .unwrap(),
    ];
    for orientation in ["", "horizontal"] {
        let source = format!(
            "xychart {orientation}\ntitle TitleProbe\nx-axis Month [A, B]\ny-axis Count 0 --> 10\nbar Sales [4, 7]"
        );
        let mut svgs = Vec::new();
        #[cfg(all(feature = "png", feature = "pdf"))]
        let mut pixels = Vec::new();
        for theme in &themes {
            let renderer = Renderer::new();
            let RenderOutput::Svg(Some(output)) = renderer
                .render(
                    RenderRequest::svg(&source, OperationControl::new(), Default::default())
                        .with_theme(theme.clone()),
                )
                .unwrap()
            else {
                panic!("expected SVG")
            };
            let xml = roxmltree::Document::parse(output.svg()).unwrap();
            for (text, size, weight) in [
                ("TitleProbe", "18", Some("700")),
                ("Month", "13", None),
                ("Count", "13", None),
                ("Sales", "12", None),
            ] {
                let node = xml
                    .descendants()
                    .find(|node| node.has_tag_name("text") && node.text() == Some(text))
                    .unwrap();
                assert_eq!(
                    node.attribute("font-size"),
                    Some(size),
                    "{orientation}/{text}"
                );
                assert_eq!(
                    node.attribute("font-weight"),
                    weight,
                    "{orientation}/{text}"
                );
                assert_eq!(
                    node.attribute("fill"),
                    Some("#00f2ff"),
                    "{orientation}/{text}"
                );
            }
            let tick = xml
                .descendants()
                .find(|node| node.has_tag_name("text") && node.text() == Some("A"))
                .unwrap();
            assert_eq!(
                tick.attribute("font-size"),
                Some("14"),
                "axis-label recipe stays unchanged"
            );
            svgs.push(output.svg().to_owned());
            #[cfg(all(feature = "png", feature = "pdf"))]
            {
                let (width, height, buffer) = native_role_pixels(&renderer, &source, theme);
                assert!(
                    buffer
                        .chunks_exact(4)
                        .any(|pixel| pixel[..3] == [0, 242, 255])
                );
                pixels.push((width, height, buffer));
            }
        }
        assert_eq!(svgs[0], svgs[1], "{orientation}: actual exported preset");
        #[cfg(all(feature = "png", feature = "pdf"))]
        assert_eq!(
            pixels[0], pixels[1],
            "{orientation}: native preset exchange"
        );
    }
}

#[cfg(all(feature = "png", feature = "pdf"))]
fn native_role_pixels(
    renderer: &Renderer,
    source: &str,
    theme: &merman::svg::DiagramTheme,
) -> (u32, u32, Vec<u8>) {
    let RenderOutput::Document(Some(document)) = renderer
        .render(
            RenderRequest::document(source, OperationControl::new(), Default::default())
                .with_theme(theme.clone()),
        )
        .unwrap()
    else {
        panic!("expected document")
    };
    let png = document
        .export_png(&Default::default(), OperationControl::new())
        .unwrap();
    let pdf = document
        .export_pdf(&Default::default(), OperationControl::new())
        .unwrap();
    for admission in [png.admission(), pdf.admission()] {
        assert!(
            !admission
                .reasons()
                .contains(&merman::TargetAdmissionReason::ThemeEvidenceIncomplete),
            "{admission:?}; {:?}",
            document.evidence().theme_diagnostics()
        );
    }
    assert!(pdf.bytes().starts_with(b"%PDF-"));
    let mut reader = png::Decoder::new(std::io::Cursor::new(png.bytes()))
        .read_info()
        .unwrap();
    let mut buffer = vec![0; reader.output_buffer_size().unwrap()];
    let info = reader.next_frame(&mut buffer).unwrap();
    assert_eq!(info.color_type, png::ColorType::Rgba);
    buffer.truncate(info.buffer_size());
    (info.width, info.height, buffer)
}
