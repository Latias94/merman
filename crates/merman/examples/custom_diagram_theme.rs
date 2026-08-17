use merman::svg::theme_contract::{ThemeColorTokenV1, ThemeDefinitionV1, ThemeTokensV1};
use merman::svg::{
    CssOverridePolicy, DiagramThemeCompiler, SvgOutputPolicy, SvgPipelinePreset, ThemeMaterializer,
};
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
            .with_color(ThemeColorTokenV1::Canvas, "#0f172a")
            .with_color(ThemeColorTokenV1::Surface, "#111827")
            .with_color(ThemeColorTokenV1::SurfaceAlt, "#1f2937")
            .with_color(ThemeColorTokenV1::SurfaceMuted, "#334155")
            .with_color(ThemeColorTokenV1::Text, "#e5e7eb")
            .with_color(ThemeColorTokenV1::SubtleText, "#cbd5e1")
            .with_color(ThemeColorTokenV1::Border, "#475569")
            .with_color(ThemeColorTokenV1::Line, "#94a3b8")
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
