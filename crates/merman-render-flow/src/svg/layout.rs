use std::collections::HashMap;

use merman_render_core::{
    LayoutCluster, LayoutEdge, LayoutNode, SwimlaneDirection, SwimlaneEdgeLayout,
    SwimlaneLaneLayout, SwimlaneNodeLayout,
};
use rustc_hash::FxHashSet;

use crate::projection::FlowchartEdgeOwners;

#[cfg(feature = "canonical-edge-paint")]
use crate::graph::AxisRoutes;
#[cfg(not(feature = "canonical-edge-paint"))]
type AxisRoutes = std::convert::Infallible;

/// The geometry and occurrence ownership retained by the selected layout provider.
#[derive(Clone, Copy)]
pub struct FlowchartSvgLayout<'a> {
    pub nodes: &'a [LayoutNode],
    pub edges: &'a [LayoutEdge],
    pub edge_owners: &'a FlowchartEdgeOwners,
    pub clusters: &'a [LayoutCluster],
    pub dom_node_order_by_root: &'a HashMap<String, Vec<String>>,
    pub route_contract: FlowchartSvgRoute<'a>,
}

#[derive(Clone, Copy)]
pub enum FlowchartSvgRoute<'a> {
    DagreParity,
    ElkParity,
    Swimlane,
    CanonicalAxis(&'a AxisRoutes),
    Legacy,
}

impl<'a> FlowchartSvgRoute<'a> {
    pub const fn is_canonical_axis(self) -> bool {
        matches!(self, Self::CanonicalAxis(_))
    }

    pub const fn is_elk_parity(self) -> bool {
        matches!(self, Self::ElkParity)
    }

    pub const fn uses_projected_edges(self) -> bool {
        self.is_elk_parity() || self.is_canonical_axis()
    }

    pub fn group_parents(self) -> Option<&'a [Option<u32>]> {
        #[cfg(feature = "canonical-edge-paint")]
        if let Self::CanonicalAxis(routes) = self {
            return Some(&routes.group_parents);
        }
        None
    }

    pub fn struck_group_titles(self) -> Option<&'a FxHashSet<String>> {
        #[cfg(feature = "canonical-edge-paint")]
        if let Self::CanonicalAxis(routes) = self {
            return Some(&routes.struck_group_titles);
        }
        None
    }

    #[cfg(feature = "layout-shoal")]
    pub fn flowchart_svg_edges(self) -> Option<&'a [crate::graph::FlowchartSvgEdgeGeometry]> {
        match self {
            Self::CanonicalAxis(routes) => routes.svg_edges.as_deref(),
            _ => None,
        }
    }
}

/// Swimlane-specific geometry used alongside a projected Flow layout.
#[derive(Clone, Copy)]
pub struct SwimlaneSvgLayout<'a> {
    pub direction: SwimlaneDirection,
    pub nodes: &'a [SwimlaneNodeLayout],
    pub lanes: &'a [SwimlaneLaneLayout],
    pub edges: &'a [SwimlaneEdgeLayout],
    pub edge_owners: &'a FlowchartEdgeOwners,
    pub has_canonical_routes: bool,
}
