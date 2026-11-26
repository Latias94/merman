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
#[cfg(all(
    any(feature = "layout-shoal", test),
    any(
        feature = "diagram-flowchart",
        feature = "diagram-swimlane",
        feature = "diagram-agentflow"
    )
))]
use ir::{Axis, BorderStitch, Endpoint, KnotKind, Side};
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

#[cfg(any(feature = "layout-shoal", test))]
#[cfg(any(
    feature = "diagram-flowchart",
    feature = "diagram-swimlane",
    feature = "diagram-agentflow",
    all(
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
    )
))]
pub(crate) fn edge_paint_error(edge_id: &str, error: route_paint::RoutePaintError) -> crate::Error {
    crate::Error::InvalidModel {
        message: format!("Could not paint canonical edge `{edge_id}`: {error}"),
    }
}

/// Render-only metadata. Compatibility JSON already carries the coordinates in occurrence order.
#[cfg(all(
    any(feature = "layout-shoal", test),
    any(
        feature = "diagram-flowchart",
        feature = "diagram-swimlane",
        feature = "diagram-agentflow"
    )
))]
#[derive(Clone, Debug, PartialEq)]
pub(crate) struct RouteTopology {
    pub(crate) endpoints: (Endpoint, Endpoint),
    pub(crate) ports: (Side, Side),
    pub(crate) knots: Vec<(Axis, KnotKind)>,
}

#[cfg(all(
    any(feature = "layout-shoal", test),
    any(
        feature = "diagram-flowchart",
        feature = "diagram-swimlane",
        feature = "diagram-agentflow"
    )
))]
#[derive(Debug)]
pub(crate) struct AxisRoutes {
    pub(crate) nodes: Vec<contour::ShapePortProfile>,
    pub(crate) group_parents: Vec<Option<u32>>,
    pub(crate) border_stitches: Vec<BorderStitch>,
    pub(crate) clearance: axis_clearance::AxisClearance,
    pub(crate) routes: Vec<Option<RouteTopology>>,
}

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
    pub(crate) fn is_elk_parity(&self) -> bool {
        #[cfg(feature = "layout-elk")]
        return matches!(self, Self::ElkParity);
        #[cfg(not(feature = "layout-elk"))]
        false
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
