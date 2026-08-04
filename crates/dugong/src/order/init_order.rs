use super::OrderNodeRange;
use crate::graphlib::Graph;
use rustc_hash::FxHashSet as HashSet;

pub fn init_order<N, E, G>(g: &Graph<N, E, G>) -> Vec<Vec<String>>
where
    N: Default + OrderNodeRange + 'static,
    E: Default + 'static,
    G: Default,
{
    let mut visited: HashSet<String> = HashSet::default();

    let simple_nodes: Vec<String> = g
        .nodes()
        .filter(|v| g.children_iter(v).next().is_none())
        .map(|v| v.to_string())
        .collect();

    let mut max_rank: i32 = i32::MIN;
    for v in &simple_nodes {
        let Some(rank) = g.node(v).and_then(|n| n.rank()) else {
            continue;
        };
        max_rank = max_rank.max(rank);
    }

    if max_rank == i32::MIN {
        return Vec::new();
    }

    let mut layers: Vec<Vec<String>> = vec![Vec::new(); (max_rank + 1).max(0) as usize];

    let mut ordered_vs = simple_nodes;

    // `simple_nodes` already follows pinned Graphlib's JavaScript object-key enumeration.
    // Rust's stable `sort_by_key` therefore preserves the official relative order within a rank
    // without a second ID vector or an auxiliary String-keyed position map.
    ordered_vs.sort_by_key(|v| g.node(v).and_then(|n| n.rank()).unwrap_or(i32::MAX));
    for v in ordered_vs {
        let mut stack = vec![v];
        while let Some(v) = stack.pop() {
            if !visited.insert(v.clone()) {
                continue;
            }

            let Some(rank) = g.node(&v).and_then(|n| n.rank()) else {
                continue;
            };
            let idx = rank.max(0) as usize;
            if let Some(layer) = layers.get_mut(idx) {
                layer.push(v.clone());
            }

            let successors = g.successors(&v);
            for w in successors.into_iter().rev() {
                stack.push(w.to_string());
            }
        }
    }

    layers
}
