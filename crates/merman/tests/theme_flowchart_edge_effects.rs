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
    let theme = DiagramThemeCompiler::new().compile(
        DiagramThemeSpec::new()
            .with_typography(TypographySpec::default().with_family_style(
                merman::DiagramFamilyId::FLOWCHART,
                ThemeTextStyle::default().with_font_stack(FontStack::single("Excalifont").unwrap()),
            ))
            .with_assets(ThemeAssets::default().with_font_catalog(FontCatalogSpec::new([
                FontAssetSpec::new("excalifont", include_bytes!("../../../fixtures/themes/assets/fonts/Excalifont-Regular-Latin.woff2")),
            ])))
            .with_effects(DiagramEffectSet::default().with_graph(
                EffectGraph::new("text-glow", [EffectPrimitive::DropShadow {
                    input: EffectInput::SourceGraphic,
                    offset_x: 0.0, offset_y: 0.0, blur_radius: 6.0, spread: 0.0,
                    color: ThemeColorValue::parse("#ff2080").unwrap(),
                }]).unwrap().with_color_space(EffectColorSpace::Srgb),
            ).unwrap()
                .with_binding(EffectBinding::new(ThemeTarget::NodeLabel, "text-glow").unwrap()).unwrap()
                .with_binding(EffectBinding::new(ThemeTarget::EdgeLabel, "text-glow").unwrap()).unwrap())
            .with_styles(ThemeRuleSet::default()
                .with_rule(ThemeRule::new(ThemeTarget::Node, ThemeStylePatch::default()
                    .with_fill(CanvasPaint::solid("#ffffff").unwrap()).with_stroke(CanvasPaint::solid("#000000").unwrap())))
                .with_rule(ThemeRule::new(ThemeTarget::Edge, ThemeStylePatch::default().with_stroke(CanvasPaint::solid("#000000").unwrap())))),
    ).unwrap();
    let source =
        "---\nconfig:\n  htmlLabels: false\n---\nflowchart LR\nA[Alpha] -->|Advance| B[Beta]";
    let RenderOutput::Document(Some(document)) = Renderer::new()
        .render(
            RenderRequest::document(source, OperationControl::new(), Default::default())
                .with_theme(theme),
        )
        .unwrap()
    else {
        panic!("document required")
    };
    let png = document
        .export_png(
            &merman::svg::export::RasterOptions::default(),
            OperationControl::new(),
        )
        .unwrap();
    assert!(
        !png.admission()
            .reasons()
            .contains(&merman::TargetAdmissionReason::ThemeEvidenceIncomplete)
    );
    let mut reader = png::Decoder::new(std::io::Cursor::new(png.bytes()))
        .read_info()
        .unwrap();
    let mut pixels = vec![0; reader.output_buffer_size().unwrap()];
    let info = reader.next_frame(&mut pixels).unwrap();
    assert_eq!(info.color_type, png::ColorType::Rgba);
    // Only the text shadow has a chromatic paint; definitions alone cannot satisfy this.
    assert!(
        pixels[..info.buffer_size()]
            .chunks_exact(4)
            .any(|pixel| pixel[3] > 0 && pixel[0] > pixel[2] && pixel[2] > pixel[1]),
        "text glow must reach PNG pixels"
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
