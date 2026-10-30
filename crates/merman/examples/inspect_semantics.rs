use merman::SemanticParser;
use merman::model::SemanticDiagramModel;

const SOURCE: &str = "flowchart TD\n  A[API] --> B[Semantic model]\n";

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let parsed = SemanticParser::new().parse_diagram_sync(SOURCE)?;
    println!("diagram: {}", parsed.metadata.diagram_type);
    let SemanticDiagramModel::Flowchart(model) = parsed.model else {
        return Err("expected Flowchart meaning".into());
    };
    for node in &model.nodes {
        println!("node: {}", node.id);
    }
    for edge in &model.edges {
        println!("edge: {} -> {}", edge.from, edge.to);
    }
    Ok(())
}
