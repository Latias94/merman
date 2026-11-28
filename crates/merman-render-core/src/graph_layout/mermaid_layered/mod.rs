//! Mermaid 12.4.0 Layered over measured graph occurrences.

use super::{
    Axis, Direction, MeasuredGraph, Point, RoutePoint, RoutingStyle as Routing, Side, Size,
    SolvedGraph,
};
use crate::resources::{OperationWorkError, OperationWorkMeter};

mod compound;
mod crossings;
mod flat;
mod inner;
mod level;
mod measured;
mod work;

use level::{
    Attachment, Charge, CompoundError, LevelEdge, LevelNode, LevelOptions, LevelPlan, LevelResult,
    Membership, Mode, NodeKind, OrderedLevel, PlacedLevel, RankPoint, Result, Solve, Spacing,
};
pub use level::{InputError, Owner};

pub type LayeredError = CompoundError<OperationWorkError>;

/// The effective Mermaid `layered.edgeRouting` option.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum EdgeRouting {
    Rounded,
    Curved,
    Stepped,
    Straight,
}

impl EdgeRouting {
    pub fn from_name(name: Option<&str>) -> Self {
        match name {
            Some("curved") => Self::Curved,
            Some("stepped") => Self::Stepped,
            Some("straight") => Self::Straight,
            _ => Self::Rounded,
        }
    }

    pub const fn geometry(self) -> Routing {
        match self {
            Self::Rounded | Self::Stepped => Routing::Orthogonal,
            Self::Curved => Routing::Curved,
            Self::Straight => Routing::Polyline,
        }
    }
}

/// Controls that are specific to Layered. The shared measured graph supplies node and rank spacing.
#[derive(Clone, Copy, Debug)]
pub struct Options {
    pub edge_spacing: f64,
    pub diagram_padding: f64,
    pub head_room: f64,
    /// Optional measured-kernel width. Mermaid 12.4.0's LayoutData adapter leaves it absent.
    pub available_width: Option<f64>,
}

impl Default for Options {
    fn default() -> Self {
        Self {
            edge_spacing: 20.0,
            diagram_padding: 0.0,
            head_room: 7.0,
            available_width: None,
        }
    }
}

#[derive(Debug)]
pub struct LayeredLayout {
    pub graph: SolvedGraph,
    /// Source occurrence indices in Mermaid's drawing order. Group-to-member edges come last.
    pub edge_order: Vec<usize>,
}

/// Solve already-measured inputs. The family keeps source IDs and owns contour and marker paint.
pub fn layout(
    graph: &MeasuredGraph,
    options: Options,
    meter: &OperationWorkMeter,
) -> std::result::Result<LayeredLayout, LayeredError> {
    let result = measured::layout(
        graph,
        options,
        &mut |units| meter.charge(units),
        &mut flat::solve,
    );
    // Replay a sticky host terminal even when geometry itself also failed.
    meter.charge(0).map_err(CompoundError::Work)?;
    match result {
        Err(CompoundError::ArithmeticOverflow) => {
            Err(CompoundError::Work(meter.arithmetic_overflow()))
        }
        result => result,
    }
}

trait PointExt {
    fn translated(self, other: Point) -> Point;
    fn is_finite(&self) -> bool;
}

impl PointExt for Point {
    fn translated(self, other: Point) -> Point {
        Point {
            x: self.x + other.x,
            y: self.y + other.y,
        }
    }

    fn is_finite(&self) -> bool {
        self.x.is_finite() && self.y.is_finite()
    }
}

trait SizeExt {
    fn valid(self) -> bool;
    fn in_axes(self, direction: Direction) -> Size;
}

impl SizeExt for Size {
    fn valid(self) -> bool {
        self.width.is_finite() && self.width >= 0.0 && self.height.is_finite() && self.height >= 0.0
    }

    fn in_axes(self, direction: Direction) -> Size {
        if direction.vertical() {
            self
        } else {
            Size {
                width: self.height,
                height: self.width,
            }
        }
    }
}

trait DirectionExt {
    fn vertical(self) -> bool;
    fn flipped(self) -> bool;
    fn start(self) -> Side;
    fn axis(self) -> Axis;
}

impl DirectionExt for Direction {
    fn vertical(self) -> bool {
        matches!(self, Self::Down | Self::Up)
    }

    fn flipped(self) -> bool {
        matches!(self, Self::Up | Self::Left)
    }

    fn start(self) -> Side {
        match self {
            Self::Down => Side::Top,
            Self::Up => Side::Bottom,
            Self::Right => Side::Left,
            Self::Left => Side::Right,
        }
    }

    fn axis(self) -> Axis {
        self.start().axis()
    }
}

trait SideExt {
    fn opposite(self) -> Side;
}

impl SideExt for Side {
    fn opposite(self) -> Side {
        match self {
            Self::Top => Self::Bottom,
            Self::Right => Self::Left,
            Self::Bottom => Self::Top,
            Self::Left => Self::Right,
        }
    }
}

trait AxisExt {
    fn perpendicular(self) -> Axis;
}

impl AxisExt for Axis {
    fn perpendicular(self) -> Axis {
        match self {
            Self::Vertical => Self::Horizontal,
            Self::Horizontal => Self::Vertical,
        }
    }
}

trait RoutePointExt {
    fn position(self) -> Point;
}

impl RoutePointExt for RoutePoint {
    fn position(self) -> Point {
        Point {
            x: self.x,
            y: self.y,
        }
    }
}

fn route_point(point: Point, axis: Axis) -> RoutePoint {
    RoutePoint::new(point.x, point.y, axis)
}

#[cfg(test)]
mod tests;
