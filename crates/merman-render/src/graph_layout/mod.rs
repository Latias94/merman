//! Shared boundary between measured diagram families, graph solvers, and path painting.

#[cfg(any(feature = "layout-shoal", test))]
pub(crate) mod contour;
pub(crate) mod ir;
#[cfg(any(feature = "layout-shoal", test))]
pub(crate) mod route_paint;
#[cfg(feature = "layout-shoal")]
pub(crate) mod shoal_bridge;
#[cfg(test)]
pub(crate) mod validate;

#[cfg(any(feature = "layout-shoal", test))]
pub(crate) use ir::Marker;
#[cfg(any(feature = "layout-shoal", test))]
use ir::{Axis, BorderStitch, Endpoint, Side};

/// Render-only metadata. Compatibility JSON already carries the coordinates in occurrence order.
#[cfg(any(feature = "layout-shoal", test))]
#[derive(Clone, Debug, PartialEq)]
pub(crate) struct RouteAxes {
    pub(crate) endpoints: (Endpoint, Endpoint),
    pub(crate) ports: (Side, Side),
    pub(crate) axes: Vec<Axis>,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub(crate) enum RouteContract {
    #[default]
    DagreParity,
    ElkParity,
    Swimlane,
    /// Axes indexed by semantic occurrence, including holes for hidden edges.
    #[cfg(any(feature = "layout-shoal", test))]
    CanonicalAxis {
        nodes: Vec<contour::ShapePortProfile>,
        group_parents: Vec<Option<u32>>,
        border_stitches: Vec<BorderStitch>,
        clearance: route_paint::Clearance,
        routes: Vec<Option<RouteAxes>>,
    },
}

impl RouteContract {
    pub(crate) fn group_parents(&self) -> Option<&[Option<u32>]> {
        #[cfg(any(feature = "layout-shoal", test))]
        if let Self::CanonicalAxis { group_parents, .. } = self {
            return Some(group_parents);
        }
        None
    }

    pub(crate) fn uses_projected_edges(&self) -> bool {
        match self {
            Self::ElkParity => true,
            #[cfg(any(feature = "layout-shoal", test))]
            Self::CanonicalAxis { .. } => true,
            Self::DagreParity | Self::Swimlane => false,
        }
    }
}
