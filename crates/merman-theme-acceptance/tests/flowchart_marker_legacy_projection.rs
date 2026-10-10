//! Visual investigation of the historical Marker -> arrowheadColor dead projection.
//!
//! These comparisons do not issue KTD23 receipts or qualify typed Marker support. A Release run
//! on pre-retirement revision 395a4d2f2 verified all 64 former bridge projections and all 64 native
//! pixel comparisons. The current tests retain the visual regression and require bridge absence
//! after the projections become explicit Unsupported requests.

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
use merman_render::__private::SvgArtifactReceipt;

const FAMILIES: [DiagramFamilyId; 2] = [DiagramFamilyId::FLOWCHART, DiagramFamilyId::SWIMLANE];
const PROFILES: [(&str, bool); 4] = [
    ("classic", false),
    ("neo", false),
    ("neo", true),
    ("handDrawn", false),
];
const CONTROL: &str = "#b316cd";

fn source(family: DiagramFamilyId, animated: bool) -> String {
    let prefix = if family == DiagramFamilyId::SWIMLANE {
        "---\nconfig:\n  layout: swimlane\n---\n"
    } else {
        ""
    };
    let body = if animated {
        "flowchart LR\nA[Alpha] circles@o--o B[Beta]\nB crosses@x--x C[Gamma]\nC points@<--> D[Delta]\ncircles@{ animate: true }\ncrosses@{ animate: true }\npoints@{ animate: true }\n"
    } else {
        "flowchart LR\nA[Alpha] o--o B[Beta]\nB x--x C[Gamma]\nC <--> D[Delta]\n"
    };
    format!("{prefix}{body}")
}

fn engine(look: &str) -> Engine {
    Engine::new().with_site_config(MermaidConfig::from_value(serde_json::json!({
        "htmlLabels": false,
        "flowchart": { "htmlLabels": false },
        "look": look,
    })))
}

fn marker_theme(
    family: DiagramFamilyId,
    variant: Option<ThemeVariant>,
    stroke: bool,
    transparent: bool,
) -> DiagramTheme {
    let paint = if transparent {
        CanvasPaint::Transparent
    } else {
        CanvasPaint::solid(CONTROL).unwrap()
    };
    let patch = if stroke {
        ThemeStylePatch::default().with_stroke(paint)
    } else {
        ThemeStylePatch::default().with_fill(paint)
    };
    let mut rule = ThemeRule::new(ThemeTarget::Marker, patch).for_family(family);
    if let Some(variant) = variant {
        rule = rule.with_variant(variant);
    }
    DiagramThemeCompiler::new()
        .compile(DiagramThemeSpec::new().with_styles(ThemeRuleSet::default().with_rule(rule)))
        .unwrap()
}

fn render(renderer: &Renderer, source: &str, theme: DiagramTheme) -> RenderedDocument {
    let RenderOutput::Document(Some(document)) = renderer
        .render(
            RenderRequest::document(source, OperationControl::new(), Default::default())
                .with_theme(theme),
        )
        .unwrap()
    else {
        panic!("missing marker investigation document")
    };
    document
}

fn marker_references(receipt: &SvgArtifactReceipt) -> Vec<String> {
    assert!(receipt.has_native_text());
    assert!(!receipt.has_foreign_object());
    assert!(
        receipt
            .elements()
            .iter()
            .all(|element| !element.has_class("arrowheadPath")),
        "the historical arrowheadColor selector must not match an actual element"
    );
    let paths = receipt
        .elements()
        .iter()
        .filter(|element| element.tag_name() == "path" && element.has_class("flowchart-link"))
        .collect::<Vec<_>>();
    assert_eq!(
        paths.len(),
        3,
        "the scenario must emit all three edge paths"
    );
    let mut references = Vec::new();
    for path in paths {
        assert!(path.attribute("d").is_some_and(|value| !value.is_empty()));
        for position in ["marker-start", "marker-end"] {
            let reference = path
                .attribute(position)
                .expect("bidirectional marker reference");
            let id = reference
                .strip_prefix("url(#")
                .and_then(|value| value.strip_suffix(')'))
                .expect("local marker URL");
            let definitions = receipt
                .elements()
                .iter()
                .filter(|element| element.tag_name() == "marker" && element.id() == Some(id))
                .collect::<Vec<_>>();
            assert_eq!(
                definitions.len(),
                1,
                "marker reference must resolve uniquely"
            );
            let definition = definitions[0];
            for dimension in ["markerWidth", "markerHeight"] {
                assert!(
                    definition
                        .numeric_attribute(dimension)
                        .is_some_and(|value| value > 0.0)
                );
            }
            assert!(receipt.descendants_of(definition.index()).any(|element| {
                matches!(element.tag_name(), "path" | "polygon" | "circle")
                    && element.has_class("arrowMarkerPath")
            }));
            references.push(reference.to_owned());
        }
    }
    assert_eq!(references.len(), 6);
    references
}

