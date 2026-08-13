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
