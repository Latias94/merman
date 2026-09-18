//! Series styling must reach actual plot paint through the public renderer.

#![cfg(feature = "svg")]

use merman::svg::{
    CanvasPaint, CanvasSpec, DiagramTheme, DiagramThemeCompiler, DiagramThemeSpec, OrdinalSelector,
    Specified, ThemeRule, ThemeRuleSet, ThemeStylePatch, ThemeTarget,
};
use merman::{OperationControl, RenderOutput, RenderRequest, Renderer};

fn series_theme(fill_opacity: f32, width: f32) -> DiagramTheme {
    let mut bar = ThemeStylePatch::default()
        .with_fill(CanvasPaint::solid("#ff0000").unwrap())
        .with_stroke(CanvasPaint::solid("#00ff00").unwrap())
        .with_stroke_width(width)
        .unwrap();
    bar.paint.fill_opacity = Specified::Value(fill_opacity);
    let line = ThemeStylePatch::default()
        .with_stroke(CanvasPaint::solid("#0000ff").unwrap())
        .with_stroke_width(5.0)
        .unwrap();
    DiagramThemeCompiler::new()
        .compile(
            DiagramThemeSpec::new()
                .with_canvas(
                    CanvasSpec::default().with_base(CanvasPaint::solid("#000000").unwrap()),
                )
                .with_styles(
                    ThemeRuleSet::default()
                        .with_rule(
                            ThemeRule::new(ThemeTarget::ChartSeries, bar)
                                .with_ordinal(OrdinalSelector::Exact(1)),
                        )
                        .with_rule(
                            ThemeRule::new(ThemeTarget::ChartSeries, line)
                                .with_ordinal(OrdinalSelector::Exact(2)),
                        ),
                ),
        )
        .unwrap()
}

const SOURCE: &str = "xychart\nx-axis [A, B]\ny-axis 0 --> 10\nbar [4, 7]\nline [2, 3]";

fn series_svg(theme: DiagramTheme) -> String {
    let RenderOutput::Svg(Some(output)) = Renderer::new()
        .render(
            RenderRequest::svg(SOURCE, OperationControl::new(), Default::default())
                .with_theme(theme),
        )
        .unwrap()
    else {
        panic!("expected SVG")
    };
    output.svg().to_owned()
}

#[test]
fn public_xy_series_rules_keep_fill_alpha_separate_from_stroke() {
    let svg = series_svg(series_theme(0.25, 4.0));
    let document = roxmltree::Document::parse(&svg).unwrap();
    let plot = |class: &str| {
        document
            .descendants()
            .find(|n| n.attribute("class") == Some(class))
            .unwrap()
    };
    let bars: Vec<_> = plot("bar-plot-0")
        .children()
        .filter(|n| n.has_tag_name("rect"))
        .collect();
    assert_eq!(bars.len(), 2);
    for bar in bars {
        assert_eq!(bar.attribute("fill"), Some("#ff0000"));
        assert_eq!(bar.attribute("stroke"), Some("#00ff00"));
        assert_eq!(bar.attribute("stroke-width"), Some("4"));
        assert_eq!(bar.attribute("fill-opacity"), Some("0.25"));
        assert!(matches!(bar.attribute("opacity"), None | Some("1")));
        assert!(matches!(bar.attribute("stroke-opacity"), None | Some("1")));
    }
    let line = plot("line-plot-1")
        .children()
        .find(|n| n.has_tag_name("path"))
        .unwrap();
    assert_eq!(line.attribute("stroke"), Some("#0000ff"));
    assert_eq!(line.attribute("stroke-width"), Some("5"));
    assert_eq!(line.attribute("fill"), Some("none"));
}

