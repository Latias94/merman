use std::collections::HashMap;

use merman_render_core::{
    LayoutCluster, LayoutEdge, LayoutNode, SwimlaneDirection, SwimlaneEdgeLayout,
    SwimlaneLaneLayout, SwimlaneNodeLayout,
};
use rustc_hash::FxHashSet;

#[cfg(feature = "layout-grid")]
pub use merman_render_core::graph_layout::mermaid_grid::{
    GridLineHops, GridPaint as FlowchartGridPaint,
};

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
    #[cfg(all(feature = "layout-layered", feature = "layout-dagre"))]
    LayeredDagreFallback,
    ElkParity,
    Swimlane,
    /// Original Grid points and label anchors, in materialized layout edges keyed by occurrence.
    #[cfg(feature = "layout-grid")]
    Grid(FlowchartGridPaint),
    #[cfg(feature = "layout-layered")]
    Layered(&'a crate::layered::FlowLayeredRoutes),
    CanonicalAxis(&'a AxisRoutes),
    Legacy,
}

/// The DOM structures selected by Flow's layout adapters.
#[derive(Clone, Copy)]
pub enum FlowchartLayoutDom {
    Nested,
    Elk,
    #[cfg(feature = "layout-grid")]
    Grid,
    #[cfg(feature = "layout-layered")]
    Layered,
}

impl FlowchartLayoutDom {
    pub const fn is_flat(self) -> bool {
        !matches!(self, Self::Nested)
    }

    pub const fn is_elk(self) -> bool {
        matches!(self, Self::Elk)
    }
}

impl<'a> FlowchartSvgRoute<'a> {
    pub const fn dom(self) -> FlowchartLayoutDom {
        match self {
            Self::ElkParity => FlowchartLayoutDom::Elk,
            #[cfg(feature = "layout-grid")]
            Self::Grid(_) => FlowchartLayoutDom::Grid,
            #[cfg(feature = "layout-layered")]
            Self::Layered(_) => FlowchartLayoutDom::Layered,
            _ => FlowchartLayoutDom::Nested,
        }
    }

    pub const fn is_canonical_axis(self) -> bool {
        matches!(self, Self::CanonicalAxis(_))
    }

    pub const fn is_elk_parity(self) -> bool {
        matches!(self, Self::ElkParity)
    }

    pub const fn is_layered(self) -> bool {
        #[cfg(feature = "layout-layered")]
        return matches!(self, Self::Layered(_));
        #[cfg(not(feature = "layout-layered"))]
        false
    }

    pub const fn layout_fallback(self) -> Option<&'static str> {
        match self {
            #[cfg(all(feature = "layout-layered", feature = "layout-dagre"))]
            Self::LayeredDagreFallback => Some("dagre"),
            _ => None,
        }
    }

    /// Identifies Grid's materialized edges and returns their shared paint settings.
    /// Their `LayoutEdge::label` positions and sizes belong to the measured label helpers.
    #[cfg(feature = "layout-grid")]
    pub const fn grid_paint(self) -> Option<FlowchartGridPaint> {
        match self {
            Self::Grid(settings) => Some(settings),
            _ => None,
        }
    }

    pub const fn uses_projected_edges(self) -> bool {
        // Grid, like the other projected providers, retains entire semantic edges in source
        // order. The storage for those edges is still materialized.
        #[cfg(feature = "layout-grid")]
        if self.grid_paint().is_some() {
            return true;
        }
        self.is_elk_parity() || self.is_layered() || self.is_canonical_axis()
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
