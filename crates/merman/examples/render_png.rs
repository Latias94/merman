use merman::svg::{
    HeadlessRenderer,
    export::{RasterFitBox, RasterOptions},
};
use std::path::PathBuf;

const SOURCE: &str = r#"flowchart TD
    Source[Mermaid source] --> Svg[resvg-safe SVG]
    Svg --> Png[Bounded PNG]
"#;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let output = std::env::args()
        .nth(1)
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("target/merman-example.png"));

    // Bound the final pixel allocation instead of trusting an arbitrary SVG viewBox.
    let raster = RasterOptions::default()
        .with_fit_to(RasterFitBox::contain(960, 540))
        .with_scale(2.0)
        .with_background("white");
    let renderer = HeadlessRenderer::new()
        .with_strict_parsing()
        .with_diagram_id("png-example");
    let Some(bytes) = renderer.render_png_sync(SOURCE, &raster)? else {
        return Err("no Mermaid diagram detected".into());
    };

    if let Some(parent) = output
        .parent()
        .filter(|parent| !parent.as_os_str().is_empty())
    {
        std::fs::create_dir_all(parent)?;
    }
    std::fs::write(&output, bytes)?;
    eprintln!("wrote {}", output.display());
    Ok(())
}
