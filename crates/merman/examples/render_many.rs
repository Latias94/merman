use merman::svg::HeadlessRenderer;
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
    let renderer = HeadlessRenderer::new()
        .with_strict_parsing()
        .with_vendored_text_measurer();

    for (name, source) in DIAGRAMS {
        let Some(svg) = renderer.render_svg_sync(source)? else {
            return Err(format!("no Mermaid diagram detected for {name}").into());
        };
        write_svg(&output_dir, name, &svg)?;
    }

    Ok(())
}

fn write_svg(output_dir: &Path, name: &str, svg: &str) -> std::io::Result<()> {
    let path = output_dir.join(format!("{name}.svg"));
    std::fs::write(&path, svg)?;
    eprintln!("wrote {}", path.display());
    Ok(())
}
