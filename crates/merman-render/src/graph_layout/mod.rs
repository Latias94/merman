//! Shared boundary between measured diagram families, graph solvers, and path painting.

#[cfg(all(
    any(feature = "layout-shoal", test),
    any(
        feature = "diagram-flowchart",
        feature = "diagram-swimlane",
        feature = "diagram-agentflow"
    )
))]
pub(crate) mod axis_clearance;
#[cfg(all(
    feature = "layout-shoal",
    any(
        feature = "diagram-architecture",
        feature = "diagram-c4",
        feature = "diagram-class",
        feature = "diagram-er",
        feature = "diagram-requirement",
        feature = "diagram-state",
        feature = "diagram-usecase"
    )
))]
pub(crate) mod canonical;
#[cfg(any(feature = "layout-shoal", test))]
pub(crate) mod contour;
pub(crate) mod ir;
#[cfg(feature = "layout-shoal")]
pub(crate) mod placement;
#[cfg(feature = "layout-shoal")]
pub(crate) mod shoal_bridge;
#[cfg(test)]
pub(crate) mod validate;

#[cfg(any(feature = "layout-shoal", test))]
pub(crate) use ir::Marker;
#[cfg(any(
    feature = "layout-shoal",
    all(
        test,
        any(
            feature = "diagram-flowchart",
            feature = "diagram-swimlane",
            feature = "diagram-agentflow"
        )
    )
))]
pub(crate) use merman_render_core::route_paint;
#[cfg(all(
    feature = "layout-shoal",
    any(
        feature = "diagram-flowchart",
        feature = "diagram-swimlane",
        feature = "diagram-agentflow"
    )
))]
pub(crate) use merman_render_flow::graph::FlowchartSvgEdgeGeometry;
#[cfg(all(
    any(feature = "layout-shoal", test),
    any(
        feature = "diagram-flowchart",
        feature = "diagram-swimlane",
        feature = "diagram-agentflow"
    )
))]
pub(crate) use merman_render_flow::graph::{AxisRoutes, RouteTopology};

#[derive(Clone, Debug, Default)]
#[cfg(any(
    feature = "diagram-agentflow",
    feature = "diagram-class",
    feature = "diagram-er",
    feature = "diagram-flowchart",
    feature = "diagram-requirement",
    feature = "diagram-state",
    feature = "diagram-swimlane",
    feature = "diagram-usecase"
))]
pub(crate) enum RouteContract {
    #[default]
    DagreParity,
    #[cfg(feature = "layout-elk")]
    ElkParity,
    #[cfg(any(
        feature = "diagram-flowchart",
        feature = "diagram-swimlane",
        feature = "diagram-agentflow",
        feature = "diagram-er"
    ))]
    Swimlane,
    /// Axes indexed by semantic occurrence, including holes for hidden edges.
    #[cfg(all(
        any(feature = "layout-shoal", test),
        any(
            feature = "diagram-flowchart",
            feature = "diagram-swimlane",
            feature = "diagram-agentflow"
        )
    ))]
    CanonicalAxis(std::sync::Arc<AxisRoutes>),
    #[cfg(all(
        feature = "layout-shoal",
        any(
            feature = "diagram-class",
            feature = "diagram-er",
            feature = "diagram-requirement",
            feature = "diagram-state",
            feature = "diagram-usecase"
        )
    ))]
    CanonicalGraph(std::sync::Arc<canonical::CanonicalRoutes>),
}

#[cfg(any(
    feature = "diagram-agentflow",
    feature = "diagram-class",
    feature = "diagram-er",
    feature = "diagram-flowchart",
    feature = "diagram-requirement",
    feature = "diagram-state",
    feature = "diagram-swimlane",
    feature = "diagram-usecase"
))]
impl RouteContract {
    #[cfg(any(
        feature = "diagram-agentflow",
        feature = "diagram-class",
        feature = "diagram-er",
        feature = "diagram-flowchart",
        feature = "diagram-state",
        feature = "diagram-swimlane"
    ))]
    pub(crate) fn is_elk_parity(&self) -> bool {
        #[cfg(feature = "layout-elk")]
        return matches!(self, Self::ElkParity);
        #[cfg(not(feature = "layout-elk"))]
        false
    }

    #[cfg(all(
        feature = "layout-shoal",
        any(
            feature = "diagram-flowchart",
            feature = "diagram-swimlane",
            feature = "diagram-agentflow"
        )
    ))]
    pub(crate) fn flowchart_svg_edges(&self) -> Option<&[FlowchartSvgEdgeGeometry]> {
        match self {
            Self::CanonicalAxis(axis) => axis.svg_edges.as_deref(),
            _ => None,
        }
    }

    #[cfg(any(
        feature = "diagram-flowchart",
        feature = "diagram-swimlane",
        feature = "diagram-agentflow"
    ))]
    pub(crate) fn group_parents(&self) -> Option<&[Option<u32>]> {
        #[cfg(any(feature = "layout-shoal", test))]
        if let Self::CanonicalAxis(routes) = self {
            return Some(&routes.group_parents);
        }
        None
    }

    #[cfg(any(
        feature = "diagram-flowchart",
        feature = "diagram-swimlane",
        feature = "diagram-agentflow"
    ))]
    pub(crate) fn uses_projected_edges(&self) -> bool {
        if self.is_elk_parity() {
            return true;
        }
        match self {
            #[cfg(any(feature = "layout-shoal", test))]
            Self::CanonicalAxis(_) => true,
            _ => false,
        }
    }
}
