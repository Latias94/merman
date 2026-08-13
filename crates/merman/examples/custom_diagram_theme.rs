use merman::svg::{
    CssOverridePolicy, DiagramThemeCompiler, SvgOutputPolicy, SvgPipelinePreset, ThemeTokens,
};
use merman::{OperationControl, RenderOutput, RenderRequest, Renderer, SvgRequest};

const SOURCE: &str = r#"sequenceDiagram
    participant Host
    participant Merman
    Host->>Merman: Render preview
    Note over Host,Merman: Typed diagram theme
"#;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let spec = ThemeTokens::default()
        .with_canvas("#0f172a")?
        .with_surface("#111827")?
        .with_surface_alt("#1f2937")?
        .with_text("#e5e7eb")?
        .with_subtle_text("#cbd5e1")?
        .with_border("#475569")?
        .with_line("#94a3b8")?
        .with_note_background("#422006")?
        .with_note_border("#a16207")?
        .with_note_text("#fef3c7")?
        .with_actor_background("#1f2937")?
        .with_actor_border("#64748b")?
        .with_actor_text("#e5e7eb")?
        .with_activation_background("#334155")?
        .with_activation_border("#94a3b8")?
        .with_series(["#60a5fa", "#34d399", "#f59e0b"])?
        .into_theme_spec();
    let theme = DiagramThemeCompiler::new().compile(spec)?;
    let output = SvgOutputPolicy {
        preset: SvgPipelinePreset::ResvgSafe,
        css_override_policy: CssOverridePolicy::StripExistingImportant,
        root_background_color: Some("#0f172a".to_string()),
        ..SvgOutputPolicy::default()
    };
    let mut request = SvgRequest::default();
    request.options.diagram_id = Some("custom-diagram-theme-example".to_owned());
    request.pipeline = Some(output.pipeline());
    let rendered = Renderer::new()
        .render(RenderRequest::svg(SOURCE, OperationControl::new(), request).with_theme(theme))?;
    let RenderOutput::Svg(Some(svg)) = rendered else {
        return Err("no Mermaid diagram detected".into());
    };

    print!("{}", svg.svg());
    Ok(())
}
