use merman::{
    OperationControl, PrepareSourceRequest, RenderOutput, RenderTarget, Renderer, SvgRequest,
};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let renderer = Renderer::new();
    let Some(prepared) = renderer.prepare_source(PrepareSourceRequest::new(
        "gantt\ndateFormat YYYY-MM-DD\nsection Delivery\nRelease: release,2026-10-19,2d\n",
        OperationControl::new(),
    ))?
    else {
        return Ok(());
    };
    for width in [480.0, 960.0] {
        let mut request = SvgRequest::default();
        request.layout.container_width = width;
        let output = renderer.render_prepared(
            &prepared,
            RenderTarget::Svg(request),
            OperationControl::new(),
        )?;
        if let RenderOutput::Svg(Some(output)) = output {
            println!("width={width}: {} SVG bytes", output.svg().len());
        }
    }
    Ok(())
}
