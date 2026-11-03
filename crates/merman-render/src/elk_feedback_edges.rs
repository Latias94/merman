//! Mermaid 12.1 compound feedback-edge orientation around an immutable provider invocation.
//! Source: elk/subgraphFeedbackEdges.ts and elk/render.ts at 21f72f07ea22c0af48a3149c550654e80d8e40cb.

use crate::Result;
use crate::elk_hierarchy::{
    HierarchyIndex, charge_adapter_work, checked_adapter_add, checked_adapter_mul,
};
use crate::layout_work::OperationLayoutWorkControl;
use merman_layout_elk as elk;
use std::collections::HashMap;

/// Own the temporary provider orientation and restore semantic endpoints on every exit path.
/// Borrowing the graph exclusively prevents a prepared graph from escaping in its reversed state.
pub(crate) struct FeedbackOrientation<'a> {
    graph: &'a mut elk::Graph,
    reversed: Vec<usize>,
    order_swaps: Vec<(usize, usize)>,
}

impl FeedbackOrientation<'_> {
    pub(crate) fn graph(&self) -> &elk::Graph {
        self.graph
    }

    /// Restore provider routes to semantic direction before frame/shape projection. Labels are
    /// absolute geometry; their positions, terminal keys and TAIL/HEAD placement never swap.
    /// This matches upstream's endpoint-only reversal, including terminal-label ownership.
    pub(crate) fn restore(
        self,
        provider: &mut elk::LayoutResult,
        work: &mut Option<&mut OperationLayoutWorkControl>,
    ) -> Result<()> {
        if self.reversed.is_empty() {
            return Ok(());
        }
        charge_adapter_work(work, self.reversed.len())?;
        let reversed: std::collections::HashSet<_> = self
            .reversed
            .iter()
            .map(|&index| self.graph.edges[index].id.as_str())
            .collect();
        charge_adapter_work(work, provider.edges.len())?;
        let mut reversal_work = 0;
        for edge in &provider.edges {
            if reversed.contains(edge.id.as_str()) {
                reversal_work = checked_adapter_add(work, reversal_work, edge.points.len())?;
            }
        }
        // No provider geometry is changed until the complete reverse tranche has been admitted.
        charge_adapter_work(work, reversal_work)?;
        for edge in &mut provider.edges {
            if reversed.contains(edge.id.as_str()) {
                edge.points.reverse();
            }
        }
        Ok(())
    }
}

impl Drop for FeedbackOrientation<'_> {
    fn drop(&mut self) {
        // These bounded swaps were admitted before mutation. Cleanup must also run after the
        // operation's budget or cancellation has latched, so it cannot request additional work.
        for &index in &self.reversed {
            let edge = &mut self.graph.edges[index];
            std::mem::swap(&mut edge.source, &mut edge.target);
        }
        for &(left, right) in self.order_swaps.iter().rev() {
            self.graph.edges.swap(left, right);
        }
    }
}

/// Apply after semantic entry constraints and hierarchy policy, immediately before the provider.
/// On success consume the guard with `restore`; on an error Drop restores the graph automatically.
pub(crate) fn orient_feedback_edges<'a>(
    graph: &'a mut elk::Graph,
    config: &serde_json::Value,
    work: &mut Option<&mut OperationLayoutWorkControl>,
) -> Result<FeedbackOrientation<'a>> {
    let reversed = if crate::elk_options::orient_feedback_edges(config) {
        find_subgraph_feedback_edges(graph, work)?
    } else {
        Vec::new()
    };
    // Account for both the forward swaps and their infallible cleanup before touching the graph.
    charge_adapter_work(work, checked_adapter_mul(work, reversed.len(), 2)?)?;
    for &index in &reversed {
        let edge = &mut graph.edges[index];
        std::mem::swap(&mut edge.source, &mut edge.target);
    }
    let mut orientation = FeedbackOrientation {
        graph,
        reversed,
        order_swaps: Vec::new(),
    };
    let order = nested_edge_order(orientation.graph, work)?;
    charge_adapter_work(work, checked_adapter_mul(work, order.len(), 5)?)?;
    if order
        .iter()
        .enumerate()
        .all(|(index, original)| index == *original)
    {
        return Ok(orientation);
    }
    let mut original_at_position: Vec<_> = (0..order.len()).collect();
    let mut position_by_original = original_at_position.clone();
    orientation.order_swaps.reserve(order.len());
    for (target, original) in order.into_iter().enumerate() {
        let source = position_by_original[original];
        if source != target {
            orientation.graph.edges.swap(target, source);
            original_at_position.swap(target, source);
            position_by_original[original_at_position[target]] = target;
            position_by_original[original_at_position[source]] = source;
            orientation.order_swaps.push((target, source));
        }
    }
    for index in &mut orientation.reversed {
        *index = position_by_original[*index];
    }
    Ok(orientation)
}

