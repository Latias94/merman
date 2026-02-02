use crate::algo::CoseBilkentOptions;
use crate::error::Result;
use crate::graph::{Graph, LayoutResult, Point};

pub fn layout(graph: &Graph, _opts: &CoseBilkentOptions) -> Result<LayoutResult> {
    graph.validate()?;

    // Temporary deterministic fallback: preserve sorted-by-id order and place nodes on a line.
    // This is a scaffold; it will be replaced by the COSE-Bilkent port.
    let mut positions: std::collections::BTreeMap<String, Point> =
        std::collections::BTreeMap::new();
    let mut ids = graph
        .nodes
        .iter()
        .map(|n| n.id.as_str())
        .collect::<Vec<_>>();
    ids.sort();
    for (idx, id) in ids.iter().enumerate() {
        positions.insert(
            (*id).to_string(),
            Point {
                x: idx as f64 * 100.0,
                y: 0.0,
            },
        );
    }

    Ok(LayoutResult { positions })
}
