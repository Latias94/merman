use merman::ascii::{AsciiRenderOptions, HeadlessAsciiRenderer};

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
    let renderer = HeadlessAsciiRenderer::new()
        .with_strict_parsing()
        .with_ascii_options(options);
    let Some(text) = renderer.render_ascii_sync(SOURCE)? else {
        return Err("no Mermaid diagram detected".into());
    };

    print!("{text}");
    Ok(())
}
