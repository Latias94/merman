use merman::svg::export::{RasterFitBox, RasterOptions};
use merman::{OperationControl, PngRequest, RenderOutput, RenderRequest, Renderer, SvgRequest};
use std::path::PathBuf;

const SOURCE: &str = r#"flowchart TD
    Source[Mermaid source] --> Svg[resvg-safe SVG]
    Svg --> Png[Bounded PNG]
"#;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let output_path = std::env::args()
        .nth(1)
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("target/merman-example.png"));

    // Bound the final pixel allocation instead of trusting an arbitrary SVG viewBox.
    let raster = RasterOptions::default()
        .with_fit_to(RasterFitBox::contain(960, 540))
        .with_scale(2.0)
        .with_background("white");
    let renderer = Renderer::new().with_parse_options(merman::ParseOptions::strict());
    let output = renderer.render(RenderRequest::png(
        SOURCE,
        OperationControl::new(),
        PngRequest {
            svg: SvgRequest {
                options: merman::svg::SvgRenderOptions {
                    diagram_id: Some("png-example".to_string()),
                    ..Default::default()
                },
                ..Default::default()
            },
            options: raster,
        },
    ))?;
    let RenderOutput::Png(Some(output)) = output else {
        return Err("no Mermaid diagram detected".into());
    };

    if let Some(parent) = output_path
        .parent()
        .filter(|parent| !parent.as_os_str().is_empty())
    {
        std::fs::create_dir_all(parent)?;
    }
    std::fs::write(&output_path, output.bytes)?;
    eprintln!("wrote {}", output_path.display());
    Ok(())
}
