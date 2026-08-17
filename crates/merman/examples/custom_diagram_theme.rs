use merman::diagram_theme::{
    DiagramThemeCompiler, ThemeDefinitionV1, ThemeMaterializer, ThemeTokensV1,
};
use merman::svg::{CssOverridePolicy, SvgOutputPolicy, SvgPipelinePreset};
use merman::{OperationControl, RenderOutput, RenderRequest, Renderer, SvgRequest};

const SOURCE: &str = r#"sequenceDiagram
    participant Host
    participant Merman
    Host->>Merman: Render preview
    Note over Host,Merman: Typed diagram theme
"#;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let definition = ThemeDefinitionV1::new(
        ThemeTokensV1::default()
            .with_canvas("#0f172a")
            .with_surface("#111827")
            .with_surface_alt("#1f2937")
            .with_surface_muted("#334155")
            .with_text("#e5e7eb")
            .with_subtle_text("#cbd5e1")
            .with_border("#475569")
            .with_line("#94a3b8")
            .with_series(
                ["#60a5fa", "#34d399", "#f59e0b"]
                    .map(str::to_owned)
                    .to_vec(),
            ),
    );
    let materialized = ThemeMaterializer::new().materialize_theme(&definition)?;
    let theme = DiagramThemeCompiler::new().compile_spec_wire(materialized.into_spec())?;
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
