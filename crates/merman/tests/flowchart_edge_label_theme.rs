#![cfg(all(feature = "svg", feature = "diagram-flowchart"))]

use merman::theme::ThemeMode;
use merman::{OperationControl, RenderOutput, RenderRequest, Renderer, SvgRequest};

fn edge_label_text_content(svg: &str) -> Vec<String> {
    let document = roxmltree::Document::parse(svg).expect("valid Flowchart SVG XML");
    document
        .descendants()
        .filter(|node| {
            node.has_tag_name("text")
                && node.ancestors().any(|ancestor| {
                    ancestor.has_tag_name("g")
                        && ancestor.attribute("class").is_some_and(|classes| {
                            classes
                                .split_ascii_whitespace()
                                .any(|class| class == "edgeLabel")
                        })
                })
        })
        .map(|node| {
            node.descendants()
                .filter(|part| part.is_text())
                .filter_map(|part| part.text())
                .collect()
        })
        .collect()
}

#[test]
fn unthemed_edge_labels_keep_the_legacy_svg_path() {
    let RenderOutput::Svg(Some(output)) = Renderer::new()
        .render(RenderRequest::svg(
            r#"---
config:
  htmlLabels: false
  flowchart:
    htmlLabels: false
---
flowchart LR
A -->|legacy edge label| B
"#,
            OperationControl::new(),
            SvgRequest::default(),
        ))
        .expect("unthemed Flowchart should render")
    else {
        panic!("expected a Flowchart SVG");
    };

    assert_eq!(output.theme_outcome().mode, ThemeMode::MermaidParity);
    assert!(
        !output.svg().contains("merman-prepared-"),
        "{}",
        output.svg()
    );
    assert!(
        edge_label_text_content(output.svg())
            .iter()
            .any(|text| text == "legacy edge label"),
        "legacy path should retain the authored edge label: {}",
        output.svg(),
    );
}
