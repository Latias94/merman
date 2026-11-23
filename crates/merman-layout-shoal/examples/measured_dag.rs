use merman_layout_shoal::{Direction, Edge, Graph, NodeId, Size, layout};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let stage = Size {
        width: 96.0,
        height: 36.0,
    };
    let measured_label = Size {
        width: 54.0,
        height: 18.0,
    };
    let edge = |source, target, label| Edge {
        source: NodeId(source),
        target: NodeId(target),
        min_rank_span: 1,
        label,
        start_clearance: 0.0,
        end_clearance: 8.0,
    };
    let graph = Graph {
        direction: Direction::Right,
        node_spacing: 24.0,
        rank_spacing: 48.0,
        nodes: vec![stage; 3],
        edges: vec![
            edge(0, 1, None),
            edge(1, 2, None),
            edge(0, 2, Some(measured_label)),
        ],
    };

    let drawing = layout(&graph)?;
    for (name, center) in ["Receive", "Review", "Archive"]
        .iter()
        .zip(&drawing.centers)
    {
        println!("{name} at ({}, {})", center.x, center.y);
    }
    for (edge, route) in graph.edges.iter().zip(&drawing.edges) {
        println!("{} -> {}  {:?}", edge.source.0, edge.target.0, route.points);
        if let Some(anchor) = route.label_center {
            println!("  label at ({}, {})", anchor.x, anchor.y);
        }
    }
    println!("Drawing bounds {:?}", drawing.bounds);
    Ok(())
}
