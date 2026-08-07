use merman::svg::{HeadlessRenderer, HostTheme, HostThemePreset, PresentationProfile, SvgPipeline};

const SOURCE: &str = r#"flowchart LR
    Source[Mermaid source] --> Profile[Merman Modern]
    Profile --> Theme[One Dark]
    Theme --> Preview[Editor preview]
"#;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let renderer = HeadlessRenderer::new()
        .with_presentation_profile(PresentationProfile::MermanModern)
        .with_host_theme(HostTheme::from_preset(HostThemePreset::OneDark))
        .with_svg_pipeline(SvgPipeline::resvg_safe())
        .with_vendored_text_measurer()
        .with_diagram_id("presentation-profile-example");
    let Some(svg) = renderer.render_svg_sync(SOURCE)? else {
        return Err("no Mermaid diagram detected".into());
    };

    print!("{svg}");
    Ok(())
}
