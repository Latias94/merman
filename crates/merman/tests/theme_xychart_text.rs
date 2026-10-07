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
fn public_cyberpunk_xy_roles_survive_preset_exchange() {
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
    for [title, x_title, y_title, legend] in [
        ["TitleProbe", "Month", "Count", "Sales"],
        ["请求统计", "月份", "数量", "销售"],
    ] {
        for orientation in ["", "horizontal"] {
            let source = format!(
                "xychart {orientation}\ntitle \"{title}\"\nx-axis \"{x_title}\" [A, B]\ny-axis \"{y_title}\" 0 --> 10\nbar \"{legend}\" [4, 7]"
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
                for (text, size, weight, sigma, alpha) in [
                    (title, "18", Some("700"), "7.5", "0.8"),
                    (x_title, "13", None, "5", "0.6"),
                    (y_title, "13", None, "5", "0.6"),
                    (legend, "12", None, "4", "0.5"),
                    ("A", "14", Some("600"), "5", "0.5"),
                    ("B", "14", Some("600"), "5", "0.5"),
                    ("0", "14", Some("600"), "5", "0.5"),
                    ("10", "14", Some("600"), "5", "0.5"),
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
                    let reference = node
                        .attribute("filter")
                        .expect("public role glow must be bound");
                    let id = reference
                        .strip_prefix("url(#")
                        .unwrap()
                        .strip_suffix(')')
                        .unwrap();
                    let filter = xml
                        .descendants()
                        .find(|n| n.has_tag_name("filter") && n.attribute("id") == Some(id))
                        .unwrap();
                    assert_eq!(
                        filter.attribute("color-interpolation-filters"),
                        Some("sRGB")
                    );
                    let blur = filter
                        .descendants()
                        .find(|n| n.has_tag_name("feGaussianBlur"))
                        .unwrap();
                    assert_eq!(blur.attribute("stdDeviation"), Some(sigma));
                    let flood = filter
                        .descendants()
                        .find(|n| n.has_tag_name("feFlood"))
                        .unwrap();
                    let color = format!("rgba(0, 242, 255, {alpha})");
                    assert_eq!(flood.attribute("flood-color"), Some(color.as_str()));
                }
                let tick = xml
                    .descendants()
                    .find(|node| node.has_tag_name("text") && node.text() == Some("A"))
                    .unwrap();
                assert_eq!(
                    tick.attribute("font-size"),
                    Some("14"),
                    "axis-label size keeps the source default"
                );
                assert!(tick.attribute("filter").is_some());
                assert!(tick.attribute("opacity").is_none());
                let mut tick_count = 0;
                for path in xml.descendants().filter(|n| n.has_tag_name("path")) {
                    if path
                        .ancestors()
                        .any(|n| n.attribute("class") == Some("ticks"))
                    {
                        tick_count += 1;
                        assert_eq!(path.attribute("stroke"), Some("#00f2ff"));
                        assert_eq!(
                            path.attribute("opacity"),
                            Some("0.3"),
                            "public tick opacity"
                        );
                    } else if path
                        .ancestors()
                        .any(|n| matches!(n.attribute("class"), Some("axis-line" | "axisl-line")))
                    {
                        assert!(
                            path.attribute("opacity").is_none(),
                            "axis lines stay opaque"
                        );
                    }
                }
                assert!(tick_count > 0);
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
            matches!(
                admission.status(),
                merman::TargetAdmissionStatus::Portable
                    | merman::TargetAdmissionStatus::HostDependent
            ),
            "unexpected native admission: {admission:?}; source: {source}"
        );
        for reason in [
            merman::TargetAdmissionReason::ThemeEvidenceIncomplete,
            merman::TargetAdmissionReason::NativeFilterReceiptMismatch,
            merman::TargetAdmissionReason::PdfNativeFilterNotLocalized,
        ] {
            assert!(
                !admission.reasons().contains(&reason),
                "{admission:?}; {:?}",
                document.evidence().theme_diagnostics()
            );
        }
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

#[cfg(all(feature = "png", feature = "pdf"))]
fn xy_role_glow_theme() -> merman::svg::DiagramTheme {
    let recipe: merman::svg::ThemeRecipeV1 = serde_json::from_value(serde_json::json!({
        "schema_version": 1, "kind": "complete_spec", "complete_spec": {
            "effects": [
                {"kind":"graph", "id":"text-glow", "color_space":"srgb", "primitives":[
                    {"kind":"drop-shadow", "input":"source-graphic", "offset_x":0, "offset_y":0,
                     "blur_radius":7.5, "spread":0, "color":"#00f2ff"}
                ]},
                {"kind":"binding", "target":"title", "effect_id":"text-glow"},
                {"kind":"binding", "target":"axis-title", "effect_id":"text-glow"},
                {"kind":"binding", "target":"legend", "effect_id":"text-glow"}
            ]
        }
    }))
    .unwrap();
    DiagramThemeCompiler::new().compile_recipe(recipe).unwrap()
}

#[cfg(all(feature = "png", feature = "pdf"))]
#[test]
fn xy_role_glow_reaches_native_pixels_and_complete_filter_receipts() {
    use merman::TargetAdmissionReason;
    let theme = xy_role_glow_theme();
    for orientation in ["", " horizontal"] {
        let source = format!(
            "xychart{orientation}\ntitle TitleProbe\nx-axis Month [A, B]\ny-axis Count 0 --> 10\nbar Sales [4, 7]"
        );
        let RenderOutput::Document(Some(document)) = Renderer::new()
            .render(
                RenderRequest::document(&source, OperationControl::new(), Default::default())
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
            for reason in [
                TargetAdmissionReason::ThemeEvidenceIncomplete,
                TargetAdmissionReason::NativeFilterReceiptMismatch,
                TargetAdmissionReason::PdfNativeFilterNotLocalized,
            ] {
                assert!(
                    !admission.reasons().contains(&reason),
                    "{orientation}: {admission:?}"
                );
            }
        }
        let mut reader = png::Decoder::new(std::io::Cursor::new(png.bytes()))
            .read_info()
            .unwrap();
        let mut buffer = vec![0; reader.output_buffer_size().unwrap()];
        let info = reader.next_frame(&mut buffer).unwrap();
        assert_eq!(info.color_type, png::ColorType::Rgba);
        assert!(
            buffer[..info.buffer_size()]
                .chunks_exact(4)
                .any(|p| p[1] > p[0].saturating_add(10) && p[2] > p[0].saturating_add(10)),
            "text shadow must paint cyan pixels"
        );
    }
}

#[cfg(all(feature = "png", feature = "pdf"))]
#[test]
fn xy_role_glow_paint_reserve_does_not_certify_underestimated_host_metrics() {
    use merman::svg::{
        MeasurementProfileId, TextMeasurementPolicy, TextMeasurementProfile,
        TextMeasurementProfileIdentity, TextMeasurer, TextMetrics, TextStyle,
    };
    #[derive(Debug)]
    struct Underestimated;
    impl TextMeasurer for Underestimated {
        fn measure(&self, _: &str, style: &TextStyle) -> TextMetrics {
            TextMetrics {
                width: 0.01,
                height: style.font_size,
                line_count: 1,
            }
        }
    }
    let environment = merman::SvgEnvironment::deterministic().with_text_measurement_policy(
        TextMeasurementPolicy::uniform(TextMeasurementProfile::new(
            TextMeasurementProfileIdentity::new(
                MeasurementProfileId::new("test.underestimated-text").unwrap(),
                "1",
            )
            .unwrap(),
            std::sync::Arc::new(Underestimated),
        )),
    );
    let source = format!(
        "xychart\ntitle {}\nx-axis [A, B]\ny-axis 0 --> 10\nbar [4, 7]",
        "W".repeat(128)
    );
    let RenderOutput::Document(Some(document)) = Renderer::new()
        .render(
            RenderRequest::document(
                &source,
                OperationControl::new(),
                merman::SvgRequest {
                    environment,
                    ..Default::default()
                },
            )
            .with_theme(xy_role_glow_theme()),
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
            admission
                .reasons()
                .contains(&merman::TargetAdmissionReason::NativeFilterReceiptMismatch),
            "finite allocation must not certify overflowing glyph ink: {admission:?}"
        );
    }
}

#[cfg(all(feature = "png", feature = "pdf"))]
#[test]
fn complete_cyberpunk_xy_scene_fits_default_native_filter_budget() {
    use merman::svg::{
        ThemePreset,
        export::{PdfOptions, RasterOptions, SvgConversionLimits},
    };
    let theme = DiagramThemeCompiler::new()
        .compile_preset(ThemePreset::Cyberpunk)
        .unwrap();
    let source = include_str!("../../merman-theme-fixtures/fixtures/public-cyberpunk/xychart.mmd");
    let RenderOutput::Document(Some(document)) = Renderer::new()
        .render(
            RenderRequest::document(source, OperationControl::new(), Default::default())
                .with_theme(theme),
        )
        .unwrap()
    else {
        panic!("expected document")
    };
    let png = document
        .export_png(&RasterOptions::default(), OperationControl::new())
        .unwrap();
    let pdf = document
        .export_pdf(&PdfOptions::default(), OperationControl::new())
        .unwrap();
    // Eight bars, two lines, fifteen axis labels and three titles each retain one shadow.
    let expected_filters = 8 + 2 + 15 + 3;
    assert_eq!(
        png.export_report().conversion().filtered_groups,
        expected_filters
    );
    assert_eq!(
        png.export_report().conversion().filter_primitives,
        expected_filters * 4
    );
    assert_eq!(
        png.export_report().conversion(),
        pdf.export_report().conversion()
    );
    assert_eq!(
        png.export_report().native_filter_receipt(),
        pdf.export_report().native_filter_receipt()
    );
    let native_filter_receipt = png.export_report().native_filter_receipt().unwrap_or_else(|| {
        panic!(
            "complete XY scene must retain native filter evidence: admission={:?}; fonts={:?}; source={source}",
            png.admission(),
            png.export_report().fonts(),
        )
    });
    assert_eq!(
        native_filter_receipt.drop_shadow_count(),
        expected_filters as u32
    );
    for admission in [png.admission(), pdf.admission()] {
        assert!(
            admission
                .reasons()
                .iter()
                .all(|reason| *reason == merman::TargetAdmissionReason::SystemOrHostFontDependency),
            "{admission:?}"
        );
    }
    let limits = SvgConversionLimits {
        max_total_filter_primitives: Some(expected_filters * 4 - 1),
        ..Default::default()
    };
    let error = document
        .export_png(
            &RasterOptions::default().with_conversion_limits(limits),
            OperationControl::new(),
        )
        .err()
        .expect("exact lower limit remains enforced");
    assert!(
        error
            .to_string()
            .contains("max_total_svg_conversion_filter_primitives"),
        "{error}"
    );
}
