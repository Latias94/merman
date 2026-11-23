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
use ir::{Axis, Side};

/// Render-only metadata. Compatibility JSON already carries the coordinates in occurrence order.
#[cfg(any(feature = "layout-shoal", test))]
#[derive(Clone, Debug, PartialEq)]
pub(crate) struct RouteAxes {
    pub(crate) endpoints: (usize, usize),
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
        routes: Vec<Option<RouteAxes>>,
    },
}

impl RouteContract {
    pub(crate) fn uses_projected_edges(&self) -> bool {
        match self {
            Self::ElkParity => true,
            #[cfg(any(feature = "layout-shoal", test))]
            Self::CanonicalAxis { .. } => true,
            Self::DagreParity | Self::Swimlane => false,
        }
    }
}

/// The SVG marker's setback and footprint in drawing units, measured before routing.
#[derive(Clone, Copy, Debug, Default)]
pub(crate) struct Marker {
    pub(crate) trim: f64,
    pub(crate) depth: f64,
    pub(crate) width: f64,
}
