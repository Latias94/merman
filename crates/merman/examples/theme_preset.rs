use merman::svg::{DiagramThemeCompiler, SvgPipeline, ThemePreset};
use merman::{OperationControl, RenderOutput, RenderRequest, Renderer, SvgRequest};

const SOURCE: &str = r#"flowchart LR
    Source[Mermaid source] --> Theme[One Dark]
    Theme --> Preview[Editor preview]
"#;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let theme = DiagramThemeCompiler::new().compile_preset(ThemePreset::OneDark)?;
    let mut request = SvgRequest::default();
    request.options.diagram_id = Some("theme-preset-example".to_owned());
    request.pipeline = Some(SvgPipeline::resvg_safe());
    let output = Renderer::new()
        .render(RenderRequest::svg(SOURCE, OperationControl::new(), request).with_theme(theme))?;
    let RenderOutput::Svg(Some(svg)) = output else {
        return Err("no Mermaid diagram detected".into());
    };

    print!("{}", svg.svg());
    Ok(())
}
