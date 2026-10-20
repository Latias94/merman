use merman::diagram_theme::{DiagramThemeCompiler, ThemeDesignV1, ThemeRecipeV1};
use merman::svg::{CssOverridePolicy, SvgOutputPolicy, SvgPipelinePreset};
use merman::{OperationControl, RenderOutput, RenderRequest, Renderer, SvgRequest};

const SOURCE: &str = r#"sequenceDiagram
    participant Host
    participant Merman
    Host->>Merman: Render preview
    Note over Host,Merman: Typed diagram theme
"#;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let design: ThemeDesignV1 = serde_json::from_value(serde_json::json!({
        "tokens": {
            "color.ink": {"type":"color","value":"#e5e7eb","label":"Text"},
            "paint.surface": {"type":"paint","value":"#111827","label":"Surface"},
            "stroke.line": {"type":"stroke","value":{"paint":"#94a3b8","width":2}}
        },
        "canvas": {"base":"#0f172a"},
        "styles": {
            "actor": {"fill":{"ref":"paint.surface"},"stroke":{"ref":"stroke.line"}},
            "actor-label": {"fill":{"kind":"solid","color":{"ref":"color.ink"}}},
            "text": {"fill":{"kind":"solid","color":{"ref":"color.ink"}}},
            "message": {"stroke":{"ref":"stroke.line"}},
            "note": {"fill":"#1f2937","stroke":{"ref":"stroke.line"}}
        }
    }))?;
    let theme = DiagramThemeCompiler::new().compile_recipe(ThemeRecipeV1::Design { design })?;
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
