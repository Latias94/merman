//! Deterministic layered layout for measured, flat directed acyclic multigraphs.

#![forbid(unsafe_code)]

use std::{convert::Infallible, error::Error, fmt};

mod kernel;
mod layers;
mod routing;
mod work;

/// Index in [`Graph::nodes`].
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct NodeId(pub usize);

/// Outer dimensions in the caller's coordinate system, including any label padding.
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

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum Direction {
    #[default]
    Down,
    Up,
    Right,
    Left,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Graph {
    pub direction: Direction,
    /// Minimum space between nodes on the same rank and between components.
    pub node_spacing: f64,
    /// Minimum space between facing rank boundaries per logical rank step.
    pub rank_spacing: f64,
    pub nodes: Vec<Size>,
    pub edges: Vec<Edge>,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Edge {
    pub source: NodeId,
    pub target: NodeId,
    /// A forward edge advances at least one rank, including when this is zero.
    pub min_rank_span: usize,
    pub label: Option<Size>,
    /// Required distance beyond the source box before the first bend.
    pub start_clearance: f64,
    /// Required distance beyond the target box before the last bend.
    pub end_clearance: f64,
}

/// Extents of all node boxes, label boxes and route points. An empty graph has zero bounds.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Bounds {
    pub min_x: f64,
    pub min_y: f64,
    pub max_x: f64,
    pub max_y: f64,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct Layout {
    /// One center per input node, in input order.
    pub centers: Vec<Point>,
    /// One route per input edge occurrence, in input order.
    pub edges: Vec<Route>,
    pub bounds: Bounds,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct Route {
    /// Source center, exterior approach points, then target center. The caller clips each
    /// end against its actual shape and applies its marker setback when painting.
    pub points: Vec<Point>,
    /// Center of this occurrence's measured label box, when present.
    pub label_center: Option<Point>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum LayoutError<E> {
    InvalidInput(&'static str),
    Unsupported(&'static str),
    /// A derived rank, allocation, work estimate or coordinate exceeds its representation.
    Overflow,
    Interrupted(E),
}

impl<E: fmt::Display> fmt::Display for LayoutError<E> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidInput(reason) => f.write_str(reason),
            Self::Unsupported(construct) => write!(f, "Shoal does not support {construct}"),
            Self::Overflow => f.write_str("Shoal layout exceeds representable size"),
            Self::Interrupted(error) => write!(f, "Shoal layout interrupted: {error}"),
        }
    }
}

impl<E: Error + 'static> Error for LayoutError<E> {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Interrupted(error) => Some(error),
            _ => None,
        }
    }
}

/// Lays out a graph with no caller-imposed work limit.
pub fn layout(graph: &Graph) -> Result<Layout, LayoutError<Infallible>> {
    layout_with_control(graph, |_| Ok(()))
}

/// Charges incremental units of work. An error from `charge` stops the operation and is
/// returned unchanged inside [`LayoutError::Interrupted`]. Units are local to this algorithm.
pub fn layout_with_control<E>(
    graph: &Graph,
    charge: impl FnMut(usize) -> Result<(), E>,
) -> Result<Layout, LayoutError<E>> {
    kernel::run(graph, charge)
}
