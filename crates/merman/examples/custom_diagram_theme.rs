use merman::svg::SvgPipeline;
use merman::theme::{Appearance, BuiltinPreset, ThemeCompiler, ThemeSelection};
use merman::{OperationControl, RenderOutput, RenderRequest, Renderer, SvgRequest};

const SOURCE: &str = "flowchart LR\nA[Review] --> B[Publish]";
const DOCUMENT: &[u8] = br##"{
    "schema_version": 2,
    "extends": "studio",
    "seed": { "accent": "#5B8DEF" },
    "form": { "radius": 10 }
}"##;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let theme = match std::env::args().nth(1).as_deref() {
        None => ThemeSelection::compiled(
            ThemeCompiler::new().compile_json(DOCUMENT, Some(Appearance::Dark))?,
        ),
        Some(id) => ThemeSelection::preset(
            BuiltinPreset::from_id(id).ok_or_else(|| format!("unknown theme preset `{id}`"))?,
            None,
        ),
    };
    let mut request = SvgRequest::default().with_native_theme(theme);
    request.options.diagram_id = Some("custom-diagram-theme-example".to_owned());
    request.pipeline = Some(SvgPipeline::resvg_safe());
    let rendered =
        Renderer::new().render(RenderRequest::svg(SOURCE, OperationControl::new(), request))?;
    let RenderOutput::Svg(Some(svg)) = rendered else {
        return Err("no Mermaid diagram detected".into());
    };

    print!("{}", svg.svg());
    Ok(())
}
