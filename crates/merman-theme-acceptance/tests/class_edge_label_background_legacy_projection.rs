//! Compare the unused Class edge-label background projection with real visible paint controls.

#![cfg(all(merman_internal_theme_acceptance, feature = "png"))]

use merman::__theme_acceptance::TargetArtifactView;
use merman::svg::{
    CanvasPaint, DiagramTheme, DiagramThemeCompiler, DiagramThemeSpec, ThemeRule, ThemeRuleSet,
    ThemeStylePatch, ThemeTarget, ThemeVariant,
};
use merman::{
    DiagramFamilyId, Engine, MermaidConfig, OperationControl, RenderOutput, RenderRequest,
    RenderedDocument, Renderer,
};

const SOURCE: &str = "classDiagram\nnamespace Alpha {\nclass A\n}\nnamespace Beta {\nclass B\n}\nA --> B : relates\n";

fn theme(target: ThemeTarget, variant: Option<ThemeVariant>, transparent: bool) -> DiagramTheme {
    let paint = if transparent {
        CanvasPaint::Transparent
    } else {
        CanvasPaint::solid("#b316cd").unwrap()
    };
    let mut rule = ThemeRule::new(target, ThemeStylePatch::default().with_fill(paint))
        .for_family(DiagramFamilyId::CLASS);
    if let Some(variant) = variant {
        rule = rule.with_variant(variant);
    }
    DiagramThemeCompiler::new()
        .compile(DiagramThemeSpec::new().with_styles(ThemeRuleSet::default().with_rule(rule)))
        .unwrap()
}

fn render(renderer: &Renderer, theme: DiagramTheme) -> RenderedDocument {
    let RenderOutput::Document(Some(document)) = renderer
        .render(
            RenderRequest::document(SOURCE, OperationControl::new(), Default::default())
                .with_theme(theme),
        )
        .unwrap()
    else {
        panic!("missing Class document")
    };
    document
}

fn raster(document: &RenderedDocument) -> (u32, u32, Vec<u8>) {
    let png = document
        .export_png(
            &merman_export::RasterOptions::default(),
            OperationControl::new(),
        )
        .unwrap();
    let mut reader = png::Decoder::new(std::io::Cursor::new(png.bytes()))
        .read_info()
        .unwrap();
    let mut pixels = vec![0; reader.output_buffer_size().unwrap()];
    let frame = reader.next_frame(&mut pixels).unwrap();
    assert_eq!(frame.color_type, png::ColorType::Rgba);
    assert_eq!(frame.bit_depth, png::BitDepth::Eight);
    pixels.truncate(frame.buffer_size());
    (frame.width, frame.height, pixels)
}

#[test]
fn class_edge_label_background_projection_has_no_native_consumer() {
    for scheme in ["default", "base", "dark", "forest", "neutral"] {
        for look in ["classic", "neo", "handDrawn"] {
            for html_labels in [false, true] {
                let renderer = Renderer::new().with_engine(Engine::new().with_site_config(
                    MermaidConfig::from_value(serde_json::json!({
                        "theme": scheme, "look": look, "htmlLabels": html_labels,
                    })),
                ));
                let baseline = render(
                    &renderer,
                    DiagramThemeCompiler::new()
                        .compile(DiagramThemeSpec::new())
                        .unwrap(),
                );
                let view = TargetArtifactView::from_rendered_document(&baseline);
                let receipt = view.svg_artifact_receipt().unwrap();
                assert!(receipt.has_native_text());
                assert!(!receipt.has_foreign_object());
                assert!(baseline.svg().contains("relates"));
                let groups = receipt
                    .elements()
                    .iter()
                    .filter(|element| element.tag_name() == "g" && element.has_class("edgeLabel"))
                    .collect::<Vec<_>>();
                assert!(!groups.is_empty());
                assert!(
                    groups
                        .iter()
                        .all(|group| group.attribute("data-look").is_none())
                );
                let pixels = raster(&baseline);
                assert!(pixels.0 > 1 && pixels.1 > 1);
                assert!(
                    pixels
                        .2
                        .chunks_exact(4)
                        .any(|pixel| pixel[3] != 0 && pixel[..3] != [255; 3])
                );
                for variant in [None, Some(ThemeVariant::Default)] {
                    for transparent in [false, true] {
                        let document = render(
                            &renderer,
                            theme(ThemeTarget::EdgeLabelBackground, variant, transparent),
                        );
                        assert_eq!(
                            document.svg(),
                            baseline.svg(),
                            "retired Class background paint must not alter even the unused CSS"
                        );
                        assert_eq!(
                            raster(&document),
                            pixels,
                            "{scheme}/{look}/html={html_labels}/{variant:?}/{transparent}"
                        );
                    }
                }
                for target in [ThemeTarget::Node, ThemeTarget::NodeLabel] {
                    let control = render(&renderer, theme(target, None, false));
                    assert_ne!(
                        raster(&control),
                        pixels,
                        "{scheme}/{look}/html={html_labels}/{target:?}: visible paint control"
                    );
                }
            }
        }
    }
}
