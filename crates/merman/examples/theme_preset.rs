use merman::svg::{DiagramThemeCompiler, HeadlessRenderer, SvgPipeline, ThemePreset};

const SOURCE: &str = r#"flowchart LR
    Source[Mermaid source] --> Theme[One Dark]
    Theme --> Preview[Editor preview]
"#;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let theme = DiagramThemeCompiler::new().compile_preset(ThemePreset::OneDark)?;
    let renderer = HeadlessRenderer::new()
        .with_theme(theme)
        .with_svg_pipeline(SvgPipeline::resvg_safe())
        .with_vendored_text_measurer()
        .with_diagram_id("theme-preset-example");
    let Some(svg) = renderer.render_svg_sync(SOURCE)? else {
        return Err("no Mermaid diagram detected".into());
    };

    print!("{svg}");
    Ok(())
}
