//! Native export must retain bounded Flowchart edge effects and their application evidence.

#![cfg(all(feature = "svg", feature = "png"))]

use merman::svg::{
    CanvasPaint, DiagramEffectSet, DiagramThemeCompiler, DiagramThemeSpec, EffectBinding,
    EffectColorSpace, EffectGraph, EffectInput, EffectPrimitive, ThemeColorValue, ThemeRule,
    ThemeRuleSet, ThemeStylePatch, ThemeTarget,
};
use merman::{OperationControl, RenderOutput, RenderRequest, Renderer};

#[test]
fn edge_glow_survives_native_export_for_line_paths_and_nested_roots() {
    for source in [
        "flowchart LR\nA[Alpha] --> B[Beta]",
        "flowchart TB\nA[Alpha] --> B[Beta]",
        "flowchart LR\nsubgraph Outer\nsubgraph Inner\nA[Alpha] --> B[Beta]\nend\nend",
        "flowchart LR\nA[Alpha] o--o B[Beta] x--x C[Gamma]",
    ] {
        let source = format!("---\nconfig:\n  htmlLabels: false\n---\n{source}");
        let theme = DiagramThemeCompiler::new()
            .compile(
                DiagramThemeSpec::new()
                    .with_effects(
                        DiagramEffectSet::default()
                            .with_graph(
                                EffectGraph::new(
                                    "edge-glow",
                                    [EffectPrimitive::DropShadow {
                                        input: EffectInput::SourceGraphic,
                                        offset_x: 0.0,
                                        offset_y: 0.0,
                                        blur_radius: 6.0,
                                        spread: 0.0,
                                        color: ThemeColorValue::parse("#00f2ff").unwrap(),
                                    }],
                                )
                                .unwrap()
                                .with_color_space(EffectColorSpace::Srgb),
                            )
                            .unwrap()
                            .with_binding(
                                EffectBinding::new(ThemeTarget::Edge, "edge-glow").unwrap(),
                            )
                            .unwrap(),
                    )
                    .with_styles(
                        ThemeRuleSet::default()
                            .with_rule(ThemeRule::new(
                                ThemeTarget::Node,
                                ThemeStylePatch::default()
                                    .with_fill(CanvasPaint::solid("#ffffff").unwrap())
                                    .with_stroke(CanvasPaint::solid("#000000").unwrap()),
                            ))
                            .with_rule(ThemeRule::new(
                                ThemeTarget::Edge,
                                ThemeStylePatch::default()
                                    .with_stroke(CanvasPaint::solid("#000000").unwrap()),
                            )),
                    ),
            )
            .unwrap();
        let RenderOutput::Document(Some(document)) = Renderer::new()
            .render(
                RenderRequest::document(&source, OperationControl::new(), Default::default())
                    .with_theme(theme),
            )
            .unwrap()
        else {
            panic!("document required")
        };
        let output = document
            .export_png(
                &merman::svg::export::RasterOptions::default(),
                OperationControl::new(),
            )
            .unwrap();
        assert!(
            !output
                .admission()
                .reasons()
                .contains(&merman::TargetAdmissionReason::ThemeEvidenceIncomplete)
        );
        let mut reader = png::Decoder::new(std::io::Cursor::new(output.bytes()))
            .read_info()
            .unwrap();
        let mut pixels = vec![0; reader.output_buffer_size().unwrap()];
        let info = reader.next_frame(&mut pixels).unwrap();
        assert_eq!(info.color_type, png::ColorType::Rgba);
        // All source paint is achromatic. Cyan identifies the shadow without requiring
        // an arbitrary contrast from a thin blurred path composited over white.
        assert!(
            pixels[..info.buffer_size()]
                .chunks_exact(4)
                .any(|pixel| pixel[3] > 0 && pixel[1] > pixel[0] && pixel[2] > pixel[0]),
            "cyan glow must reach native pixels: {source}"
        );
        #[cfg(feature = "pdf")]
        {
            let pdf = document
                .export_pdf(
                    &merman::svg::export::PdfOptions::default(),
                    OperationControl::new(),
                )
                .unwrap();
            assert!(
                !pdf.admission()
                    .reasons()
                    .contains(&merman::TargetAdmissionReason::ThemeEvidenceIncomplete)
            );
            assert!(pdf.bytes().starts_with(b"%PDF"));
        }
    }
}

