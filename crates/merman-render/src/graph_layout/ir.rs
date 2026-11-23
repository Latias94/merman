//! Measured and solved geometry in occurrence order. A family retains its semantic bindings.

#![allow(
    dead_code,
    reason = "The shared IR includes groups and terminal labels for families beyond the flat Flowchart consumer."
)]

use crate::model::LayoutLabel;

/// The marker's setback and footprint in drawing units, measured before routing.
#[derive(Clone, Copy, Debug, Default)]
pub(crate) struct Marker {
    pub(crate) trim: f64,
    pub(crate) depth: f64,
    pub(crate) width: f64,
}

// Share plain geometry so a solver can transfer entire rectangle and route buffers to the IR.
#[cfg(feature = "layout-shoal")]
pub(crate) use merman_layout_shoal::{
    Axis, Direction, Point, Rect, RoutePoint, RoutingStyle, Side, Size, TitlePosition,
};

#[cfg(not(feature = "layout-shoal"))]
pub(crate) use geometry::*;

#[cfg(not(feature = "layout-shoal"))]
mod geometry {
    #[derive(Clone, Copy, Debug, Eq, PartialEq)]
    pub(crate) enum Axis {
        Horizontal,
        Vertical,
    }

    #[derive(Clone, Copy, Debug, Eq, PartialEq)]
    pub(crate) enum Side {
        Top,
        Right,
        Bottom,
        Left,
    }

    impl Side {
        pub(crate) fn axis(self) -> Axis {
            match self {
                Self::Top | Self::Bottom => Axis::Vertical,
                Self::Left | Self::Right => Axis::Horizontal,
            }
        }
    }

    #[derive(Clone, Copy, Debug, PartialEq)]
    pub(crate) struct RoutePoint {
        pub(crate) x: f64,
        pub(crate) y: f64,
        pub(crate) axis: Axis,
    }

    #[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
    pub(crate) enum Direction {
        #[default]
        Down,
        Up,
        Right,
        Left,
    }

    #[derive(Clone, Copy, Debug, Eq, PartialEq)]
    pub(crate) enum RoutingStyle {
        Curved,
        Orthogonal,
        Polyline,
    }

    #[derive(Clone, Copy, Debug, Default, PartialEq)]
    pub(crate) struct Size {
        pub(crate) width: f64,
        pub(crate) height: f64,
    }

    #[derive(Clone, Copy, Debug, Default, PartialEq)]
    pub(crate) struct Point {
        pub(crate) x: f64,
        pub(crate) y: f64,
    }

    /// Top-left corner and outer dimensions in drawing coordinates.
    #[derive(Clone, Copy, Debug, Default, PartialEq)]
    pub(crate) struct Rect {
        pub(crate) x: f64,
        pub(crate) y: f64,
        pub(crate) width: f64,
        pub(crate) height: f64,
    }

    impl Rect {
        pub(crate) fn center(self) -> Point {
            Point {
                x: self.x + self.width * 0.5,
                y: self.y + self.height * 0.5,
            }
        }
    }

    #[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
    pub(crate) enum TitlePosition {
        TopLeft,
        #[default]
        TopCenter,
        TopRight,
        BottomLeft,
        BottomCenter,
        BottomRight,
    }
}

/// Dense index in the graph's node or group array. Textual IDs belong to the family.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum Endpoint {
    Node(u32),
    Group(u32),
}

#[derive(Debug)]
pub(crate) struct MeasuredGraph {
    pub(crate) direction: Direction,
    pub(crate) routing: RoutingStyle,
    pub(crate) node_spacing: f64,
    pub(crate) rank_spacing: f64,
    pub(crate) component_spacing: f64,
    pub(crate) nodes: Vec<MeasuredNode>,
    pub(crate) groups: Vec<MeasuredGroup>,
    pub(crate) edges: Vec<MeasuredEdge>,
}

#[derive(Debug)]
pub(crate) struct MeasuredNode {
    pub(crate) size: Size,
    pub(crate) seq: u32,
    pub(crate) parent: Option<u32>,
    /// Usable length along the flow-facing side. Zero pins the port to its anchor.
    pub(crate) port_span: Option<f64>,
    pub(crate) anchor_y: Option<f64>,
}

#[derive(Debug)]
pub(crate) struct MeasuredGroup {
    pub(crate) seq: u32,
    pub(crate) parent: Option<u32>,
    pub(crate) direction: Option<Direction>,
    pub(crate) padding: kurbo::Insets,
    pub(crate) min_size: Size,
    pub(crate) title: Option<Size>,
    pub(crate) title_position: TitlePosition,
}

#[derive(Debug)]
pub(crate) struct MeasuredEdge {
    pub(crate) source: Endpoint,
    pub(crate) target: Endpoint,
    pub(crate) min_rank_span: u32,
    pub(crate) weight: f32,
    pub(crate) label: Option<Size>,
    pub(crate) start_label: Option<Size>,
    pub(crate) end_label: Option<Size>,
    pub(crate) start_marker: Marker,
    pub(crate) end_marker: Marker,
}

/// Geometry remains in input order, with routes running from semantic source to target.
#[derive(Debug)]
pub(crate) struct SolvedGraph {
    pub(crate) nodes: Vec<Rect>,
    pub(crate) groups: Vec<SolvedGroup>,
    pub(crate) edges: Vec<SolvedEdge>,
    pub(crate) direction: Direction,
    /// Extent of the drawing after translation to the origin.
    pub(crate) bounds: Size,
}

#[derive(Debug)]
pub(crate) struct SolvedGroup {
    pub(crate) rect: Rect,
    pub(crate) title: Option<LayoutLabel>,
    pub(crate) direction: Direction,
}

#[derive(Debug)]
pub(crate) struct SolvedEdge {
    pub(crate) source: Endpoint,
    pub(crate) target: Endpoint,
    pub(crate) points: Vec<RoutePoint>,
    pub(crate) ports: (Side, Side),
    pub(crate) label: Option<LayoutLabel>,
    pub(crate) start_label: Option<LayoutLabel>,
    pub(crate) end_label: Option<LayoutLabel>,
}
