use merman::{OperationControl, RenderOutput, RenderRequest, Renderer, SvgRequest};
use std::path::{Path, PathBuf};

const DIAGRAMS: [(&str, &str); 3] = [
    ("flowchart", "flowchart LR\n  Parse --> Layout --> Render\n"),
    (
        "sequence",
        "sequenceDiagram\n  Client->>Merman: render\n  Merman-->>Client: SVG\n",
    ),
    (
        "state",
        "stateDiagram-v2\n  [*] --> Ready\n  Ready --> [*]\n",
    ),
];

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let output_dir = std::env::args()
        .nth(1)
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("target/merman-render-many"));
    std::fs::create_dir_all(&output_dir)?;

    // Reuse one configured renderer when many independent files share the same policy.
    let renderer = Renderer::new().with_parse_options(merman::ParseOptions::strict());

    for (name, source) in DIAGRAMS {
        let output = renderer.render(RenderRequest::svg(
            source,
            OperationControl::new(),
            SvgRequest::default(),
        ))?;
        let RenderOutput::Svg(Some(svg)) = output else {
            return Err(format!("no Mermaid diagram detected for {name}").into());
        };
        write_svg(&output_dir, name, svg.svg())?;
    }

    Ok(())
}

fn write_svg(output_dir: &Path, name: &str, svg: &str) -> std::io::Result<()> {
    let path = output_dir.join(format!("{name}.svg"));
    std::fs::write(&path, svg)?;
    eprintln!("wrote {}", path.display());
    Ok(())
}
