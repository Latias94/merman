//! Layered graph configuration before processor assembly.
//!
//! Source references:
//! - https://github.com/eclipse-elk/elk/blob/62d5909f96fad541bc101ad52dabaece6b7eab7e/plugins/org.eclipse.elk.alg.layered/src/org/eclipse/elk/alg/layered/GraphConfigurator.java
//! - https://github.com/eclipse-elk/elk/blob/62d5909f96fad541bc101ad52dabaece6b7eab7e/plugins/org.eclipse.elk.alg.layered/src/org/eclipse/elk/alg/layered/options/Spacings.java

use crate::graph::LGraph;
use crate::options::{EdgeRouting, ElkDirection, LayeredOptions};
use crate::random::{JavaRandom, RandomSeedError};

const MIN_EDGE_SPACING: f64 = 2.0;

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct LayeredSpacings {
    pub edge_edge: f64,
}

impl Default for LayeredSpacings {
    fn default() -> Self {
        Self {
            edge_edge: MIN_EDGE_SPACING,
        }
    }
}

/// Configure graph properties before assembling processors.
///
/// This mirrors the non-mutating Rust subset of `GraphConfigurator.configureGraphProperties(...)`:
/// keep discovered graph properties synchronized with options, normalize undefined direction, enforce
/// minimum edge-edge spacing, and set the default straight-edge preference based on edge routing.
pub fn configure_graph_properties(graph: &mut LGraph) -> Result<(), RandomSeedError> {
    graph.sync_graph_properties_to_options();
    if graph.options.direction == ElkDirection::Undefined {
        graph.options.direction = ElkDirection::Right;
    }
    if graph.options.spacing.edge_edge < MIN_EDGE_SPACING {
        graph.options.spacing.edge_edge = MIN_EDGE_SPACING;
    }
    if graph.options.node_placement_favor_straight_edges.is_none() {
        graph.options.node_placement_favor_straight_edges =
            Some(graph.options.edge_routing == EdgeRouting::Orthogonal);
    }
    graph.random = JavaRandom::new(graph.resolve_random_seed_for_configuration()?);

    for node in &mut graph.layerless_nodes {
        if let Some(nested_graph) = node.nested_graph.as_mut() {
            configure_graph_properties(nested_graph)?;
        }
    }
    Ok(())
}

pub fn configured_options(graph: &LGraph) -> LayeredOptions {
    let mut options = graph.options.clone();
    graph.graph_properties.apply_to_options(&mut options);
    if options.direction == ElkDirection::Undefined {
        options.direction = ElkDirection::Right;
    }
    if options.spacing.edge_edge < MIN_EDGE_SPACING {
        options.spacing.edge_edge = MIN_EDGE_SPACING;
    }
    if options.node_placement_favor_straight_edges.is_none() {
        options.node_placement_favor_straight_edges =
            Some(options.edge_routing == EdgeRouting::Orthogonal);
    }
    options
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{LGraph, RandomSeedPolicy};

    #[test]
    fn raw_graph_with_upstream_zero_requires_an_explicit_policy() {
        let options = LayeredOptions {
            random_seed: 0,
            ..Default::default()
        };
        let mut graph = LGraph::new("root", options);

        assert_eq!(
            configure_graph_properties(&mut graph),
            Err(RandomSeedError::Unresolved {
                graph_path: "root".to_string(),
            })
        );
    }

    #[test]
    fn deterministic_policy_keeps_zero_config_and_reseeds_each_configuration() {
        let options = LayeredOptions {
            random_seed: 0,
            ..Default::default()
        };
        let policy = RandomSeedPolicy::deterministic(0x4d45_524d_414e);

        let mut first = LGraph::new("root", options.clone()).with_random_seed_policy(policy);
        configure_graph_properties(&mut first).unwrap();
        let first_invocation = first.random.clone().next_long();
        configure_graph_properties(&mut first).unwrap();
        let second_invocation = first.random.clone().next_long();

        let mut replayed = LGraph::new("root", options).with_random_seed_policy(policy);
        configure_graph_properties(&mut replayed).unwrap();
        let replayed_first = replayed.random.clone().next_long();
        configure_graph_properties(&mut replayed).unwrap();
        let replayed_second = replayed.random.clone().next_long();

        assert_eq!(first.options.random_seed, 0);
        assert_ne!(first_invocation, second_invocation);
        assert_eq!(first_invocation, replayed_first);
        assert_eq!(second_invocation, replayed_second);
    }
}
