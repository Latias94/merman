use merman_core::SemanticParser;
use merman_model::SemanticDiagramModel;
use std::collections::BTreeSet;
use std::io::{self, Write};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let input = std::env::args()
        .nth(1)
        .ok_or("expected a Mermaid source path or --families")?;
    if input == "--families" {
        let families: BTreeSet<_> = merman_core::diagram_family_capabilities()
            .iter()
            .filter(|family| family.has_semantic_parser && family.logical_family_kind != "error")
            .map(|family| family.logical_family_kind)
            .collect();
        for family in families {
            println!("{family}");
        }
        return Ok(());
    }
    let source = std::fs::read_to_string(input)?;
    let parsed = SemanticParser::new().parse_diagram_sync(&source)?;
    let mut output = io::BufWriter::new(io::stdout().lock());
    // Consume runtime-fed canonical fields directly. Compatibility JSON would pull an
    // unrelated projection into the executable whose closure this fixture measures.
    match &parsed.model {
        SemanticDiagramModel::Flowchart(model) => {
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
        SemanticDiagramModel::Sequence(model) => {
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
        SemanticDiagramModel::State(model) => {
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
