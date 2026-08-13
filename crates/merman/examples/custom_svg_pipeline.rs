use merman::svg::{
    CssOverridePolicy, RootBackgroundPostprocessor, ScopedCssPostprocessor, SvgPipeline,
};
use merman::{OperationControl, RenderOutput, RenderRequest, Renderer, SvgRequest};

const SOURCE: &str = r#"flowchart TD
    Mermaid --> Pipeline[Host output pipeline]
    Pipeline --> Preview[resvg-safe preview]
"#;

const HOST_CSS: &str = r#"
.node rect,
.node polygon,
.node path {
  stroke: #38bdf8;
  stroke-width: 2px;
}
.merman-foreignobject-fallback-text {
  fill: #e5e7eb;
}
"#;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let pipeline = SvgPipeline::resvg_safe()
        .with_postprocessor(RootBackgroundPostprocessor::new("#111827"))
        .with_postprocessor(
            ScopedCssPostprocessor::new(HOST_CSS)
                .with_override_policy(CssOverridePolicy::StripExistingImportant),
        );
    let renderer = Renderer::new();
    let request = SvgRequest {
        pipeline: Some(pipeline),
        options: merman::svg::SvgRenderOptions {
            diagram_id: Some("custom-svg-pipeline-example".to_string()),
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
