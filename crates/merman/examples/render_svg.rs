use merman::render_svg;

const SOURCE: &str = r#"flowchart LR
    Source[Mermaid source] --> Merman --> Svg[SVG string]
"#;

fn main() -> Result<(), merman::RenderSvgError> {
    let svg = render_svg(SOURCE)?;
    print!("{svg}");
    Ok(())
}