#[cfg(feature = "png")]
fn series_pixels(fill_opacity: f32, stroke_width: f32) -> (u32, u32, Vec<u8>) {
    let RenderOutput::Document(Some(document)) = Renderer::new()
        .render(
            RenderRequest::document(SOURCE, OperationControl::new(), Default::default())
                .with_theme(series_theme(fill_opacity, stroke_width)),
        )
        .unwrap()
    else {
        panic!("expected document")
    };
    let output = document
        .export_png(
            &merman::svg::export::RasterOptions::default().with_scale(2.0),
            OperationControl::new(),
        )
        .unwrap();
    assert!(
        !output
            .admission()
            .reasons()
            .contains(&merman::TargetAdmissionReason::ThemeEvidenceIncomplete),
        "all requested series facets must have final terminal evidence"
    );
    let mut reader = png::Decoder::new(std::io::Cursor::new(output.bytes()))
        .read_info()
        .unwrap();
    let mut pixels = vec![0; reader.output_buffer_size().unwrap()];
    let info = reader.next_frame(&mut pixels).unwrap();
    assert_eq!(info.color_type, png::ColorType::Rgba);
    pixels.truncate(info.buffer_size());
    (info.width, info.height, pixels)
}

#[cfg(feature = "png")]
#[test]
fn native_xy_series_fill_alpha_does_not_dim_the_border_or_other_series() {
    let (width, height, translucent) = series_pixels(0.25, 4.0);
    let (opaque_width, opaque_height, opaque) = series_pixels(1.0, 4.0);
    let (borderless_width, borderless_height, borderless) = series_pixels(0.25, 0.0);
    assert_eq!((width, height), (opaque_width, opaque_height));
    assert_eq!((width, height), (borderless_width, borderless_height));
    let count = |pixels: &[u8], rgb: [u8; 3]| {
        pixels
            .chunks_exact(4)
            .filter(|p| p[..3] == rgb && p[3] == 255)
            .count()
    };
    assert!(
        count(&translucent, [64, 0, 0]) > 100,
        "bar interiors must composite quarter-alpha red over black"
    );
    assert!(
        count(&opaque, [255, 0, 0]) > 100,
        "opaque control must retain red interiors"
    );
    let green = count(&translucent, [0, 255, 0]);
    assert!(green > 100, "bar borders must remain opaque green");
    assert_eq!(green, count(&opaque, [0, 255, 0]));
    assert_eq!(
        count(&borderless, [0, 255, 0]),
        0,
        "zero width must suppress the border"
    );
    let blue = count(&translucent, [0, 0, 255]);
    assert!(blue > 100, "the independently styled line must be visible");
    assert_eq!(blue, count(&opaque, [0, 0, 255]));
    assert_eq!(blue, count(&borderless, [0, 0, 255]));
}

#[test]
fn saved_complete_recipe_reproduces_series_paint_in_a_fresh_renderer() {
    let saved = serde_json::to_vec(&serde_json::json!({
        "schema_version": 1,
        "kind": "complete_spec",
        "complete_spec": {
            "canvas": {"base": "#000000"},
            "styles": [
                {"kind": "rule", "target": "chart-series", "ordinal": {"exact": 1},
                 "style": {"fill": "#ff0000", "fill_opacity": 0.25,
                           "stroke": {"paint": "#00ff00", "width": 4.0}}},
                {"kind": "rule", "target": "chart-series", "ordinal": {"exact": 2},
                 "style": {"stroke": {"paint": "#0000ff", "width": 5.0}}}
            ]
        }
    }))
    .unwrap();
    let recipe = serde_json::from_slice(&saved).unwrap();
    let imported = DiagramThemeCompiler::new().compile_recipe(recipe).unwrap();
    assert_eq!(series_svg(imported), series_svg(series_theme(0.25, 4.0)));
}