#[test]
fn text_only_glow_survives_native_png_and_pdf() {
    use merman::svg::{
        FontAssetSpec, FontCatalogSpec, FontStack, ThemeAssets, ThemeTextStyle, TypographySpec,
    };
    for (native_fonts, html_labels) in [(false, false), (true, false), (false, true)] {
        if native_fonts && !cfg!(feature = "embedded-fonts") {
            continue;
        }
        let mut spec = DiagramThemeSpec::new();
        if native_fonts {
            spec = spec.with_typography(TypographySpec::default().with_family_style(
                merman::DiagramFamilyId::FLOWCHART,
                ThemeTextStyle::default().with_font_stack(FontStack::single("Excalifont").unwrap()),
            )).with_assets(ThemeAssets::default().with_font_catalog(FontCatalogSpec::new([
                FontAssetSpec::new("excalifont", include_bytes!("../../../fixtures/themes/assets/fonts/Excalifont-Regular-Latin.woff2")),
            ])));
        }
        let effects = DiagramEffectSet::default()
            .with_graph(
                EffectGraph::new(
                    "text-glow",
                    [EffectPrimitive::DropShadow {
                        input: EffectInput::SourceGraphic,
                        offset_x: 0.0,
                        offset_y: 0.0,
                        blur_radius: 6.0,
                        spread: 0.0,
                        color: ThemeColorValue::parse("#ff2080").unwrap(),
                    }],
                )
                .unwrap()
                .with_color_space(EffectColorSpace::Srgb),
            )
            .unwrap()
            .with_binding(EffectBinding::new(ThemeTarget::NodeLabel, "text-glow").unwrap())
            .unwrap();
        let effects = effects
            .with_binding(EffectBinding::new(ThemeTarget::EdgeLabel, "text-glow").unwrap())
            .unwrap();
        let theme = DiagramThemeCompiler::new()
            .compile(
                spec.with_effects(effects).with_styles(
                    ThemeRuleSet::default()
                        .with_rule(ThemeRule::new(
                            ThemeTarget::Node,
                            ThemeStylePatch::default()
                                .with_fill(CanvasPaint::solid("#ffffff").unwrap())
                                .with_stroke(CanvasPaint::solid("#000000").unwrap()),
                        ))
                        .with_rule(ThemeRule::new(
                            ThemeTarget::Edge,
                            ThemeStylePatch::default()
                                .with_stroke(CanvasPaint::solid("#000000").unwrap()),
                        )),
                ),
            )
            .unwrap();
        for body in [
            "flowchart LR\nA[Alpha] -->|Advance| B[Beta]",
            "flowchart TB\nsubgraph Outer\nA[Alpha<br/>Gyp] -->|Advance<br/>Next| B[Beta]\nend",
            "flowchart TB\nsubgraph Outer\nA[Alpha<br/><br/>Gyp] -->|Advance<br/>Next| B[Beta]\nend",
        ] {
            let source = format!("---\nconfig:\n  htmlLabels: {html_labels}\n---\n{body}");
            let RenderOutput::Document(Some(document)) = Renderer::new()
                .render(
                    RenderRequest::document(&source, OperationControl::new(), Default::default())
                        .with_theme(theme.clone()),
                )
                .unwrap_or_else(|error| {
                    panic!("native_fonts={native_fonts}, html_labels={html_labels}, body={body}: {error:?}")
                })
            else {
                panic!("document required")
            };
            let png = document
                .export_png(&Default::default(), OperationControl::new())
                .unwrap();
            let check = |admission: &merman::TargetAdmissionReceipt| {
                for reason in [
                    merman::TargetAdmissionReason::ThemeEvidenceIncomplete,
                    merman::TargetAdmissionReason::NativeFilterReceiptMismatch,
                    merman::TargetAdmissionReason::PdfNativeFilterNotLocalized,
                ] {
                    assert!(
                        !admission.reasons().contains(&reason),
                        "native_fonts={native_fonts}, html_labels={html_labels}: {admission:?}"
                    );
                }
            };
            check(png.admission());
            assert_eq!(
                png.export_report()
                    .native_filter_receipt()
                    .unwrap()
                    .filter_count(),
                3
            );
            let mut reader = png::Decoder::new(std::io::Cursor::new(png.bytes()))
                .read_info()
                .unwrap();
            let mut pixels = vec![0; reader.output_buffer_size().unwrap()];
            let info = reader.next_frame(&mut pixels).unwrap();
            assert_eq!(info.color_type, png::ColorType::Rgba);
            // Only the text shadow has chromatic paint; definitions alone cannot satisfy this.
            assert!(
                pixels[..info.buffer_size()]
                    .chunks_exact(4)
                    .any(|p| p[3] > 0 && p[0] > p[2] && p[2] > p[1]),
                "native_fonts={native_fonts}, html_labels={html_labels}: text glow must reach PNG pixels"
            );
            #[cfg(feature = "pdf")]
            {
                let pdf = document
                    .export_pdf(&Default::default(), OperationControl::new())
                    .unwrap();
                check(pdf.admission());
                assert_eq!(
                    pdf.export_report().native_filter_receipt(),
                    png.export_report().native_filter_receipt()
                );
                assert!(pdf.bytes().starts_with(b"%PDF"));
            }
        }
    }
}

