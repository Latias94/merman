//! Geometry and occurrence indices used to paint routed edges.

#[cfg(feature = "layout-shoal")]
pub use merman_layout_shoal::{Axis, KnotKind, RoutePoint, Side, sweep_handle_fraction};

#[cfg(not(feature = "layout-shoal"))]
pub use fallback::{Axis, KnotKind, RoutePoint, Side, sweep_handle_fraction};

/// The marker's setback and footprint in drawing units, measured before routing.
#[derive(Clone, Copy, Debug, Default)]
pub struct Marker {
    pub trim: f64,
    pub depth: f64,
    pub width: f64,
}

/// Dense index in the graph's node or group array. Textual IDs belong to the family.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Endpoint {
    Node(u32),
    Group(u32),
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct BorderStitch {
    pub edge: usize,
    pub point: usize,
    pub group: u32,
    pub side: Side,
}

#[cfg(not(feature = "layout-shoal"))]
mod fallback {
    #[derive(Clone, Copy, Debug, Eq, PartialEq)]
    pub enum Axis {
        Horizontal,
        Vertical,
    }

    #[derive(Clone, Copy, Debug, Eq, PartialEq)]
    pub enum Side {
        Top,
        Right,
        Bottom,
        Left,
    }

    impl Side {
        pub fn axis(self) -> Axis {
            match self {
                Self::Top | Self::Bottom => Axis::Vertical,
                Self::Left | Self::Right => Axis::Horizontal,
            }
        }
    }

    #[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
    pub enum KnotKind {
        #[default]
        Free,
        GapBoundary,
    }

    #[derive(Clone, Copy, Debug, PartialEq)]
    pub struct RoutePoint {
        pub x: f64,
        pub y: f64,
        pub axis: Axis,
        pub kind: KnotKind,
    }

    impl RoutePoint {
        pub const fn new(x: f64, y: f64, axis: Axis) -> Self {
            Self {
                x,
                y,
                axis,
                kind: KnotKind::Free,
            }
        }
    }

    pub fn sweep_handle_fraction(across: f64, along: f64) -> f64 {
        let (across, along) = (across.abs(), along.abs());
        (0.45 + 0.12 * across.min(along) / across.max(along).max(1.0)).clamp(0.42, 0.56)
    }
}
