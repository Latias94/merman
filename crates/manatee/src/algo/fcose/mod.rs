use crate::algo::FcoseOptions;
use crate::error::Result;
use crate::graph::{Graph, LayoutResult, Point};

pub fn layout(graph: &Graph, _opts: &FcoseOptions) -> Result<LayoutResult> {
    graph.validate()?;

    // Temporary deterministic fallback: preserve input order and place nodes on a line.
    // This is a scaffold; it will be replaced by the FCoSE port.
    let mut positions: std::collections::BTreeMap<String, Point> =
        std::collections::BTreeMap::new();
    for (idx, n) in graph.nodes.iter().enumerate() {
        positions.insert(
            n.id.clone(),
            Point {
                x: idx as f64 * 100.0,
                y: 0.0,
            },
        );
    }
    Ok(LayoutResult { positions })
}
