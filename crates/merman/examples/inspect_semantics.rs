use merman::{Engine, ManagedSemanticJson, ParseOptions};
use serde_json::json;
use std::io::Write as _;

const SOURCE: &str = "flowchart TD\n  A[API] --> B[Semantic model]\n";

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let engine = Engine::new();
    let Some(parsed) = engine.parse_diagram_sync(SOURCE, ParseOptions::strict())? else {
        return Err("no Mermaid diagram detected".into());
    };

    // Keep routing metadata beside the model so a caller does not need to parse twice.
    let mut fields = serde_json::Map::from_iter([
        ("diagramType".to_owned(), json!(parsed.meta.diagram_type)),
        ("title".to_owned(), json!(parsed.meta.title)),
    ]);
    fields.insert("model".to_owned(), parsed.model.into_unmanaged_value());
    let output = ManagedSemanticJson::from(serde_json::Value::Object(fields));
    let mut stdout = std::io::stdout().lock();
    output.write_json_pretty(&mut stdout)?;
    writeln!(stdout)?;
    Ok(())
}
