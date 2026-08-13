use merman::ascii::AsciiRenderOptions;
use merman::{AsciiRequest, OperationControl, RenderOutput, RenderRequest, Renderer};

const SOURCE: &str = r#"sequenceDiagram
    participant User
    participant Service
    User->>Service: Request terminal output
    Service-->>User: Rendered text
"#;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let options = if std::env::args().any(|arg| arg == "--ascii") {
        AsciiRenderOptions::ascii()
    } else {
        AsciiRenderOptions::unicode()
    };
    let renderer = Renderer::new().with_parse_options(merman::ParseOptions::strict());
    let output = renderer.render(RenderRequest::ascii(
        SOURCE,
        OperationControl::new(),
        AsciiRequest {
            options,
            ..Default::default()
        },
    ))?;
    let RenderOutput::Ascii(Some(text)) = output else {
        return Err("no Mermaid diagram detected".into());
    };

    print!("{text}");
    Ok(())
}
