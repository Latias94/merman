//! Block text themes follow actual terminals, not unused CSS selectors.
//!
//! The pinned Block renderer emits composite and leaf labels but no diagram-title node.
//! Native output keeps absent title projections inert and paints composite labels directly.

#![cfg(all(merman_internal_theme_acceptance, feature = "png"))]

use merman::SourcePresentationConfig;
use merman::model::family::DiagramFamilyId;
use merman::svg::{
    CanvasPaint, DiagramTheme, DiagramThemeCompiler, DiagramThemeSpec, ThemeRule, ThemeRuleSet,
    ThemeStylePatch, ThemeTarget, ThemeVariant,
};
use merman::{
    MermaidEngine, OperationControl, RenderOutput, RenderRequest, RenderedDocument, Renderer,
};
use merman_theme_acceptance::svg_observation::SvgArtifactObservation;

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

#[test]
fn block_title_projection_has_no_native_consumer() {
    for scheme in ["default", "base", "dark", "forest", "neutral"] {
        for look in ["classic", "neo", "handDrawn"] {
            let renderer = Renderer::new().with_presentation_engine(
                MermaidEngine::new().with_site_config(SourcePresentationConfig::from_value(
                    serde_json::json!({"theme": scheme, "htmlLabels": false, "look": look}),
                )),
            );
            let baseline = render(
                &renderer,
                DiagramThemeCompiler::new()
                    .compile(DiagramThemeSpec::new())
                    .unwrap(),
            );
            let view = SvgArtifactObservation::observe(baseline.svg()).unwrap();
            let receipt = &view;
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
                    .as_chunks::<4>()
                    .0
                    .iter()
                    .any(|pixel| { pixel[3] != 0 && pixel[..3] != [255; 3] })
            );
            for variant in [None, Some(ThemeVariant::Default)] {
                for transparent in [false, true] {
                    let document =
                        render(&renderer, theme(ThemeTarget::Title, variant, transparent));
                    assert!(
                        raster(&document) == baseline_pixels,
                        "{scheme}/{look}/{variant:?}/{transparent}: absent title paint must preserve native pixels"
                    );
                }
            }
            for control_target in [ThemeTarget::Node, ThemeTarget::NodeLabel] {
                let control = render(&renderer, theme(control_target, None, false));
                assert!(
                    raster(&control) != baseline_pixels,
                    "{look}/{control_target:?}: the pixel comparison must detect real paint"
                );
            }
        }
    }
}

#[test]
fn block_cluster_label_paint_reaches_native_output_without_reviving_legacy_css() {
    for scheme in ["default", "base", "dark", "forest", "neutral"] {
        for look in ["classic", "neo", "handDrawn"] {
            let renderer = Renderer::new().with_presentation_engine(
                MermaidEngine::new().with_site_config(SourcePresentationConfig::from_value(
                    serde_json::json!({"theme": scheme, "htmlLabels": false, "look": look}),
                )),
            );
            let baseline = render(
                &renderer,
                DiagramThemeCompiler::new()
                    .compile(DiagramThemeSpec::new())
                    .unwrap(),
            );
            let baseline_pixels = raster(&baseline);
            for variant in [None, Some(ThemeVariant::Default)] {
                for transparent in [false, true] {
                    let document = render(
                        &renderer,
                        theme(ThemeTarget::ClusterLabel, variant, transparent),
                    );
                    let view = SvgArtifactObservation::observe(document.svg()).unwrap();
                    assert!(view.has_native_text());
                    assert!(!view.has_foreign_object());
                    let labels = view
                        .elements()
                        .iter()
                        .filter(|element| element.tag_name() == "text")
                        .collect::<Vec<_>>();
                    assert_eq!(
                        labels.len(),
                        3,
                        "composite and both leaf labels must remain"
                    );
                    let composite = labels
                        .iter()
                        .find(|element| element.text() == "Composite title")
                        .expect("composite label terminal");
                    let expected = if transparent {
                        "transparent"
                    } else {
                        "#b316cd"
                    };
                    assert_eq!(composite.style_value("fill"), Some(expected));
                    assert_eq!(
                        document
                            .svg()
                            .replacen(&format!(r#" style="fill:{expected}""#), "", 1),
                        baseline.svg(),
                        "{scheme}/{look}/{variant:?}/{transparent}: only the composite label terminal may change; leaf labels, geometry and legacy CSS must remain identical"
                    );
                    let evidence =
                        merman::__theme_acceptance::theme_acceptance_evidence(document.evidence());
                    let family = evidence.family();
                    assert!(family.is_verified());
                    assert_eq!(family.required_count(), 1);
                    assert_eq!(family.applied_count(), 1);
                    assert_eq!(family.not_applicable_count(), 0);
                    assert_eq!(family.residual_count(), 0);

                    let pixels = raster(&document);
                    assert_eq!((pixels.0, pixels.1), (baseline_pixels.0, baseline_pixels.1));
                    assert!(
                        pixels.2 != baseline_pixels.2,
                        "{scheme}/{look}/{variant:?}/{transparent}: direct composite label paint must change native pixels"
                    );
                    if !transparent {
                        assert!(
                            pixels
                                .2
                                .as_chunks::<4>()
                                .0
                                .iter()
                                .any(|pixel| { pixel[3] != 0 && pixel[..3] == [0xb3, 0x16, 0xcd] }),
                            "{scheme}/{look}/{variant:?}: composite label must paint the requested color"
                        );
                    }
                }
            }
        }
    }
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
            let engine =
                MermaidEngine::new().with_site_config(SourcePresentationConfig::from_value(
                    serde_json::json!({"htmlLabels": false, "look": look}),
                ));
            let baseline = engine.parse_metadata_sync(SOURCE).unwrap();
            for variant in [None, Some(ThemeVariant::Default)] {
                for transparent in [false, true] {
                    let theme = theme(target, variant, transparent);
                    let plan = merman::ThemeCompatibilityPlan::from_recipe(
                        theme.compatibility_recipe().clone(),
                    );
                    let parser = engine.clone().with_theme_plan(&plan);
                    let metadata = parser.parse_metadata_sync(SOURCE).unwrap();
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
