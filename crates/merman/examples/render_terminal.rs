use merman::ascii::{AsciiOutputOutcome, AsciiRenderOptions, AsciiViewportPolicy, OverflowPolicy};
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
            viewport: AsciiViewportPolicy::with_max_width(80).overflow(OverflowPolicy::Fallback),
            ..Default::default()
        },
    ))?;
    let RenderOutput::Ascii(Some(report)) = output else {
        return Err("no Mermaid diagram detected".into());
    };

    if matches!(report.outcome, AsciiOutputOutcome::Fallback) {
        eprintln!(
            "selected typed fallback: {}×{} cells",
            report.emitted_extent.width, report.emitted_extent.height
        );
    }
    print!("{}", report.text);
    Ok(())
}
