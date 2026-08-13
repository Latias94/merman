use merman::{OperationControl, RenderOutput, RenderRequest, Renderer, SvgRequest};

const FLOWCHART: &str = r#"flowchart LR
    Edit --> Preview --> Export
"#;

const SEQUENCE: &str = r#"sequenceDiagram
    participant UI
    participant Engine
    UI->>Engine: render
    Engine-->>UI: SVG
"#;

const HTML_START: &str = r#"<!doctype html>
<html lang="en">
<head>
  <meta charset="utf-8">
  <title>Merman diagrams</title>
  <style>body { font-family: system-ui; display: grid; gap: 2rem; padding: 2rem; }</style>
</head>
<body>
"#;

const HTML_END: &str = "</body>\n</html>\n";

fn main() -> Result<(), Box<dyn std::error::Error>> {
    // IDs must remain unique after normalization when the SVGs share one document.
    let renderer = Renderer::new();
    let flowchart = render(&renderer, FLOWCHART, "editor-flow")?;
    let sequence = render(&renderer, SEQUENCE, "render-sequence")?;

    print!("{HTML_START}{flowchart}{sequence}{HTML_END}");
    Ok(())
}

fn render(
    renderer: &Renderer,
    source: &str,
    diagram_id: &str,
) -> Result<String, Box<dyn std::error::Error>> {
    let request = SvgRequest {
        options: merman::svg::SvgRenderOptions {
            diagram_id: Some(merman::svg::sanitize_svg_id(diagram_id)),
            ..Default::default()
        },
        ..Default::default()
    };
    let output = renderer.render(RenderRequest::svg(source, OperationControl::new(), request))?;
    let RenderOutput::Svg(Some(svg)) = output else {
        return Err("no Mermaid diagram detected".into());
    };
    Ok(svg.svg().to_string())
}