#[test]
fn imported_xy_series_effects_follow_winners_without_filtering_labels() {
    let saved = serde_json::to_vec(&serde_json::json!({
        "schema_version": 1,
        "kind": "complete_spec",
        "complete_spec": {
            "effects": [
                {"kind": "graph", "id": "glow", "color_space": "srgb", "primitives": [
                    {"kind": "drop-shadow", "input": "source-graphic", "offset_x": 0,
                     "offset_y": 0, "blur_radius": 3, "spread": 0, "color": "#00f2ff"},
                    {"kind": "drop-shadow", "input": "previous", "offset_x": 0,
                     "offset_y": 0, "blur_radius": 6, "spread": 0, "color": "#ff00ff"}
                ]},
                {"kind": "binding", "target": "chart-series", "effect_id": "glow"}
            ],
            "styles": [
                {"kind": "rule", "target": "chart-series", "ordinal": {"exact": 2},
                 "style": {"effect": null}}
            ]
        }
    }))
    .unwrap();
    for orientation in ["", " horizontal"] {
        let recipe = serde_json::from_slice(&saved).unwrap();
        let theme = DiagramThemeCompiler::new().compile_recipe(recipe).unwrap();
        let source = format!(
            "xychart{orientation}\nx-axis [A, B]\ny-axis 0 --> 10\nbar Bars [4, 6]\nline Trend [7 \"Left\", 7 \"Right\"]"
        );
        let RenderOutput::Svg(Some(output)) = Renderer::new()
            .render(
                RenderRequest::svg(&source, OperationControl::new(), Default::default())
                    .with_theme(theme),
            )
            .unwrap()
        else {
            panic!("expected SVG")
        };
        let document = roxmltree::Document::parse(output.svg()).unwrap();
        let filtered: Vec<_> = document
            .descendants()
            .filter(|node| node.attribute("filter").is_some())
            .collect();
        assert_eq!(
            filtered.len(),
            3,
            "two bars and their visible legend marker"
        );
        for node in filtered {
            assert!(node.has_tag_name("rect"));
            let id = node
                .attribute("filter")
                .unwrap()
                .strip_prefix("url(#")
                .unwrap()
                .strip_suffix(')')
                .unwrap();
            let definition = document
                .descendants()
                .find(|candidate| {
                    candidate.has_tag_name("filter") && candidate.attribute("id") == Some(id)
                })
                .unwrap();
            assert_eq!(
                definition.attribute("color-interpolation-filters"),
                Some("sRGB")
            );
            assert_eq!(
                definition
                    .children()
                    .filter(|child| child.has_tag_name("feGaussianBlur"))
                    .count(),
                2
            );
        }
        for text in document
            .descendants()
            .filter(|node| node.has_tag_name("text"))
        {
            assert!(
                text.ancestors()
                    .all(|node| node.attribute("filter").is_none()),
                "geometry effect must not filter any label"
            );
        }
    }
}

