#![cfg(feature = "svg")]

use merman::{
    DiagramFamilyId, OperationControl, RenderError, RenderOutput, RenderRequest, Renderer,
    SvgRequest,
};

fn render(source: &str, diagram_id: Option<&str>) -> Result<merman::SvgOutput, Box<RenderError>> {
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
        return Err(Box::new(RenderError::UnsupportedTarget(
            "no Mermaid diagram detected",
        )));
    };
    Ok(svg)
}

fn fallback_content<'a, 'input>(
    instance: roxmltree::Node<'a, 'input>,
) -> roxmltree::Node<'a, 'input> {
    let reference = instance
        .children()
        .find(|node| node.has_tag_name("use"))
        .and_then(|node| node.attribute("href"))
        .expect("fallback instance references its definition");
    let id = reference
        .strip_prefix('#')
        .expect("same-document fallback reference");
    let definition = instance
        .document()
        .descendants()
        .find(|node| node.attribute("id") == Some(id))
        .expect("fallback definition exists");
    assert!(
        definition
            .parent()
            .is_some_and(|node| node.has_tag_name("defs"))
    );
    definition
}

#[test]
fn typed_facade_renders_a_mermaid_diagram() {
    let svg = render("flowchart TD\nA[Start] --> B[Done]", None).expect("diagram renders");

    assert!(svg.svg().contains("<svg"));
    assert!(svg.svg().contains("Start"));
    assert!(svg.svg().contains("Done"));
    assert_eq!(svg.evidence().family_id(), DiagramFamilyId::FLOWCHART);
}

#[test]
fn typed_facade_preserves_detected_diagram_errors() {
    let error = render("flowchart TD\nA -->", None)
        .map(|_| ())
        .expect_err("invalid Mermaid must fail");
    assert!(matches!(*error, RenderError::Parse(_)));
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

#[test]
fn resvg_safe_keeps_long_path_rows_inside_measured_flowchart_nodes() {
    use merman::svg::{
        DeterministicTextMeasurer, MeasurementProfileId, SvgPipeline, TextMeasurementPolicy,
        TextMeasurementProfile, TextMeasurementProfileIdentity,
    };

    let measurer = DeterministicTextMeasurer::default()
        .with_width_callback(|text, _| text.chars().count() as f64 * 10.0);
    let identity = TextMeasurementProfileIdentity::new(
        MeasurementProfileId::new("test.html-path-width").unwrap(),
        "1",
    )
    .unwrap();
    let policy = TextMeasurementPolicy::uniform(TextMeasurementProfile::new(identity, measurer));
    let source = r#"%%{init: {"layout":"dagre","look":"classic","flowchart":{"wrappingWidth":200}}}%%
flowchart TD
  A["docs/plans/2026-10-11-release-checklist.final.md"] --> B[Short]
  A --> C["src/components/navigation/sidebar-tree.tsx"]
"#;
    let render_pipeline = |pipeline| {
        let request = SvgRequest {
            environment: merman::SvgEnvironment::deterministic()
                .with_text_measurement_policy(policy.clone()),
            pipeline: Some(pipeline),
            ..Default::default()
        };
        let output = Renderer::new()
            .render(RenderRequest::svg(source, OperationControl::new(), request))
            .expect("long path diagram renders");
        let RenderOutput::Svg(Some(svg)) = output else {
            panic!("expected SVG output");
        };
        svg
    };
    let parity = render_pipeline(SvgPipeline::parity());
    let parity_document = roxmltree::Document::parse(parity.svg()).expect("valid parity SVG");
    let svg = render_pipeline(SvgPipeline::resvg_safe());
    let document = roxmltree::Document::parse(svg.svg()).expect("valid resvg-safe SVG");
    assert!(
        !document
            .descendants()
            .any(|node| node.has_tag_name("foreignObject"))
    );
    for expected_rows in [
        vec!["docs/plans/2026-10-11-", "release-", "checklist.final.md"],
        vec!["Short"],
        vec!["src/components/navigation/sidebar-", "tree.tsx"],
    ] {
        let label = expected_rows.concat();
        let source_label = parity_document
            .descendants()
            .find(|node| {
                node.has_tag_name("foreignObject")
                    && node
                        .descendants()
                        .filter(|child| child.is_text())
                        .filter_map(|child| child.text())
                        .collect::<String>()
                        == label
            })
            .expect("source HTML label");
        let source_node = source_label
            .ancestors()
            .find(|node| {
                node.attribute("class")
                    .is_some_and(|classes| classes.split_whitespace().any(|class| class == "node"))
            })
            .expect("source flowchart node");
        let node = document
            .descendants()
            .find(|node| node.attribute("id") == source_node.attribute("id"))
            .expect("converted flowchart node");
        let fallback = node
            .descendants()
            .find(|node| {
                node.attribute("data-merman-foreignobject") == Some("fallback")
                    && fallback_content(*node)
                        .descendants()
                        .filter(|child| child.has_tag_name("text"))
                        .filter_map(|child| child.text())
                        .collect::<String>()
                        == label
            })
            .expect("fallback preserves the label");
        assert!(
            fallback
                .attribute("data-merman-source-classes")
                .is_some_and(|classes| classes
                    .split_whitespace()
                    .any(|class| class == "nodeLabel"))
        );
        assert!(
            fallback
                .ancestors()
                .any(|ancestor| ancestor.attribute("id") == source_node.attribute("id"))
        );
        let content = fallback_content(fallback);
        let instance = fallback
            .children()
            .find(|child| child.has_tag_name("use"))
            .unwrap();
        for coordinate in ["x", "y"] {
            let expected = source_label
                .attribute(coordinate)
                .unwrap_or("0")
                .parse::<f64>()
                .unwrap();
            let actual = instance
                .attribute(coordinate)
                .unwrap()
                .parse::<f64>()
                .unwrap();
            assert_eq!(
                actual, expected,
                "fallback instance {coordinate} retains its source position"
            );
        }
        let viewport = content
            .children()
            .find(|child| child.has_tag_name("svg"))
            .unwrap();
        assert_eq!(viewport.attribute("x"), Some("0"));
        assert_eq!(viewport.attribute("y"), Some("0"));
        for dimension in ["width", "height"] {
            assert_eq!(
                viewport
                    .attribute(dimension)
                    .unwrap()
                    .parse::<f64>()
                    .unwrap(),
                source_label
                    .attribute(dimension)
                    .unwrap()
                    .parse::<f64>()
                    .unwrap()
            );
        }
        let rows: Vec<_> = content
            .descendants()
            .filter(|child| child.has_tag_name("text"))
            .map(|child| child.text().unwrap_or(""))
            .collect();
        assert_eq!(rows, expected_rows);
        let rectangle = node
            .children()
            .find(|child| child.has_tag_name("rect"))
            .expect("node rectangle");
        let width: f64 = rectangle.attribute("width").unwrap().parse().unwrap();
        let height: f64 = rectangle.attribute("height").unwrap().parse().unwrap();
        assert!(
            rows.iter()
                .all(|row| row.chars().count() as f64 * 10.0 < width)
        );
        assert!(rows.len() as f64 * 24.0 < height);
    }
}
