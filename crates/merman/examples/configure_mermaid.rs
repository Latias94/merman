use merman::{
    Engine, MermaidConfig, OperationControl, RenderOutput, RenderRequest, Renderer, SvgRequest,
};

const SOURCE: &str = r#"flowchart TD
    Host[Host-owned defaults] --> Diagram[Mermaid source]
    Diagram --> Svg[Configured SVG]
"#;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Keep application defaults outside user-authored Mermaid source.
    let site_config = MermaidConfig::from_value(serde_json::json!({
        "theme": "base",
        "fontFamily": "system-ui",
        "themeVariables": {
            "primaryColor": "#e0f2fe",
            "primaryBorderColor": "#0284c7",
            "primaryTextColor": "#0f172a",
            "lineColor": "#16a34a"
        }
    }));
    let renderer = Renderer::new()
        .with_engine(Engine::new().with_site_config(site_config))
        .with_parse_options(merman::ParseOptions::strict());
    let request = SvgRequest {
        options: merman::svg::SvgRenderOptions {
            diagram_id: Some("configured-mermaid-example".to_string()),
            ..Default::default()
        },
        ..Default::default()
    };
    let output = renderer.render(RenderRequest::svg(SOURCE, OperationControl::new(), request))?;
    let RenderOutput::Svg(Some(svg)) = output else {
        return Err("no Mermaid diagram detected".into());
    };

    print!("{}", svg.svg());
    Ok(())
}
