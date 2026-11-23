use merman_layout_shoal::{Direction, EdgeSpec, Graph, NodeId, NodeSpec, Options, Size, layout};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut graph = Graph::default();
    graph.direction = Direction::Right;
    let stage = Size {
        width: 96.0,
        height: 36.0,
    };
    graph
        .nodes
        .extend((0..3).map(|seq| NodeSpec::new(stage, seq)));
    for (source, target) in [(0, 1), (1, 2), (0, 2)] {
        let mut edge = EdgeSpec::new(NodeId(source).into(), NodeId(target).into());
        edge.end_marker.length = 8.0;
        graph.edges.push(edge);
    }
    graph.edges[2].label = Some(Size {
        width: 54.0,
        height: 18.0,
    });

    let mut options = Options::default();
    options.spacing.node = 24.0;
    options.spacing.rank = 48.0;
    let drawing = layout(&graph, &options)?;
    for (name, rect) in ["Receive", "Review", "Archive"].iter().zip(drawing.nodes) {
        println!(
            "{name} at ({}, {}), {} × {}",
            rect.x, rect.y, rect.width, rect.height
        );
    }
    for (spec, route) in graph.edges.iter().zip(drawing.edges) {
        println!(
            "{:?} -> {:?} via {:?} on {:?}",
            spec.source, spec.target, route.ports, route.points
        );
        if let Some(center) = route.label_center {
            println!("  label at ({}, {})", center.x, center.y);
        }
    }
    println!("Drawing bounds {:?}", drawing.bounds);
    Ok(())
}
