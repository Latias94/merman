use merman::render_svg_with_id;

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

fn main() -> Result<(), merman::RenderSvgError> {
    // IDs must remain unique after normalization when the SVGs share one document.
    let flowchart = render_svg_with_id(FLOWCHART, "editor-flow")?;
    let sequence = render_svg_with_id(SEQUENCE, "render-sequence")?;

    print!("{HTML_START}{flowchart}{sequence}{HTML_END}");
    Ok(())
}
