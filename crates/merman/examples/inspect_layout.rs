use merman::svg::HeadlessRenderer;

const SOURCE: &str = "flowchart TD\n  A[Parse] --> B[Layout]\n  B --> C[Geometry]\n";

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let renderer = HeadlessRenderer::new().with_strict_parsing();
    let Some(layout) = renderer.layout_json_sync(SOURCE)? else {
        return Err("no Mermaid diagram detected".into());
    };

    println!("{}", serde_json::to_string_pretty(&layout)?);
    Ok(())
}
