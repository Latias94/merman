use merman::diagram_theme::{DiagramThemeCompiler, ThemeAppearance, ThemePreset};
use merman::svg::SvgPipeline;
use merman::{OperationControl, RenderOutput, RenderRequest, Renderer, SvgRequest};

const SOURCE: &str = r#"flowchart LR
    Source[Mermaid source] --> Theme[Built-in design]
    Theme --> Preview[Editor preview]
    Theme --> Export[Image export]
"#;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut args = std::env::args().skip(1);
    let preset_id = args.next();
    let appearance = args
        .next()
        .map(|value| value.parse::<ThemeAppearance>())
        .transpose()?;
    if args.next().is_some() {
        return Err("usage: theme_preset [preset-id [light|dark]]".into());
    }

    let compiler = DiagramThemeCompiler::new();
    let theme = match preset_id {
        Some(id) => compiler.compile_preset_selection(&id, appearance)?,
        // Direct typed selection uses the preset's authored default appearance.
        None => compiler.compile_preset(ThemePreset::PaperCut)?,
    };
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