fn raster(document: &RenderedDocument) -> image::RgbaImage {
    let png = document
        .export_png(
            &merman_export::RasterOptions::default().with_scale(2.0),
            OperationControl::new(),
        )
        .unwrap();
    // The workspace image dependency enables JPEG only. Decode the production PNG using the
    // existing PNG leaf dependency, then retain an image buffer for exact RGBA comparison.
    let decoder = png::Decoder::new(std::io::Cursor::new(png.bytes()));
    let mut reader = decoder.read_info().unwrap();
    let mut pixels = vec![0; reader.output_buffer_size().unwrap()];
    let frame = reader.next_frame(&mut pixels).unwrap();
    assert_eq!(frame.color_type, png::ColorType::Rgba);
    assert_eq!(frame.bit_depth, png::BitDepth::Eight);
    pixels.truncate(frame.buffer_size());
    image::RgbaImage::from_raw(frame.width, frame.height, pixels).unwrap()
}

#[test]
fn marker_requests_do_not_recolor_flowchart_or_swimlane_native_output() {
    let baseline_theme = DiagramThemeCompiler::new()
        .compile(DiagramThemeSpec::new())
        .unwrap();
    for family in FAMILIES {
        for (look, animated) in PROFILES {
            let source = source(family, animated);
            let renderer = Renderer::new().with_engine(engine(look));
            let baseline = render(&renderer, &source, baseline_theme.clone());
            assert_eq!(baseline.evidence().family_id(), family);
            let baseline_view = TargetArtifactView::from_rendered_document(&baseline);
            let baseline_references =
                marker_references(baseline_view.svg_artifact_receipt().unwrap());
            let baseline_raster = raster(&baseline);
            assert!(baseline_raster.width() > 1 && baseline_raster.height() > 1);
            assert!(
                baseline_raster
                    .pixels()
                    .any(|pixel| pixel[3] != 0 && pixel.0[..3] != [255; 3])
            );
            for variant in [None, Some(ThemeVariant::Default)] {
                for stroke in [false, true] {
                    for transparent in [false, true] {
                        let context = format!(
                            "{family:?}/{look}/animated={animated}/{variant:?}/stroke={stroke}/transparent={transparent}"
                        );
                        let document = render(
                            &renderer,
                            &source,
                            marker_theme(family, variant, stroke, transparent),
                        );
                        assert_eq!(document.evidence().family_id(), family, "{context}");
                        let view = TargetArtifactView::from_rendered_document(&document);
                        assert_eq!(
                            marker_references(view.svg_artifact_receipt().unwrap()),
                            baseline_references,
                            "marker references changed: {context}"
                        );
                        let actual = raster(&document);
                        assert_eq!(
                            actual.dimensions(),
                            baseline_raster.dimensions(),
                            "{context}"
                        );
                        assert!(
                            actual.as_raw() == baseline_raster.as_raw(),
                            "Marker request changed native pixels: {context}"
                        );
                    }
                }
            }
        }
    }
}

#[test]
fn retired_marker_bridge_is_absent() {
    for family in FAMILIES {
        for (look, animated) in PROFILES {
            let source = source(family, animated);
            let baseline = engine(look).parse_metadata_sync(&source).unwrap();
            for variant in [None, Some(ThemeVariant::Default)] {
                for stroke in [false, true] {
                    for transparent in [false, true] {
                        let theme = marker_theme(family, variant, stroke, transparent);
                        let parser = merman_render::__private::install_parse_compatibility(
                            &theme,
                            engine(look),
                        );
                        let metadata = parser.parse_metadata_sync(&source).unwrap();
                        let context = format!(
                            "{family:?}/{look}/animated={animated}/{variant:?}/stroke={stroke}/transparent={transparent}"
                        );
                        assert_eq!(
                            metadata
                                .effective_config
                                .get_str("themeVariables.arrowheadColor"),
                            baseline
                                .effective_config
                                .get_str("themeVariables.arrowheadColor"),
                            "retired Marker paint changed finalized config: {context}"
                        );
                    }
                }
            }
        }
    }
}
