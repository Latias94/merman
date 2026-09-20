#![cfg(feature = "svg")]

use merman::{OperationControl, RenderError, RenderOutput, RenderRequest, Renderer, SvgRequest};

fn render(source: &str, diagram_id: Option<&str>) -> Result<merman::SvgOutput, RenderError> {
    let request = SvgRequest {
        options: merman::svg::SvgRenderOptions {
            diagram_id: diagram_id.map(str::to_owned),
            ..Default::default()
        },
        ..Default::default()
    };
    let output =
        Renderer::new().render(RenderRequest::svg(source, OperationControl::new(), request))?;
    let RenderOutput::Svg(Some(svg)) = output else {
        return Err(RenderError::UnsupportedTarget(
            "no Mermaid diagram detected",
        ));
    };
    Ok(svg)
}

#[test]
fn typed_facade_renders_a_mermaid_diagram() {
    let svg = render("flowchart TD\nA[Start] --> B[Done]", None).expect("diagram renders");

    assert!(svg.svg().contains("<svg"));
    assert!(svg.svg().contains("Start"));
    assert!(svg.svg().contains("Done"));
    assert_eq!(svg.evidence().execution_path().as_str(), "renderer");
}

#[test]
fn typed_facade_preserves_detected_diagram_errors() {
    let error = render("flowchart TD\nA -->", None).expect_err("invalid Mermaid must fail");
    assert!(matches!(error, RenderError::Parse(_)));
}

#[test]
fn typed_facade_accepts_a_document_unique_diagram_id() {
    let first = render("flowchart TD\nA --> B", Some("docs-first")).expect("first renders");
    let second = render("flowchart TD\nA --> B", Some("docs second")).expect("second renders");

    assert!(first.svg().contains(r#"id="docs-first""#));
    assert!(second.svg().contains(r#"id="docs-second""#));
}

#[test]
fn width_callback_controls_emitted_svg_rows_and_node_bounds() {
    use merman::svg::{
        DeterministicTextMeasurer, MeasurementProfileId, TextMeasurementPolicy,
        TextMeasurementProfile, TextMeasurementProfileIdentity,
    };

    // Model kerning and zero-advance combining marks without depending on installed fonts.
    fn width(text: &str) -> f64 {
        let text = text.replace("👩‍🔬", "X").replace("e\u{301}", "e");
        text.chars().count() as f64 * 10.0 - text.matches("AV").count() as f64 * 5.0
    }

    let measurer = DeterministicTextMeasurer::default().with_width_callback(|text, _| width(text));
    let identity = TextMeasurementProfileIdentity::new(
        MeasurementProfileId::new("test.whole-string-width").unwrap(),
        "1",
    )
    .unwrap();
    let policy = TextMeasurementPolicy::uniform(TextMeasurementProfile::new(identity, measurer));

    for (label, expected_rows) in [
        ("AVAVAVAV", vec!["AVAV", "AVAV"]),
        (
            "e\u{301}e\u{301}e\u{301}e\u{301}",
            vec!["e\u{301}e\u{301}e\u{301}", "e\u{301}"],
        ),
        ("👩‍🔬👩‍🔬👩‍🔬👩‍🔬", vec!["👩‍🔬👩‍🔬👩‍🔬", "👩‍🔬"]),
    ] {
        for markdown in [false, true] {
            let source_label = if markdown {
                format!("`{label}`")
            } else {
                label.to_string()
            };
            let source = format!(
                "%%{{init: {{\"htmlLabels\": false, \"flowchart\": {{\"htmlLabels\": false, \"wrappingWidth\": 30}}}}}}%%\nflowchart TD\nA[\"{source_label}\"]"
            );
            let request = SvgRequest {
                environment: merman::SvgEnvironment::deterministic()
                    .with_text_measurement_policy(policy.clone()),
                ..Default::default()
            };
            let output = Renderer::new()
                .render(RenderRequest::svg(
                    &source,
                    OperationControl::new(),
                    request,
                ))
                .expect("callback diagram renders");
            let RenderOutput::Svg(Some(svg)) = output else {
                panic!("expected SVG output");
            };
            let document = roxmltree::Document::parse(svg.svg()).expect("valid SVG");
            assert!(
                !document
                    .descendants()
                    .any(|node| node.has_tag_name("foreignObject"))
            );
            let node = document
                .descendants()
                .find(|node| {
                    node.attribute("class").is_some_and(|classes| {
                        classes.split_whitespace().any(|class| class == "node")
                    })
                })
                .expect("flowchart node");
            let rows: Vec<String> = node
                .descendants()
                .filter(|node| {
                    node.has_tag_name("tspan")
                        && node.attribute("class").is_some_and(|classes| {
                            classes
                                .split_whitespace()
                                .any(|class| class == "text-outer-tspan")
                        })
                })
                .map(|row| {
                    row.descendants()
                        .filter(|node| node.is_text())
                        .filter_map(|node| node.text())
                        .collect()
                })
                .collect();
            assert_eq!(rows, expected_rows, "label={label:?}, markdown={markdown}");
            assert_eq!(rows.concat(), label);

            let rectangle = node
                .children()
                .find(|child| child.has_tag_name("rect"))
                .expect("node rectangle");
            let node_width: f64 = rectangle.attribute("width").unwrap().parse().unwrap();
            let node_height: f64 = rectangle.attribute("height").unwrap().parse().unwrap();
            assert!(
                rows.iter()
                    .all(|row| width(row) <= 30.0 && width(row) < node_width)
            );
            assert!(rows.len() as f64 * 16.0 * 1.1 < node_height);
        }
    }
}
