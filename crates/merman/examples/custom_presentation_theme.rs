use merman::svg::{
    CssOverridePolicy, HostTheme, HostThemeAppearance, Presentation, SvgOutputPolicy,
    SvgPipelinePreset, ThemeRole,
};
use merman::{OperationControl, RenderOutput, RenderRequest, Renderer, SvgRequest};

const SOURCE: &str = r#"sequenceDiagram
    participant Host
    participant Merman
    Host->>Merman: Render preview
    Note over Host,Merman: Semantic host theme
"#;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let theme = HostTheme::new()
        .with_appearance(HostThemeAppearance::Dark)
        .try_with_font_family("Inter, system-ui, sans-serif")?
        .try_with_role(ThemeRole::Canvas, "#0f172a")?
        .try_with_role(ThemeRole::Surface, "#111827")?
        .try_with_role(ThemeRole::SurfaceAlt, "#1f2937")?
        .try_with_role(ThemeRole::Text, "#e5e7eb")?
        .try_with_role(ThemeRole::Border, "#475569")?
        .try_with_role(ThemeRole::Line, "#94a3b8")?
        .try_with_role(ThemeRole::NoteBackground, "#422006")?
        .try_with_role(ThemeRole::NoteBorder, "#f59e0b")?
        .try_with_role(ThemeRole::NoteText, "#fef3c7")?
        .try_with_series_palette(["#60a5fa", "#34d399", "#f59e0b"])?;
    let output = SvgOutputPolicy {
        preset: SvgPipelinePreset::ResvgSafe,
        css_override_policy: CssOverridePolicy::StripExistingImportant,
        root_background_color: Some("#0f172a".to_string()),
        ..SvgOutputPolicy::default()
    };
    let resolved = Presentation::new().with_theme(theme).resolve();
    let renderer = Renderer::new().with_engine(resolved.materialize_engine(merman::Engine::new()));
    let request = SvgRequest {
        pipeline: Some(output.pipeline()),
        presentation: resolved.render_policy(),
        options: merman::svg::SvgRenderOptions {
            diagram_id: Some("custom-presentation-theme-example".to_string()),
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
