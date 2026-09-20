use super::tests::{edge, graph, node};
use super::{LayeringError, layer_with_strategy};
use crate::graph::{LGraph, LNode};
use crate::options::{ElkDirection, LayeredOptions, LayeringStrategy};
use crate::pipeline::{PipelineError, execute_ported_processors};
use crate::work::{NoopWorkControl, WorkControl, WorkError};

const STRATEGIES: [LayeringStrategy; 6] = [
    LayeringStrategy::LongestPath,
    LayeringStrategy::LongestPathSource,
    LayeringStrategy::CoffmanGraham,
    LayeringStrategy::Interactive,
    LayeringStrategy::MinWidth,
    LayeringStrategy::StretchWidth,
];

struct Budget {
    remaining: usize,
    charges: usize,
    cancel_after: Option<usize>,
}

impl WorkControl for Budget {
    fn check(&mut self, units: usize) -> Result<(), WorkError> {
        if units > self.remaining || self.cancel_after.is_some_and(|limit| self.charges >= limit) {
            Err(WorkError::Interrupted)
        } else {
            Ok(())
        }
    }
    fn charge(&mut self, units: usize) -> Result<(), WorkError> {
        self.check(units)?;
        self.remaining -= units;
        self.charges += 1;
        Ok(())
    }
}

fn sample() -> LGraph {
    graph(
        (0..6).map(|index| node(&index.to_string())).collect(),
        vec![
            edge("a", "0", "2"),
            edge("b", "1", "2"),
            edge("c", "2", "3"),
            edge("d", "2", "4"),
            edge("e", "4", "5"),
        ],
    )
}

#[test]
fn layerers_admit_work_before_mutating_and_keep_interruptions_typed() {
    for strategy in STRATEGIES {
        for (remaining, cancel_after) in [(0, None), (usize::MAX, Some(3))] {
            let mut graph = sample();
            graph.set_node_layer(0, 0);
            let before = graph.clone();
            let mut budget = Budget {
                remaining,
                charges: 0,
                cancel_after,
            };
            let error = layer_with_strategy(&mut graph, strategy, &mut budget).unwrap_err();
            assert_eq!(
                error,
                LayeringError::Work(WorkError::Interrupted),
                "{strategy:?}"
            );
            assert_eq!(
                PipelineError::from(error),
                PipelineError::Work(WorkError::Interrupted)
            );
            assert_eq!(graph, before, "{strategy:?} published a partial layering");
        }
    }
}

#[test]
fn all_six_layerers_execute_the_complete_pipeline_with_self_loops() {
    for strategy in STRATEGIES {
        let mut graph = graph(
            vec![node("A"), node("B"), node("C")],
            vec![
                edge("ab", "A", "B"),
                edge("bc", "B", "C"),
                edge("bb", "B", "B"),
            ],
        );
        graph.options.layering_strategy = strategy;
        graph.options.layering_coffman_graham_layer_bound = 2;
        let processors = execute_ported_processors(&mut graph)
            .unwrap_or_else(|error| panic!("{strategy:?}: {error}"));
        assert!(!processors.is_empty());
        let nodes = graph
            .layerless_nodes
            .iter()
            .filter(|node| ["A", "B", "C"].contains(&node.id.as_str()))
            .collect::<Vec<_>>();
        assert_eq!(nodes.len(), 3);
        for node in &nodes {
            assert!(node.position.x.is_finite() && node.position.y.is_finite());
            assert!(node.size.width > 0.0 && node.size.height > 0.0);
        }
        // HierarchicalNodeResizer deliberately discards transient layers after publishing
        // geometry. The public result is a top-to-bottom placement for this DOWN graph.
        let by_id = |id| nodes.iter().find(|node| node.id == id).unwrap();
        assert!(by_id("A").position.y + by_id("A").size.height <= by_id("B").position.y);
        assert!(by_id("B").position.y + by_id("B").size.height <= by_id("C").position.y);
    }
}

#[test]
fn empty_graphs_have_no_layers_for_every_added_strategy() {
    for strategy in STRATEGIES {
        let mut graph = LGraph::new("root", LayeredOptions::default());
        layer_with_strategy(&mut graph, strategy, &mut NoopWorkControl).unwrap();
        assert!(graph.layers.is_empty());
    }
}

#[test]
fn cyclic_raw_input_cannot_enter_an_unbounded_layering_search() {
    for strategy in STRATEGIES {
        let mut graph = graph(
            vec![node("A"), node("B")],
            vec![edge("ab", "A", "B"), edge("ba", "B", "A")],
        );
        let before = graph.clone();
        assert_eq!(
            layer_with_strategy(&mut graph, strategy, &mut NoopWorkControl),
            Err(LayeringError::CyclicGraph),
            "{strategy:?}"
        );
        assert_eq!(graph, before);
    }
}

#[test]
fn zero_normal_height_rejects_stretch_widths_nonterminating_source_case() {
    let mut graph = LGraph::new("root", LayeredOptions::default());
    graph
        .layerless_nodes
        .push(LNode::new("zero", 20.0, 0.0, None));
    graph
        .layerless_nodes
        .push(LNode::new("positive", 20.0, 30.0, None));
    let before = graph.clone();
    assert_eq!(
        layer_with_strategy(
            &mut graph,
            LayeringStrategy::StretchWidth,
            &mut NoopWorkControl
        ),
        Err(LayeringError::InvalidStretchWidthDimensions)
    );
    assert_eq!(graph, before);
}

#[test]
fn longest_path_uses_an_explicit_stack_for_deep_graphs() {
    std::thread::Builder::new()
        .stack_size(64 * 1024)
        .spawn(|| {
            const COUNT: usize = 4_096;
            let nodes = (0..COUNT).map(|index| node(&index.to_string())).collect();
            let edges = (1..COUNT)
                .map(|index| {
                    edge(
                        &format!("e{index}"),
                        &(index - 1).to_string(),
                        &index.to_string(),
                    )
                })
                .collect();
            let mut graph = graph(nodes, edges);
            graph.options.direction = ElkDirection::Right;
            for strategy in [
                LayeringStrategy::LongestPath,
                LayeringStrategy::LongestPathSource,
            ] {
                layer_with_strategy(&mut graph, strategy, &mut NoopWorkControl).unwrap();
                assert_eq!(graph.layers.len(), COUNT);
                assert_eq!(graph.layerless_nodes[0].layer_index, Some(0));
                assert_eq!(
                    graph.layerless_nodes[COUNT - 1].layer_index,
                    Some(COUNT - 1)
                );
            }
        })
        .unwrap()
        .join()
        .unwrap();
}
