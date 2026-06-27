use super::super::super::model::AsciiGraphEdge;

pub(super) fn parallel_edge_index(edges: &[AsciiGraphEdge], edge_index: usize) -> usize {
    let Some(edge) = edges.get(edge_index) else {
        return 0;
    };
    edges[..edge_index]
        .iter()
        .filter(|previous| same_edge_pair(previous, edge))
        .count()
}

fn same_edge_pair(left: &AsciiGraphEdge, right: &AsciiGraphEdge) -> bool {
    (left.from == right.from && left.to == right.to)
        || (left.from == right.to && left.to == right.from)
}
