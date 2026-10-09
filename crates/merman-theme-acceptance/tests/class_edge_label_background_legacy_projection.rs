//! Class label-background themes follow current SVG terminals and native output.

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
fn class_direct_background_reaches_native_output_without_reviving_the_retired_css_projection() {
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
                        let stylesheet = |svg: &str| {
                            svg.split_once("<style>")
                                .unwrap()
                                .1
                                .split_once("</style>")
                                .unwrap()
                                .0
                                .to_owned()
                        };
                        assert_eq!(
                            stylesheet(document.svg()),
                            stylesheet(baseline.svg()),
                            "direct paint must leave the retired and parity CSS projections unchanged"
                        );
                        let view = TargetArtifactView::from_rendered_document(&document);
                        let receipt = view.svg_artifact_receipt().unwrap();
                        let expected = if transparent {
                            "transparent"
                        } else {
                            "#b316cd"
                        };
                        let label_groups = receipt
                            .elements()
                            .iter()
                            .filter(|element| {
                                if html_labels {
                                    element.attribute("data-merman-foreignobject")
                                        == Some("fallback")
                                        && element
                                            .attribute("data-merman-source-classes")
                                            .is_some_and(|classes| {
                                                classes
                                                    .split_ascii_whitespace()
                                                    .any(|class| class == "edgeLabel")
                                            })
                                } else {
                                    element.has_class("edgeLabel")
                                }
                            })
                            .collect::<Vec<_>>();
                        assert_eq!(label_groups.len(), 1);
                        let backgrounds = receipt
                            .descendants_of(label_groups[0].index())
                            .filter(|element| element.tag_name() == "rect")
                            .collect::<Vec<_>>();
                        if html_labels && transparent {
                            // Transparent HTML backgrounds intentionally produce no fallback rect.
                            assert!(backgrounds.is_empty());
                        } else {
                            assert_eq!(
                                backgrounds.len(),
                                1,
                                "{scheme}/{look}/html={html_labels}/{variant:?}/{transparent}"
                            );
                            let background = backgrounds[0];
                            assert!(
                                background
                                    .attribute("style")
                                    .unwrap_or("")
                                    .contains(expected)
                                    || background.attribute("fill") == Some(expected),
                                "{scheme}/{look}/html={html_labels}/{variant:?}/{transparent}: native label background paint"
                            );
                        }
                        assert_ne!(
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

#[test]
fn class_edge_label_background_selector_has_no_current_upstream_group_consumer() {
    // Current upstream SVG witnesses cover classic/Dagre, handDrawn/Dagre, and ELK.
    // The fixture set does not expose a separate HTML/SVG switch for every look; this test
    // records only dimensions represented by the current checked-in artifacts.
    let fixtures = [
        include_str!(
            "../../../fixtures/upstream-svgs/class/upstream_cypress_classdiagram_v3_spec_should_render_a_simple_class_diagram_with_a_custom_theme_056.svg"
        ),
        include_str!(
            "../../../fixtures/upstream-svgs/class/upstream_cypress_classdiagram_handdrawn_v3_spec_hd_should_render_a_class_with_text_label_033.svg"
        ),
        include_str!(
            "../../../fixtures/upstream-svgs/class/upstream_cypress_classdiagram_elk_v3_spec_elk_should_render_a_simple_class_diagram_with_a_custom_theme_055.svg"
        ),
    ];

    for svg in fixtures {
        assert!(svg.contains(".edgeLabel[data-look=\"neo\"]"));
        let mut cursor = 0usize;
        while let Some(relative) = svg[cursor..].find("<g class=\"edgeLabel\"") {
            let start = cursor + relative;
            let end = svg[start..]
                .find("</g>")
                .map(|offset| start + offset)
                .unwrap_or(svg.len());
            let group = &svg[start..end];
            assert!(
                !group.contains("data-look="),
                "current upstream Class edgeLabel group unexpectedly carries data-look: {group}"
            );
            cursor = end.saturating_add(4);
            if cursor >= svg.len() {
                break;
            }
        }
        assert!(svg.contains(".edgeLabel .label rect{fill:"));
        assert!(svg.contains(".labelBkg{background:"));
        assert!(svg.contains(".edgeLabel .label span{background:"));
    }
}
