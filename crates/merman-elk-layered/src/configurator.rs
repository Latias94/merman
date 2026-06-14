//! Layered graph configuration before processor assembly.
//!
//! Source references:
//! - `repo-ref/elk/plugins/org.eclipse.elk.alg.layered/src/org/eclipse/elk/alg/layered/GraphConfigurator.java`
//! - `repo-ref/elk/plugins/org.eclipse.elk.alg.layered/src/org/eclipse/elk/alg/layered/options/Spacings.java`

use crate::graph::LGraph;
use crate::options::{EdgeRouting, ElkDirection, LayeredOptions};

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
pub fn configure_graph_properties(graph: &mut LGraph) {
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

    for node in &mut graph.layerless_nodes {
        if let Some(nested_graph) = node.nested_graph.as_mut() {
            configure_graph_properties(nested_graph);
        }
    }
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
