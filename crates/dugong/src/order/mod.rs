//! Node ordering / crossing minimization.
//!
//! Ported from Dagre's `order` pipeline: barycenters, conflict resolution, and a sweep heuristic
//! that attempts to minimize edge crossings.

mod types;

// Mermaid's pinned Dagre builds layer matrices by writing nodes directly at `node.order`.
// Sparse slots are observable in crossing positions and final order assignment.
const EMPTY_LAYER_SLOT: usize = usize::MAX;

pub use types::{
    LayerGraphLabel, OrderEdgeWeight, OrderNodeLabel, OrderNodeRange, Relationship, WeightLabel,
};

mod layer_graph;
pub use layer_graph::build_layer_graph;

mod barycenter;
pub use barycenter::{
    BarycenterEntry, SortEntry, SortResult, barycenter, resolve_conflicts, sort, sort_subgraph,
};

mod constraints;
pub use constraints::add_subgraph_constraints;

mod init_order;
pub use init_order::init_order;

mod cross_count;
pub use cross_count::cross_count;

mod ordering;
pub(crate) use ordering::{
    IndexedLayerMatrix, build_current_layer_matrix_ix_controlled, order_with_layering_controlled,
};
pub use ordering::{OrderOptions, order};

mod workspace;
