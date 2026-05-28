use merman::ascii::{AsciiRenderOptions, HeadlessAsciiRenderer};
use std::io::Read;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut input = String::new();
    std::io::stdin().read_to_string(&mut input)?;
    if input.trim().is_empty() {
        input = r#"sequenceDiagram
    participant User
    participant System
    User->>System: Request ASCII output
    System-->>User: Rendered text
"#
        .to_string();
    }

    let options = if std::env::args().any(|arg| arg == "--ascii") {
        AsciiRenderOptions::ascii()
    } else {
        AsciiRenderOptions::unicode()
    };
    let renderer = HeadlessAsciiRenderer::new()
        .with_strict_parsing()
        .with_ascii_options(options);
    let Some(text) = renderer.render_ascii_sync(&input)? else {
        return Err("no Mermaid diagram detected".into());
    };

    print!("{text}");
    Ok(())
}
