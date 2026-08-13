use merman::{OperationControl, RenderOutput, RenderRequest, Renderer, SvgRequest};

const SOURCE: &str = "flowchart TD\n  A[Parse] --> B[Layout]\n  B --> C[Geometry]\n";

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let renderer = Renderer::new().with_parse_options(merman::ParseOptions::strict());
    let output = renderer.render(RenderRequest::layout_json(
        SOURCE,
        OperationControl::new(),
        SvgRequest::default(),
    ))?;
    let RenderOutput::LayoutJson(Some(layout)) = output else {
        return Err("no Mermaid diagram detected".into());
    };

    println!("{}", serde_json::to_string_pretty(layout.layout())?);
    Ok(())
}
