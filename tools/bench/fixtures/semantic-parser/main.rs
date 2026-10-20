use merman_core::{Engine, ParseOptions, RenderSemanticModel};
use std::io::{self, Write};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let input = std::env::args()
        .nth(1)
        .ok_or("expected a Mermaid source path or --families")?;
    if input == "--families" {
        for family in merman_core::diagram_family_capabilities() {
            if family.has_semantic_parser && family.logical_family_kind != "error" {
                println!("{}", family.logical_family_kind);
            }
        }
        return Ok(());
    }
    let source = std::fs::read_to_string(input)?;
    let parsed = Engine::new()
        .parse_diagram_for_render_model_sync(&source, ParseOptions::strict())?
        .ok_or("expected a parsed diagram")?;
    let mut output = io::BufWriter::new(io::stdout().lock());
    // Consume runtime-fed canonical fields directly. Compatibility JSON would pull an
    // unrelated projection into the executable whose closure this fixture measures.
    match parsed.model() {
        RenderSemanticModel::Flowchart(model) => {
            writeln!(output, "flowchart")?;
            for node in &model.nodes {
                writeln!(output, "node\t{:?}\t{:?}", node.id, node.label)?;
            }
            for edge in &model.edges {
                writeln!(
                    output,
                    "edge\t{:?}\t{:?}\t{:?}",
                    edge.from, edge.to, edge.label
                )?;
            }
        }
        RenderSemanticModel::Sequence(model) => {
            writeln!(output, "sequence")?;
            for id in &model.actor_order {
                let actor = model.actors.get(id).ok_or("missing ordered actor")?;
                writeln!(output, "actor\t{:?}\t{:?}", id, actor.description)?;
            }
            for message in &model.messages {
                writeln!(
                    output,
                    "message\t{:?}\t{:?}\t{}\t{:?}",
                    message.from,
                    message.to,
                    message.message_type,
                    message.message_text()
                )?;
            }
        }
        RenderSemanticModel::State(model) => {
            writeln!(output, "state")?;
            for node in &model.nodes {
                writeln!(output, "node\t{:?}\t{:?}", node.id, node.description)?;
            }
            for edge in &model.edges {
                writeln!(
                    output,
                    "edge\t{:?}\t{:?}\t{:?}",
                    edge.start, edge.end, edge.label
                )?;
            }
        }
        _ => return Err("expected Flowchart, Sequence or State".into()),
    }
    output.flush()?;
    Ok(())
}