fn nested_edge_order(
    graph: &elk::Graph,
    work: &mut Option<&mut OperationLayoutWorkControl>,
) -> Result<Vec<usize>> {
    charge_adapter_work(
        work,
        checked_adapter_add(work, graph.nodes.len(), graph.edges.len())?,
    )?;
    let mut order: Vec<_> = (0..graph.edges.len()).collect();
    if !graph.nodes.iter().any(|node| node.parent.is_some()) || graph.edges.len() < 2 {
        return Ok(order);
    }
    let hierarchy = HierarchyIndex::build(
        graph
            .nodes
            .iter()
            .map(|node| (node.id.as_str(), node.parent.as_deref())),
        graph.nodes.len(),
        work,
    )?;
    for (index, edge) in graph.edges.iter().enumerate() {
        charge_adapter_work(work, order.len())?;
        let from = order
            .iter()
            .position(|&original| original == index)
            .unwrap();
        let mut to = None;
        for (position, &earlier) in order[..from].iter().enumerate() {
            charge_adapter_work(work, 1)?;
            let earlier = &graph.edges[earlier];
            let source_inside = earlier.target == edge.target
                && edge.source != earlier.source
                && hierarchy.index_of(&earlier.source).is_some()
                && hierarchy.common_ancestor_index(&edge.source, &earlier.source, work)?
                    == hierarchy.index_of(&earlier.source);
            let target_inside = earlier.source == edge.source
                && edge.target != earlier.target
                && hierarchy.index_of(&earlier.target).is_some()
                && hierarchy.common_ancestor_index(&edge.target, &earlier.target, work)?
                    == hierarchy.index_of(&earlier.target);
            if source_inside || target_inside {
                to = Some(position);
                break;
            }
        }
        if let Some(to) = to {
            charge_adapter_work(work, from - to + 1)?;
            order[to..=from].rotate_right(1);
        }
    }
    Ok(order)
}

#[derive(Clone, Copy)]
struct LevelEdge {
    index: usize,
    from: usize,
    to: usize,
    collapsed: bool,
}

