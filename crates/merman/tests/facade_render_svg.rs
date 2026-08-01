#![cfg(feature = "svg")]

use merman::{RenderSvgError, render_svg, render_svg_with_id};

#[test]
fn one_shot_facade_renders_a_mermaid_diagram() {
    let svg = render_svg("flowchart TD\nA[Start] --> B[Done]").expect("diagram renders");

    assert!(svg.contains("<svg"));
    assert!(svg.contains("Start"));
    assert!(svg.contains("Done"));
}

#[test]
fn one_shot_facade_reports_non_mermaid_source_explicitly() {
    let error = render_svg("this is not a mermaid diagram definition")
        .expect_err("prose is not a Mermaid diagram");

    assert!(matches!(error, RenderSvgError::NoDiagram));
}

#[test]
fn one_shot_facade_preserves_detected_diagram_errors() {
    let error =
        render_svg("flowchart TD\nA -->").expect_err("invalid Mermaid must not be NoDiagram");

    assert!(matches!(error, RenderSvgError::Headless(_)));
}

#[test]
fn one_shot_facade_accepts_a_document_unique_diagram_id() {
    let first =
        render_svg_with_id("flowchart TD\nA --> B", "docs-first").expect("first diagram renders");
    let second =
        render_svg_with_id("flowchart TD\nA --> B", "docs second").expect("second diagram renders");

    assert!(first.contains(r#"id="docs-first""#));
    assert!(second.contains(r#"id="docs-second""#));
}