#[test]
fn custom_host_metrics_without_prepared_geometry_retain_a_residual() {
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
                MeasurementProfileId::new("test.flowchart-small-host").unwrap(),
                "1",
            )
            .unwrap(),
            std::sync::Arc::new(Underestimated),
        )),
    );
    let theme = DiagramThemeCompiler::new()
        .compile(
            DiagramThemeSpec::new().with_effects(
                DiagramEffectSet::default()
                    .with_graph(
                        EffectGraph::new(
                            "text",
                            [EffectPrimitive::DropShadow {
                                input: EffectInput::SourceGraphic,
                                offset_x: 0.0,
                                offset_y: 0.0,
                                blur_radius: 4.0,
                                spread: 0.0,
                                color: ThemeColorValue::parse("#ff2080").unwrap(),
                            }],
                        )
                        .unwrap(),
                    )
                    .unwrap()
                    .with_binding(EffectBinding::new(ThemeTarget::NodeLabel, "text").unwrap())
                    .unwrap(),
            ),
        )
        .unwrap();
    for html_labels in [false, true] {
        let source = format!(
            "---\nconfig:\n  htmlLabels: {html_labels}\n---\nflowchart LR\nA[{}]",
            "W".repeat(128)
        );
        let RenderOutput::Document(Some(document)) = Renderer::new()
            .render(
                RenderRequest::document(
                    &source,
                    OperationControl::new(),
                    merman::SvgRequest {
                        environment: environment.clone(),
                        ..Default::default()
                    },
                )
                .with_theme(theme.clone()),
            )
            .unwrap()
        else {
            panic!("document required")
        };
        // Custom measurers do not expose the built-in operation binding needed by the
        // prepared label sidecar. An arbitrary metrics result cannot certify an effect.
        assert!(!document.svg().contains("theme-effect-label"));
        let png = document
            .export_png(&Default::default(), OperationControl::new())
            .unwrap();
        assert!(
            png.admission()
                .reasons()
                .contains(&merman::TargetAdmissionReason::ThemeEvidenceIncomplete),
            "unprepared host metrics must retain incomplete theme evidence: {:?}",
            png.admission()
        );
        #[cfg(feature = "pdf")]
        {
            let pdf = document
                .export_pdf(&Default::default(), OperationControl::new())
                .unwrap();
            assert!(
                pdf.admission()
                    .reasons()
                    .contains(&merman::TargetAdmissionReason::ThemeEvidenceIncomplete)
            );
        }
    }
}
