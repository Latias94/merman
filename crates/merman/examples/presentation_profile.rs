use merman::svg::{HostTheme, HostThemePreset, Presentation, PresentationProfile, SvgPipeline};
use merman::{OperationControl, RenderOutput, RenderRequest, Renderer, SvgRequest};

const SOURCE: &str = r#"flowchart LR
    Source[Mermaid source] --> Profile[Merman Modern]
    Profile --> Theme[One Dark]
    Theme --> Preview[Editor preview]
"#;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let presentation = Presentation::new()
        .with_profile(PresentationProfile::MermanModern)
        .with_theme(HostTheme::from_preset(HostThemePreset::OneDark));
    let resolved = presentation.resolve();
    let renderer = Renderer::new().with_engine(resolved.materialize_engine(merman::Engine::new()));
    let request = SvgRequest {
        pipeline: Some(SvgPipeline::resvg_safe()),
        presentation: resolved.render_policy(),
        options: merman::svg::SvgRenderOptions {
            diagram_id: Some("presentation-profile-example".to_string()),
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
