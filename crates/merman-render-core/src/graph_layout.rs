//! Measured and solved geometry in occurrence order. A family retains its semantic bindings.

use crate::LayoutLabel;

#[cfg(all(feature = "route-paint", feature = "svg-output"))]
pub mod axis_clearance;
#[cfg(feature = "svg-output")]
pub mod elk_geometry;
#[cfg(feature = "svg-output")]
pub mod elk_terminal_jogs;
#[cfg(feature = "shoal-bridge")]
pub mod shoal;

// Share plain geometry so a solver can transfer entire rectangle and route buffers to the IR.
#[cfg(feature = "layout-shoal")]
pub use merman_layout_shoal::{Direction, Point, Rect, RoutingStyle, Size, TitlePosition};

pub use crate::route_types::{Axis, BorderStitch, Endpoint, KnotKind, Marker, RoutePoint, Side};

#[cfg(not(feature = "layout-shoal"))]
pub use geometry::*;

#[cfg(not(feature = "layout-shoal"))]
mod geometry {
    #[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
    pub enum Direction {
        #[default]
        Down,
        Up,
        Right,
        Left,
    }

    #[derive(Clone, Copy, Debug, Eq, PartialEq)]
    pub enum RoutingStyle {
        Curved,
        Orthogonal,
        Polyline,
    }

    #[derive(Clone, Copy, Debug, Default, PartialEq)]
    pub struct Size {
        pub width: f64,
        pub height: f64,
    }

    #[derive(Clone, Copy, Debug, Default, PartialEq)]
    pub struct Point {
        pub x: f64,
        pub y: f64,
    }

    /// Top-left corner and outer dimensions in drawing coordinates.
    #[derive(Clone, Copy, Debug, Default, PartialEq)]
    pub struct Rect {
        pub x: f64,
        pub y: f64,
        pub width: f64,
        pub height: f64,
    }

    impl Rect {
        pub fn center(self) -> Point {
            Point {
                x: self.x + self.width * 0.5,
                y: self.y + self.height * 0.5,
            }
        }
    }

    #[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
    pub enum TitlePosition {
        TopLeft,
        #[default]
        TopCenter,
        TopRight,
        BottomLeft,
        BottomCenter,
        BottomRight,
    }
}

pub fn direction_from_rank(rank: &str) -> Direction {
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
#[cfg_attr(any(test, feature = "test-support"), derive(Clone))]
pub struct MeasuredGraph {
    pub direction: Direction,
    pub routing: RoutingStyle,
    pub node_spacing: f64,
    pub rank_spacing: f64,
    pub component_spacing: f64,
    pub inherit_crossed_group_direction: bool,
    pub nodes: Vec<MeasuredNode>,
    pub groups: Vec<MeasuredGroup>,
    pub edges: Vec<MeasuredEdge>,
}

#[derive(Debug)]
#[cfg_attr(any(test, feature = "test-support"), derive(Clone))]
pub struct MeasuredNode {
    pub size: Size,
    pub seq: u32,
    pub parent: Option<u32>,
    /// Usable length along the root flow's facing side. Zero pins the port to its anchor.
    pub port_span: Option<f64>,
    pub perpendicular_port_span: Option<f64>,
    pub anchor_y: Option<f64>,
}

#[derive(Debug)]
#[cfg_attr(any(test, feature = "test-support"), derive(Clone))]
pub struct MeasuredGroup {
    pub seq: u32,
    pub parent: Option<u32>,
    pub direction: Option<Direction>,
    pub padding: kurbo::Insets,
    pub min_size: Size,
    pub title: Option<Size>,
    pub title_position: TitlePosition,
}

#[derive(Debug)]
#[cfg_attr(any(test, feature = "test-support"), derive(Clone))]
pub struct MeasuredEdge {
    pub source: Endpoint,
    pub target: Endpoint,
    pub min_rank_span: u32,
    pub weight: f32,
    pub label: Option<Size>,
    pub start_label: Option<Size>,
    pub end_label: Option<Size>,
    pub start_marker: Marker,
    pub end_marker: Marker,
}

/// Geometry remains in input order, with routes running from semantic source to target.
#[derive(Debug)]
pub struct SolvedGraph {
    pub nodes: Vec<Rect>,
    pub node_ranks: Vec<u32>,
    pub groups: Vec<SolvedGroup>,
    pub edges: Vec<SolvedEdge>,
    pub border_stitches: Vec<BorderStitch>,
    pub direction: Direction,
    /// Extent of the drawing after translation to the origin.
    pub bounds: Size,
}

#[derive(Debug)]
pub struct SolvedGroup {
    pub parent: Option<u32>,
    pub rect: Rect,
    pub title: Option<LayoutLabel>,
    pub direction: Direction,
}

#[derive(Debug)]
pub struct SolvedEdge {
    /// Physical contacts after any solver-side truncation. Semantic owners stay with the family.
    pub source: Endpoint,
    pub target: Endpoint,
    pub points: Vec<RoutePoint>,
    pub ports: (Side, Side),
    pub label: Option<LayoutLabel>,
    pub start_label: Option<LayoutLabel>,
    pub end_label: Option<LayoutLabel>,
    pub reversed_for_ranking: bool,
}
