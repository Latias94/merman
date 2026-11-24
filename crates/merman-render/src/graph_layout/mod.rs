//! Shared boundary between measured diagram families, graph solvers, and path painting.

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
pub(crate) mod canonical;
#[cfg(any(feature = "layout-shoal", test))]
pub(crate) mod contour;
pub(crate) mod ir;
#[cfg(feature = "layout-shoal")]
pub(crate) mod placement;
#[cfg(any(feature = "layout-shoal", test))]
pub(crate) mod route_paint;
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

#[derive(Clone, Debug, Default)]
pub(crate) enum RouteContract {
    #[default]
    DagreParity,
    #[cfg(any(feature = "layout-elk", all(test, feature = "diagram-flowchart")))]
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
    CanonicalAxis {
        nodes: Vec<contour::ShapePortProfile>,
        group_parents: Vec<Option<u32>>,
        border_stitches: Vec<BorderStitch>,
        clearance: route_paint::Clearance,
        routes: Vec<Option<RouteTopology>>,
    },
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

impl RouteContract {
    #[cfg(any(feature = "diagram-requirement", feature = "diagram-usecase"))]
    pub(crate) fn parity(uses_elk: bool) -> Self {
        if uses_elk {
            #[cfg(feature = "layout-elk")]
            return Self::ElkParity;
        }
        Self::DagreParity
    }

    pub(crate) fn is_elk_parity(&self) -> bool {
        #[cfg(any(feature = "layout-elk", all(test, feature = "diagram-flowchart")))]
        return matches!(self, Self::ElkParity);
        #[cfg(not(any(feature = "layout-elk", all(test, feature = "diagram-flowchart"))))]
        false
    }

    #[cfg(any(
        feature = "diagram-flowchart",
        feature = "diagram-swimlane",
        feature = "diagram-agentflow"
    ))]
    pub(crate) fn group_parents(&self) -> Option<&[Option<u32>]> {
        #[cfg(any(feature = "layout-shoal", test))]
        if let Self::CanonicalAxis { group_parents, .. } = self {
            return Some(group_parents);
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
            Self::CanonicalAxis { .. } => true,
            _ => false,
        }
    }
}
