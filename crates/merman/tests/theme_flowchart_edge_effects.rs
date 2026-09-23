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
    for html_labels in [false, true] {
        let spec = DiagramThemeSpec::new();
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
                    panic!("html_labels={html_labels}, body={body}: {error:?}")
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
                        "html_labels={html_labels}: {admission:?}"
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
                "html_labels={html_labels}: text glow must reach PNG pixels"
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
fn underestimated_host_label_geometry_is_rejected_by_native_export() {
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
        // Host allocation may draw the requested effect without becoming a cache identity.
        // The actual native glyph observer must still reject underestimated paint bounds.
        assert!(document.svg().contains("theme-effect-label"));
        let png = document
            .export_png(&Default::default(), OperationControl::new())
            .unwrap();
        assert!(
            png.admission()
                .reasons()
                .contains(&merman::TargetAdmissionReason::NativeFilterReceiptMismatch),
            "underestimated host metrics must fail final glyph containment: {:?}",
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
                    .contains(&merman::TargetAdmissionReason::NativeFilterReceiptMismatch)
            );
        }
    }
}

#[test]
fn typed_edge_background_preserves_alpha_in_native_pixels() {
    for swimlane in [false, true] {
        for html in [false, true] {
            for padding in [0.0, 6.0] {
                let source = format!(
                    "---\nconfig:\n  htmlLabels: {html}\n  themeVariables:\n    clusterBkg: transparent\n{}---\nflowchart LR\nA[Alpha] -->|Advance| B[Beta]",
                    if swimlane { "  layout: swimlane\n" } else { "" },
                );
                let theme = DiagramThemeCompiler::new()
                    .compile(
                        DiagramThemeSpec::new()
                            .with_canvas(merman::svg::CanvasSpec::transparent())
                            .with_styles(
                                ThemeRuleSet::default()
                                    .with_rule(ThemeRule::new(
                                        ThemeTarget::EdgeLabelBackground,
                                        ThemeStylePatch::default().with_fill(
                                            CanvasPaint::solid("rgba(255,0,0,0.4)").unwrap(),
                                        ),
                                    ))
                                    .with_rule(ThemeRule::new(
                                        ThemeTarget::EdgeLabel,
                                        ThemeStylePatch::default()
                                            .with_padding(merman::svg::InsetsPx::all(padding)),
                                    )),
                            ),
                    )
                    .unwrap();
                let RenderOutput::Document(Some(document)) = Renderer::new()
                    .render(
                        RenderRequest::document(
                            &source,
                            OperationControl::new(),
                            Default::default(),
                        )
                        .with_theme(theme),
                    )
                    .unwrap()
                else {
                    panic!("document required")
                };
                let output = document
                    .export_png(&Default::default(), OperationControl::new())
                    .unwrap();
                assert_eq!(
                    output
                        .admission()
                        .reasons()
                        .contains(&merman::TargetAdmissionReason::ThemeEvidenceIncomplete),
                    !swimlane && html && padding > 0.0,
                    "only the unmeasured padded HTML background retains a residual",
                );
                let mut reader = png::Decoder::new(std::io::Cursor::new(output.bytes()))
                    .read_info()
                    .unwrap();
                let mut pixels = vec![0; reader.output_buffer_size().unwrap()];
                let info = reader.next_frame(&mut pixels).unwrap();
                assert_eq!(info.color_type, png::ColorType::Rgba);
                // Only the requested background is red. Its interior must retain the
                // requested alpha, independent of HTML projection or a padding rectangle.
                let max_alpha = pixels[..info.buffer_size()]
                    .chunks_exact(4)
                    .filter(|pixel| pixel[0] >= 250 && pixel[1] <= 2 && pixel[2] <= 2)
                    .map(|pixel| pixel[3])
                    .max()
                    .unwrap_or_else(|| panic!("red background pixels: swimlane={swimlane}, html={html}, padding={padding}"));
                assert!(
                    (101..=103).contains(&max_alpha),
                    "swimlane={swimlane}, html={html}, padding={padding}: alpha={max_alpha}",
                );
            }
        }
    }
}

#[test]
fn stateful_host_label_offsets_are_not_replayed_as_verified_geometry() {
    use merman::svg::{
        MeasurementProfileId, TextMeasurementPolicy, TextMeasurementProfile,
        TextMeasurementProfileIdentity, TextMeasurer, TextMetrics, TextStyle, ThemePreset,
    };
    use std::sync::{
        Arc,
        atomic::{AtomicUsize, Ordering},
    };
    struct ChangingOffset(Arc<AtomicUsize>);
    impl TextMeasurer for ChangingOffset {
        fn measure(&self, _: &str, style: &TextStyle) -> TextMetrics {
            TextMetrics {
                width: 80.0,
                height: style.font_size,
                line_count: 1,
            }
        }
        fn measure_svg_create_text_bbox_y_offset_px(&self, _: &str, _: &TextStyle) -> f64 {
            self.0.fetch_add(1, Ordering::SeqCst) as f64 * 0.125
        }
    }
    let calls = Arc::new(AtomicUsize::new(0));
    let environment = merman::SvgEnvironment::deterministic().with_text_measurement_policy(
        TextMeasurementPolicy::uniform(TextMeasurementProfile::new(
            TextMeasurementProfileIdentity::new(
                MeasurementProfileId::new("test.flowchart-changing-offset").unwrap(),
                "1",
            )
            .unwrap(),
            Arc::new(ChangingOffset(calls.clone())),
        )),
    );
    let theme = DiagramThemeCompiler::new()
        .compile_preset(ThemePreset::Cyberpunk)
        .unwrap();
    let RenderOutput::Document(Some(document)) = Renderer::new().render(
        RenderRequest::document(
            "---\nconfig:\n  htmlLabels: false\n---\nflowchart LR\nA[Alpha] -->|Advance| B[Beta]",
            OperationControl::new(),
            merman::SvgRequest { environment, ..Default::default() },
        ).with_theme(theme),
    ).unwrap() else { panic!("document required") };
    assert!(
        calls.load(Ordering::SeqCst) >= 2,
        "the host must observe both geometry requests"
    );
    let xml = roxmltree::Document::parse(document.svg()).unwrap();
    assert!(
        xml.descendants()
            .any(|node| node.is_text() && node.text() == Some("Advance"))
    );
    assert!(
        !xml.descendants()
            .filter(|node| node.attribute("class").is_some_and(|classes| {
                classes.split_whitespace().any(|class| class == "edgeLabel")
            }))
            .any(|label| label
                .descendants()
                .any(|node| node.attribute("filter").is_some())),
        "a changed terminal translation cannot consume the prepared edge-label filter"
    );
    let png = document
        .export_png(&Default::default(), OperationControl::new())
        .unwrap();
    assert!(
        png.admission()
            .reasons()
            .contains(&merman::TargetAdmissionReason::ThemeEvidenceIncomplete)
    );
}