fn find_subgraph_feedback_edges(
    graph: &elk::Graph,
    work: &mut Option<&mut OperationLayoutWorkControl>,
) -> Result<Vec<usize>> {
    if graph.edges.is_empty() {
        return Ok(Vec::new());
    }
    charge_adapter_work(work, graph.nodes.len())?;
    if !graph.nodes.iter().any(|node| node.parent.is_some()) {
        return Ok(Vec::new());
    }
    let hierarchy = HierarchyIndex::build(
        graph
            .nodes
            .iter()
            .map(|node| (node.id.as_str(), node.parent.as_deref())),
        graph.nodes.len(),
        work,
    )?;
    charge_adapter_work(work, checked_adapter_mul(work, graph.edges.len(), 3)?)?;
    let mut levels = Vec::<Vec<LevelEdge>>::new();
    let mut level_indices = HashMap::new();
    let mut flags = vec![false; graph.edges.len()];
    for (index, edge) in graph.edges.iter().enumerate() {
        charge_adapter_work(work, 1)?;
        if edge.source == edge.target {
            continue;
        }
        let (Some(source), Some(target)) = (
            hierarchy.index_of(&edge.source),
            hierarchy.index_of(&edge.target),
        ) else {
            continue;
        };
        let ancestor = hierarchy.common_ancestor_index(&edge.source, &edge.target, work)?;
        if ancestor == Some(source) || ancestor == Some(target) {
            continue;
        }
        let from = hierarchy.child_on_path(source, ancestor, work)?;
        let to = hierarchy.child_on_path(target, ancestor, work)?;
        let next_level = levels.len();
        let level = *level_indices.entry(ancestor).or_insert_with(|| {
            levels.push(Vec::new());
            next_level
        });
        levels[level].push(LevelEdge {
            index,
            from,
            to,
            collapsed: from != source || to != target,
        });
    }

    for level in levels {
        // At most two level-local vertices per edge; do not allocate a full graph-sized array
        // for every hierarchy level. Across all levels retained adjacency stays O(E).
        charge_adapter_work(work, checked_adapter_mul(work, level.len(), 8)?)?;
        let mut local_index = HashMap::new();
        let mut successors: Vec<Vec<LevelEdge>> = Vec::new();
        let mut indegree: Vec<usize> = Vec::new();
        for mut edge in level {
            for endpoint in [&mut edge.from, &mut edge.to] {
                let next = successors.len();
                *endpoint = *local_index.entry(*endpoint).or_insert_with(|| {
                    successors.push(Vec::new());
                    indegree.push(0);
                    next
                });
            }
            indegree[edge.to] = checked_adapter_add(work, indegree[edge.to], 1)?;
            successors[edge.from].push(edge);
        }
        let mut state = vec![0u8; successors.len()];
        let mut stack = Vec::with_capacity(successors.len());
        // Source-first DFS, then unsourced components, preserving first edge occurrence order.
        for root in (0..successors.len())
            .filter(|&node| indegree[node] == 0)
            .chain(0..successors.len())
        {
            charge_adapter_work(work, 1)?;
            if state[root] != 0 {
                continue;
            }
            state[root] = 1;
            stack.push((root, 0usize));
            while let Some((node, next)) = stack.last_mut() {
                charge_adapter_work(work, 1)?;
                if *next == successors[*node].len() {
                    state[*node] = 2;
                    stack.pop();
                    continue;
                }
                let edge = successors[*node][*next];
                *next += 1;
                if state[edge.to] == 1 {
                    flags[edge.index] = edge.collapsed;
                } else if state[edge.to] == 0 {
                    state[edge.to] = 1;
                    stack.push((edge.to, 0));
                }
            }
        }
    }
    charge_adapter_work(work, flags.len())?;
    Ok(flags
        .into_iter()
        .enumerate()
        .filter_map(|(index, reversed)| reversed.then_some(index))
        .collect())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::resources::{OperationWorkMeter, RenderResourcePolicy};
    use std::sync::Arc;

    fn graph(parents: &[(&str, &str)], pairs: &[(&str, &str)]) -> elk::Graph {
        let mut ids = Vec::new();
        for &(source, target) in pairs.iter().chain(parents) {
            for id in [source, target] {
                if !ids.contains(&id) {
                    ids.push(id);
                }
            }
        }
        elk::Graph {
            nodes: ids
                .into_iter()
                .map(|id| elk::Node {
                    id: id.to_owned(),
                    kind: if parents.iter().any(|(_, parent)| *parent == id) {
                        elk::NodeKind::Group
                    } else {
                        elk::NodeKind::Leaf
                    },
                    parent: parents
                        .iter()
                        .find_map(|(child, parent)| (*child == id).then(|| (*parent).to_owned())),
                    label_text: None,
                    container: Default::default(),
                    width: 40.0,
                    height: 20.0,
                    direction: None,
                    hierarchy_handling: None,
                    layer_constraint: None,
                    port_alignment: None,
                    label: None,
                })
                .collect(),
            edges: pairs
                .iter()
                .enumerate()
                .map(|(index, (source, target))| elk::Edge {
                    id: format!("e{index}"),
                    source: (*source).to_owned(),
                    target: (*target).to_owned(),
                    label: None,
                    terminal_labels: Vec::new(),
                    minlen: 1,
                    inside_self_loops_yo: false,
                })
                .collect(),
            ..Default::default()
        }
    }

    fn work() -> OperationLayoutWorkControl {
        OperationLayoutWorkControl::new(Arc::new(OperationWorkMeter::new(
            RenderResourcePolicy::unbounded_for_trusted_input(),
        )))
    }

    fn limited_work(limit: usize) -> OperationLayoutWorkControl {
        OperationLayoutWorkControl::new(Arc::new(OperationWorkMeter::new(
            RenderResourcePolicy::unbounded_for_trusted_input()
                .with_limit(crate::ResourceLimitId::MaxLayoutWorkUnits, limit)
                .unwrap(),
        )))
    }

    #[test]
    fn nested_edge_order_matches_mermaid_12_3_source_cases() {
        for (parents, pairs, expected) in [
            (
                vec![("da", "inner"), ("inner", "middle"), ("middle", "outer")],
                vec![("outer", "a"), ("da", "a")],
                vec![1, 0],
            ),
            (
                vec![("da", "outer")],
                vec![("a", "outer"), ("a", "da")],
                vec![1, 0],
            ),
            (
                vec![("b1", "two"), ("c2", "three")],
                vec![("three", "two"), ("c2", "two")],
                vec![1, 0],
            ),
            (
                vec![("da", "outer")],
                vec![("da", "a"), ("outer", "a")],
                vec![0, 1],
            ),
            (
                vec![("da", "outer")],
                vec![("outer", "a"), ("da", "b"), ("x", "outer")],
                vec![0, 1, 2],
            ),
            (
                vec![("a1", "A"), ("b1", "B")],
                vec![("A", "t"), ("b1", "t")],
                vec![0, 1],
            ),
            (
                vec![("s1", "S")],
                vec![("x", "y"), ("S", "t"), ("x", "z"), ("s1", "t"), ("S", "u")],
                vec![0, 3, 1, 2, 4],
            ),
        ] {
            let mut graph = graph(&parents, &pairs);
            let original = graph.clone();
            assert_eq!(
                nested_edge_order(&graph, &mut Some(&mut work())).unwrap(),
                expected
            );
            let guard = orient_feedback_edges(
                &mut graph,
                &serde_json::json!({"elk":{"orientFeedbackEdges":false}}),
                &mut Some(&mut work()),
            )
            .unwrap();
            assert_eq!(
                guard
                    .graph()
                    .edges
                    .iter()
                    .map(|edge| edge.id.clone())
                    .collect::<Vec<_>>(),
                expected
                    .iter()
                    .map(|index| format!("e{index}"))
                    .collect::<Vec<_>>()
            );
            drop(guard);
            assert_eq!(graph, original);
        }
    }

    #[test]
    fn nested_order_budget_rejections_restore_semantic_edges() {
        let scene = graph(
            &[("inner", "outer"), ("a", "inner")],
            &[("outer", "target"), ("a", "target")],
        );
        let mut measured = work();
        let mut input = scene.clone();
        drop(
            orient_feedback_edges(&mut input, &serde_json::json!({}), &mut Some(&mut measured))
                .unwrap(),
        );
        assert_eq!(input, scene);
        for limit in 1..measured.adapter_work() {
            let mut input = scene.clone();
            assert!(
                orient_feedback_edges(
                    &mut input,
                    &serde_json::json!({}),
                    &mut Some(&mut limited_work(limit))
                )
                .is_err()
            );
            assert_eq!(input, scene, "limit {limit}");
        }
    }

    #[test]
    fn feedback_flags_match_pinned_subgraph_cycles_and_source_first_dfs() {
        let parents = [("A", "G"), ("B", "G"), ("C", "G")];
        for (parents, pairs, expected) in [
            (
                parents.as_slice(),
                vec![("S", "A"), ("A", "B"), ("B", "X"), ("X", "C")],
                vec![3],
            ),
            (&[], vec![("A", "B"), ("B", "C"), ("C", "A")], vec![]),
            (parents.as_slice(), vec![("A", "B"), ("B", "A")], vec![]),
            (
                parents.as_slice(),
                vec![("S", "A"), ("A", "B"), ("B", "X"), ("S", "C")],
                vec![],
            ),
            (
                parents.as_slice(),
                vec![("G", "A"), ("A", "G"), ("A", "A")],
                vec![],
            ),
            (
                parents.as_slice(),
                vec![("S", "X"), ("X", "B"), ("A", "Y"), ("Y", "X")],
                vec![],
            ),
        ] {
            assert_eq!(
                find_subgraph_feedback_edges(&graph(parents, &pairs), &mut Some(&mut work()))
                    .unwrap(),
                expected
            );
        }
    }

    #[test]
    fn feedback_flags_are_scoped_to_lowest_common_subgraph_and_declaration_order() {
        let nested = graph(
            &[("A", "G"), ("B", "G"), ("G", "OUTER"), ("Y", "OUTER")],
            &[("S", "A"), ("A", "Y"), ("Y", "B")],
        );
        assert_eq!(
            find_subgraph_feedback_edges(&nested, &mut Some(&mut work())).unwrap(),
            [2]
        );
        for pairs in [[("A", "C"), ("D", "B")], [("D", "B"), ("A", "C")]] {
            let mut input = graph(&[("A", "G"), ("B", "G"), ("C", "H"), ("D", "H")], &pairs);
            assert_eq!(
                find_subgraph_feedback_edges(&input, &mut Some(&mut work())).unwrap(),
                [1]
            );
            input.nodes.reverse();
            assert_eq!(
                find_subgraph_feedback_edges(&input, &mut Some(&mut work())).unwrap(),
                [1]
            );
        }
    }

    fn feedback_scene() -> (elk::Graph, elk::LayoutResult) {
        let mut graph = graph(
            &[("A", "G"), ("B", "G"), ("C", "H"), ("D", "H")],
            &[("A", "C"), ("D", "B")],
        );
        for edge in &mut graph.edges {
            edge.terminal_labels = elk::TerminalLabelKey::ALL
                .into_iter()
                .map(|key| elk::TerminalLabel {
                    key,
                    label: elk::Label {
                        width: 20.0,
                        height: 10.0,
                    },
                })
                .collect();
        }
        let provider = elk::LayoutResult {
            nodes: Vec::new(),
            edges: graph
                .edges
                .iter()
                .rev()
                .map(|edge| elk::EdgeLayout {
                    id: edge.id.clone(),
                    points: vec![
                        elk::Point { x: 10.0, y: 20.0 },
                        elk::Point { x: 30.0, y: 40.0 },
                    ],
                    labels: std::iter::once(None)
                        .chain(elk::TerminalLabelKey::ALL.into_iter().map(Some))
                        .enumerate()
                        .map(|(index, terminal)| elk::EdgeLabelLayout {
                            x: 50.0 + index as f64,
                            y: 60.0 + index as f64,
                            width: 70.0,
                            height: 80.0,
                            terminal,
                        })
                        .collect(),
                })
                .collect(),
        };
        (graph, provider)
    }

    #[test]
    fn orientation_defaults_on_and_restores_graph_routes_and_label_ownership() {
        for config in [
            serde_json::json!({}),
            serde_json::json!({"elk":{"orientFeedbackEdges":true}}),
        ] {
            let (mut graph, mut provider) = feedback_scene();
            let original_graph = graph.clone();
            let original_provider = provider.clone();
            let guard = orient_feedback_edges(&mut graph, &config, &mut Some(&mut work())).unwrap();
            assert_eq!(guard.graph().edges[1].source, "B");
            assert_eq!(guard.graph().edges[1].target, "D");
            assert_eq!(
                guard.graph().edges[1].terminal_labels,
                original_graph.edges[1].terminal_labels
            );
            guard
                .restore(&mut provider, &mut Some(&mut work()))
                .unwrap();
            assert_eq!(graph, original_graph);
            assert_eq!(
                provider.edges[0].points[0],
                original_provider.edges[0].points[1]
            );
            assert_eq!(
                provider.edges[0].points[1],
                original_provider.edges[0].points[0]
            );
            assert_eq!(provider.edges[0].labels, original_provider.edges[0].labels);
            assert_eq!(provider.edges[1], original_provider.edges[1]);
        }
    }

    #[test]
    fn disabled_feedback_option_preserves_legacy_graph_and_provider_routing() {
        let (mut graph, mut provider) = feedback_scene();
        let original_graph = graph.clone();
        let original_provider = provider.clone();
        let mut measured = work();
        let guard = orient_feedback_edges(
            &mut graph,
            &serde_json::json!({"elk":{"orientFeedbackEdges":false}}),
            &mut Some(&mut measured),
        )
        .unwrap();
        assert_eq!(guard.graph(), &original_graph);
        guard
            .restore(&mut provider, &mut Some(&mut measured))
            .unwrap();
        assert_eq!(graph, original_graph);
        assert_eq!(provider, original_provider);
        assert!(measured.adapter_work() > 0);
    }

    #[test]
    fn feedback_guard_restores_graph_on_provider_failure_and_restore_budget_rejection() {
        let (mut graph, mut provider) = feedback_scene();
        let original_graph = graph.clone();
        let original_provider = provider.clone();
        {
            let guard =
                orient_feedback_edges(&mut graph, &serde_json::json!({}), &mut Some(&mut work()))
                    .unwrap();
            assert_ne!(guard.graph(), &original_graph);
            // Dropping on a provider error takes exactly this same cleanup path.
        }
        assert_eq!(graph, original_graph);
        let guard =
            orient_feedback_edges(&mut graph, &serde_json::json!({}), &mut Some(&mut work()))
                .unwrap();
        assert!(matches!(
            guard.restore(&mut provider, &mut Some(&mut limited_work(1))),
            Err(crate::Error::ResourceLimitExceeded(_))
        ));
        assert_eq!(graph, original_graph);
        assert_eq!(provider, original_provider);
        assert!(matches!(
            orient_feedback_edges(
                &mut graph,
                &serde_json::json!({}),
                &mut Some(&mut limited_work(1))
            ),
            Err(crate::Error::ResourceLimitExceeded(_))
        ));
        assert_eq!(graph, original_graph);
    }

    #[test]
    fn feedback_restore_cancellation_restores_graph_without_reversing_output() {
        let (mut graph, mut provider) = feedback_scene();
        let original_graph = graph.clone();
        let original_provider = provider.clone();
        let guard =
            orient_feedback_edges(&mut graph, &serde_json::json!({}), &mut Some(&mut work()))
                .unwrap();
        let control = merman_model::OperationControl::new();
        control.cancel();
        let mut cancelled =
            OperationLayoutWorkControl::new(Arc::new(OperationWorkMeter::new_with_control(
                RenderResourcePolicy::unbounded_for_trusted_input(),
                control,
            )));
        assert!(matches!(
            guard.restore(&mut provider, &mut Some(&mut cancelled)),
            Err(crate::Error::Cancelled(_))
        ));
        assert_eq!(graph, original_graph);
        assert_eq!(provider, original_provider);
    }

    #[test]
    fn feedback_queries_remain_metered_and_nonrecursive_in_deep_hierarchies() {
        let depth = 4096;
        let mut graph = graph(&[("A", "G0"), ("B", "G0")], &[("A", "X"), ("X", "B")]);
        for index in 0..depth {
            let id = format!("G{index}");
            let parent = (index + 1 < depth).then(|| format!("G{}", index + 1));
            if index == 0 {
                graph
                    .nodes
                    .iter_mut()
                    .find(|node| node.id == id)
                    .unwrap()
                    .parent = parent;
            } else {
                graph.nodes.push(elk::Node {
                    id,
                    parent,
                    kind: elk::NodeKind::Group,
                    label_text: None,
                    container: Default::default(),
                    width: 0.0,
                    height: 0.0,
                    direction: None,
                    hierarchy_handling: None,
                    layer_constraint: None,
                    port_alignment: None,
                    label: None,
                });
            }
        }
        let mut measured = work();
        assert_eq!(
            find_subgraph_feedback_edges(&graph, &mut Some(&mut measured)).unwrap(),
            [1]
        );
        assert!(measured.adapter_work() < 12 * graph.nodes.len());
    }
}
