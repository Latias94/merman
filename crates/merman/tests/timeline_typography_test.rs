#![cfg(all(feature = "native-fonts", feature = "diagram-timeline"))]

use merman::svg::export::NativeFontOptions;
use merman::svg::{TextMeasurementOperation, TextMeasurementSource};
use merman::{
    MermaidEngine, OperationControl, RenderOutput, RenderRequest, Renderer,
    SourcePresentationConfig, SvgEnvironment, SvgOutput, SvgRequest,
};
use serde_json::json;

fn geometry(svg: &str) -> Vec<(String, Vec<(String, String)>)> {
    roxmltree::Document::parse(svg)
        .unwrap()
        .descendants()
        .filter(|node| node.is_element() && !node.has_tag_name("style"))
        .map(|node| {
            (
                node.tag_name().name().to_owned(),
                node.attributes()
                    .map(|attribute| (attribute.name().to_owned(), attribute.value().to_owned()))
                    .collect(),
            )
        })
        .collect()
}

#[test]
fn native_timeline_shared_sizes_keep_known_geometry_and_measurement_routes() {
    let fonts =
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../merman-export/tests/fonts");
    let context = NativeFontOptions::default()
        .with_font_file(fonts.join("Cousine-Regular.ttf"))
        .with_font_file(fonts.join("Cousine-Bold.ttf"))
        .load(&OperationControl::new())
        .unwrap();
    let request = SvgRequest {
        environment: SvgEnvironment::deterministic()
            .with_native_font_context(context)
            .unwrap(),
        ..Default::default()
    };
    for theme in ["default", "redux"] {
        for direction in ["LR", "TD"] {
            for root_size in [0, 32] {
                let source = format!(
                    "timeline {direction}\nsection Plan\nTask : A long event label containing several words that must wrap\n"
                );
                let render = |size: &str| -> SvgOutput {
                    let renderer = Renderer::new().with_presentation_engine(
                        MermaidEngine::new().with_site_config(
                            SourcePresentationConfig::from_value(json!({
                                "theme": theme,
                                "fontSize": root_size,
                                "themeVariables": {"fontFamily": "Cousine", "fontSize": size}
                            })),
                        ),
                    );
                    let RenderOutput::Svg(Some(output)) = renderer
                        .render(RenderRequest::svg(
                            &source,
                            OperationControl::new(),
                            request.clone(),
                        ))
                        .unwrap()
                    else {
                        panic!("expected Timeline SVG");
                    };
                    output
                };
                let expected = render("16px");
                for size in ["-1px", "", "bogus"] {
                    let output = render(size);
                    assert_eq!(
                        output.svg(),
                        expected.svg(),
                        "{theme}/{direction}/{root_size}/{size:?}"
                    );
                    let entries = output.evidence().measurement().entries();
                    assert!(entries.iter().any(|entry| {
                        entry.provenance().operation == TextMeasurementOperation::ComputedLength
                    }));
                    assert!(entries.iter().all(|entry| {
                        entry.provenance().source == TextMeasurementSource::Host
                            && entry.provenance().fallback_reason.is_none()
                    }));
                }
                for (size, control) in [
                    ("0em", "0px"),
                    ("0rem", "0px"),
                    ("0%", "0px"),
                    ("24pt", "32px"),
                ] {
                    let output = render(size);
                    assert_eq!(
                        geometry(output.svg()),
                        geometry(render(control).svg()),
                        "{theme}/{direction}/{root_size}/{size:?}"
                    );
                    assert!(output.svg().contains(&format!("font-size:{size};")));
                }
            }
        }
    }
}
