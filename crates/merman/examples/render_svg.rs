use merman::{OperationControl, RenderOutput, RenderRequest, Renderer, SvgRequest};

const SOURCE: &str = r#"flowchart LR
    Source[Mermaid source] --> Merman --> Svg[SVG string]
"#;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let output = Renderer::new().render(RenderRequest::svg(
        SOURCE,
        OperationControl::new(),
        SvgRequest::default(),
    ))?;
    let RenderOutput::Svg(Some(svg)) = output else {
        return Err("no Mermaid diagram detected".into());
    };
    print!("{}", svg.svg());
    Ok(())
}
