//! Measured and solved geometry in occurrence order. A family retains its semantic bindings.

#![allow(
    dead_code,
    reason = "The shared IR includes groups and terminal labels for families beyond the flat Flowchart consumer."
)]

use crate::model::LayoutLabel;

// Share plain geometry so a solver can transfer entire rectangle and route buffers to the IR.
#[cfg(feature = "layout-shoal")]
pub(crate) use merman_layout_shoal::{Direction, Point, Rect, RoutingStyle, Size, TitlePosition};

#[allow(
    unused_imports,
    reason = "Axis and knot kinds are used by canonical routing in selected diagram families."
)]
pub(crate) use merman_render_core::route_types::{Axis, KnotKind};
pub(crate) use merman_render_core::route_types::{
    BorderStitch, Endpoint, Marker, RoutePoint, Side,
};

#[cfg(not(feature = "layout-shoal"))]
pub(crate) use geometry::*;

#[cfg(not(feature = "layout-shoal"))]
mod geometry {
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

pub(crate) fn direction_from_rank(rank: &str) -> Direction {
    let rank = rank.trim();
    if rank.eq_ignore_ascii_case("BT") {
        Direction::Up
    } else if rank.eq_ignore_ascii_case("LR") {
        Direction::Right
    } else if rank.eq_ignore_ascii_case("RL") {
        Direction::Left
    } else {
        Direction::Down
    }
}

#[derive(Debug)]
#[cfg_attr(test, derive(Clone))]
pub(crate) struct MeasuredGraph {
    pub(crate) direction: Direction,
    pub(crate) routing: RoutingStyle,
    pub(crate) node_spacing: f64,
    pub(crate) rank_spacing: f64,
    pub(crate) component_spacing: f64,
    pub(crate) inherit_crossed_group_direction: bool,
    pub(crate) nodes: Vec<MeasuredNode>,
    pub(crate) groups: Vec<MeasuredGroup>,
    pub(crate) edges: Vec<MeasuredEdge>,
}

#[derive(Debug)]
#[cfg_attr(test, derive(Clone))]
pub(crate) struct MeasuredNode {
    pub(crate) size: Size,
    pub(crate) seq: u32,
    pub(crate) parent: Option<u32>,
    /// Usable length along the root flow's facing side. Zero pins the port to its anchor.
    pub(crate) port_span: Option<f64>,
    pub(crate) perpendicular_port_span: Option<f64>,
    pub(crate) anchor_y: Option<f64>,
}

#[derive(Debug)]
#[cfg_attr(test, derive(Clone))]
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
#[cfg_attr(test, derive(Clone))]
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
    pub(crate) node_ranks: Vec<u32>,
    pub(crate) groups: Vec<SolvedGroup>,
    pub(crate) edges: Vec<SolvedEdge>,
    pub(crate) border_stitches: Vec<BorderStitch>,
    pub(crate) direction: Direction,
    /// Extent of the drawing after translation to the origin.
    pub(crate) bounds: Size,
}

#[derive(Debug)]
pub(crate) struct SolvedGroup {
    pub(crate) parent: Option<u32>,
    pub(crate) rect: Rect,
    pub(crate) title: Option<LayoutLabel>,
    pub(crate) direction: Direction,
}

#[derive(Debug)]
pub(crate) struct SolvedEdge {
    /// Physical contacts after any solver-side truncation. Semantic owners stay with the family.
    pub(crate) source: Endpoint,
    pub(crate) target: Endpoint,
    pub(crate) points: Vec<RoutePoint>,
    pub(crate) ports: (Side, Side),
    pub(crate) label: Option<LayoutLabel>,
    pub(crate) start_label: Option<LayoutLabel>,
    pub(crate) end_label: Option<LayoutLabel>,
    pub(crate) reversed_for_ranking: bool,
}
