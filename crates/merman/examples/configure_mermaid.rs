use merman::{MermaidConfig, svg::HeadlessRenderer};

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
    let renderer = HeadlessRenderer::new()
        .with_site_config(site_config)
        .with_strict_parsing()
        .with_diagram_id("configured-mermaid-example");
    let Some(svg) = renderer.render_svg_sync(SOURCE)? else {
        return Err("no Mermaid diagram detected".into());
    };

    print!("{svg}");
    Ok(())
}
