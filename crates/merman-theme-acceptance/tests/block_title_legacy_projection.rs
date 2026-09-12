//! Block absent text projections follow actual terminals, not unused CSS selectors.
//!
//! The pinned Block renderer emits composite and leaf labels but no diagram-title node.
//! Native pixel comparisons retain those labels and an active Node fill control.

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

const SOURCE: &str = "---\ntitle: Metadata title\n---\nblock\n  columns 1\n  block:group[\"Composite title\"]\n    columns 2\n    A[\"Alpha\"] B[\"Beta\"]\n    A --> B\n  end\n";

fn theme(target: ThemeTarget, variant: Option<ThemeVariant>, transparent: bool) -> DiagramTheme {
    let paint = if transparent {
        CanvasPaint::Transparent
    } else {
        CanvasPaint::solid("#b316cd").unwrap()
    };
    let mut rule = ThemeRule::new(target, ThemeStylePatch::default().with_fill(paint))
        .for_family(DiagramFamilyId::BLOCK);
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
        panic!("missing Block document")
    };
    document
}

fn raster(document: &RenderedDocument) -> (u32, u32, Vec<u8>) {
    let png = document
        .export_png(
            &merman_export::RasterOptions::default().with_scale(2.0),
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

fn assert_no_native_consumer(target: ThemeTarget) {
    for look in ["classic", "neo", "handDrawn"] {
        let renderer = Renderer::new().with_engine(Engine::new().with_site_config(
            MermaidConfig::from_value(serde_json::json!({"htmlLabels": false, "look": look})),
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
        assert!(receipt.elements().iter().all(|element| {
            !element.has_class("flowchartTitleText") && !element.has_class("cluster-label")
        }));
        let clusters = receipt
            .elements()
            .iter()
            .filter(|element| element.has_class("cluster"));
        let mut cluster_count = 0;
        for cluster in clusters {
            cluster_count += 1;
            assert!(
                receipt
                    .descendants_of(cluster.index())
                    .all(|element| { !matches!(element.tag_name(), "text" | "span" | "p") }),
                "titleColor's cluster descendant selectors must have no text consumer"
            );
        }
        assert!(cluster_count > 0, "the composite case must be exercised");
        let baseline_pixels = raster(&baseline);
        assert!(baseline_pixels.0 > 1 && baseline_pixels.1 > 1);
        assert!(
            baseline_pixels
                .2
                .chunks_exact(4)
                .any(|pixel| { pixel[3] != 0 && pixel[..3] != [255; 3] })
        );
        for variant in [None, Some(ThemeVariant::Default)] {
            for transparent in [false, true] {
                let document = render(&renderer, theme(target, variant, transparent));
                assert_eq!(
                    raster(&document),
                    baseline_pixels,
                    "{look}/{variant:?}/{transparent}"
                );
            }
        }
        for control_target in [ThemeTarget::Node, ThemeTarget::NodeLabel] {
            let control = render(&renderer, theme(control_target, None, false));
            assert_ne!(
                raster(&control),
                baseline_pixels,
                "{look}/{control_target:?}: the pixel comparison must detect real paint"
            );
        }
    }
}

#[test]
fn block_title_projection_has_no_native_consumer() {
    assert_no_native_consumer(ThemeTarget::Title);
}

#[test]
fn block_cluster_label_projection_has_no_native_consumer() {
    assert_no_native_consumer(ThemeTarget::ClusterLabel);
}

#[test]
fn retired_block_text_bridges_are_absent() {
    for (target, paths) in [
        (ThemeTarget::Title, &["themeVariables.titleColor"][..]),
        (
            ThemeTarget::ClusterLabel,
            &[
                "themeVariables.secondaryTextColor",
                "themeVariables.tertiaryTextColor",
            ][..],
        ),
    ] {
        for look in ["classic", "neo", "handDrawn"] {
            let engine = Engine::new().with_site_config(MermaidConfig::from_value(
                serde_json::json!({"htmlLabels": false, "look": look}),
            ));
            let baseline = engine.parse_metadata_sync(SOURCE).unwrap();
            for variant in [None, Some(ThemeVariant::Default)] {
                for transparent in [false, true] {
                    let theme = theme(target, variant, transparent);
                    let parser = merman_render::__private::install_parse_compatibility(
                        &theme,
                        engine.clone(),
                    );
                    let metadata = parser.parse_metadata_sync(SOURCE).unwrap();
                    let evidence = merman::__private::theme_parse_evidence(&metadata);
                    assert_eq!(evidence.fallback_contributions().len(), 0);
                    for path in paths {
                        assert_eq!(
                            metadata.effective_config.get_str(path),
                            baseline.effective_config.get_str(path),
                            "{target:?}/{look}/{variant:?}/{transparent}/{path}"
                        );
                    }
                }
            }
        }
    }
}