#[test]
fn public_cyberpunk_series_recipe_tracks_kind_and_cycles_after_three_series() {
    use merman::svg::ThemePreset;
    let compiler = DiagramThemeCompiler::new();
    let exported =
        serde_json::to_vec(&compiler.export_preset(ThemePreset::Cyberpunk).unwrap()).unwrap();
    let selected = compiler.compile_preset(ThemePreset::Cyberpunk).unwrap();
    let imported = DiagramThemeCompiler::new()
        .compile_recipe(serde_json::from_slice(&exported).unwrap())
        .unwrap();
    let source =
        "xychart\nx-axis [A, B]\ny-axis 0 --> 10\nline [7, 8]\nbar [4, 6]\nbar [2, 3]\nline [1, 2]";
    let mut outputs = Vec::new();
    for theme in [selected, imported] {
        let RenderOutput::Svg(Some(output)) = Renderer::new()
            .render(
                RenderRequest::svg(source, OperationControl::new(), Default::default())
                    .with_theme(theme),
            )
            .unwrap()
        else {
            panic!("expected SVG")
        };
        let document = roxmltree::Document::parse(output.svg()).unwrap();
        for (index, kind, color) in [
            (0, "line", "#6cc6cb"),
            (1, "bar", "#c77dff"),
            (2, "bar", "#7ce38b"),
            (3, "line", "#6cc6cb"),
        ] {
            let class = format!("{kind}-plot-{index}");
            let group = document
                .descendants()
                .find(|n| n.attribute("class") == Some(class.as_str()))
                .unwrap();
            let marks: Vec<_> = group
                .children()
                .filter(|n| n.has_tag_name(if kind == "bar" { "rect" } else { "path" }))
                .collect();
            assert_eq!(marks.len(), if kind == "bar" { 2 } else { 1 });
            for mark in marks {
                assert_eq!(mark.attribute("stroke"), Some(color));
                assert_eq!(
                    mark.attribute("stroke-width"),
                    Some(if kind == "bar" { "2" } else { "3" })
                );
                assert_eq!(
                    mark.attribute("fill-opacity"),
                    if kind == "bar" { Some("0.2") } else { None }
                );
                assert_eq!(
                    mark.attribute("fill"),
                    Some(if kind == "bar" { color } else { "none" })
                );
                let id = mark
                    .attribute("filter")
                    .expect("every actual series mark has its glow")
                    .strip_prefix("url(#")
                    .unwrap()
                    .strip_suffix(')')
                    .unwrap();
                let filter = document
                    .descendants()
                    .find(|n| n.attribute("id") == Some(id))
                    .unwrap();
                assert_eq!(
                    filter.attribute("color-interpolation-filters"),
                    Some("sRGB")
                );
                let blur = filter
                    .children()
                    .find(|n| n.has_tag_name("feGaussianBlur"))
                    .unwrap();
                assert_eq!(
                    blur.attribute("stdDeviation"),
                    Some(if kind == "bar" { "8" } else { "6" })
                );
            }
        }
        outputs.push(output.svg().to_owned());
    }
    assert_eq!(outputs[0], outputs[1], "saved recipe is self-contained");
}

#[cfg(all(feature = "png", feature = "pdf"))]
#[test]
fn public_cyberpunk_series_recipe_preserves_native_receipts_after_exchange() {
    use merman::TargetAdmissionReason;
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
    let source =
        "xychart\nx-axis [A, B]\ny-axis 0 --> 10\nline [7, 8]\nbar [4, 6]\nbar [2, 3]\nline [1, 2]";
    let mut pixels = Vec::new();
    for theme in themes {
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
            .export_png(
                &merman::svg::export::RasterOptions::default(),
                OperationControl::new(),
            )
            .unwrap();
        let pdf = document
            .export_pdf(
                &merman::svg::export::PdfOptions::default(),
                OperationControl::new(),
            )
            .unwrap();
        for admission in [png.admission(), pdf.admission()] {
            for reason in [
                TargetAdmissionReason::ThemeEvidenceIncomplete,
                TargetAdmissionReason::NativeFilterReceiptMismatch,
                TargetAdmissionReason::PdfNativeFilterNotLocalized,
            ] {
                assert!(
                    !admission.reasons().contains(&reason),
                    "{admission:?}; diagnostics: {:?}; evidence: {:?}",
                    document.evidence().theme_diagnostics(),
                    document.evidence().theme_evidence(),
                );
            }
        }
        let receipt = png.export_report().native_filter_receipt().unwrap();
        // Four bars and two lines each consume one single-stage graph.
        assert_eq!(receipt.filter_count(), 6);
        assert_eq!(receipt.drop_shadow_count(), 6);
        assert_eq!(receipt.reference_count(), 6);
        assert_eq!(pdf.export_report().native_filter_receipt(), Some(receipt));
        assert!(pdf.bytes().starts_with(b"%PDF-"));
        let mut reader = png::Decoder::new(std::io::Cursor::new(png.bytes()))
            .read_info()
            .unwrap();
        let mut buffer = vec![0; reader.output_buffer_size().unwrap()];
        let info = reader.next_frame(&mut buffer).unwrap();
        assert_eq!(info.color_type, png::ColorType::Rgba);
        buffer.truncate(info.buffer_size());
        assert!(
            buffer
                .chunks_exact(4)
                .any(|pixel| pixel[..3] == [108, 198, 203])
        );
        pixels.push((info.width, info.height, buffer));
    }
    assert_eq!(pixels[0], pixels[1], "saved recipe retains native output");
}
