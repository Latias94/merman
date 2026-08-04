use merman::{Engine, ParseOptions};
use serde_json::json;

const SOURCE: &str = "flowchart TD\n  A[API] --> B[Semantic model]\n";

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let engine = Engine::new();
    let Some(parsed) = engine.parse_diagram_sync(SOURCE, ParseOptions::strict())? else {
        return Err("no Mermaid diagram detected".into());
    };

    // Keep routing metadata beside the model so a caller does not need to parse twice.
    let output = json!({
        "diagramType": parsed.meta.diagram_type,
        "title": parsed.meta.title,
        "model": parsed.model,
    });
    println!("{}", serde_json::to_string_pretty(&output)?);
    Ok(())
}
