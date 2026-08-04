use merman::svg::{
    CssOverridePolicy, HeadlessRenderer, RootBackgroundPostprocessor, ScopedCssPostprocessor,
    SvgPipeline,
};

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
    let renderer = HeadlessRenderer::new()
        .with_svg_pipeline(pipeline)
        .with_vendored_text_measurer()
        .with_diagram_id("custom-svg-pipeline-example");
    let Some(svg) = renderer.render_svg_sync(SOURCE)? else {
        return Err("no Mermaid diagram detected".into());
    };

    print!("{svg}");
    Ok(())
}
