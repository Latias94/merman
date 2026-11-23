/// Index in [`Graph::nodes`].
#[repr(transparent)]
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct NodeId(pub u32);

/// Index in [`Graph::groups`].
#[repr(transparent)]
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct GroupId(pub u32);

/// Index of an edge occurrence in [`Graph::edges`].
#[repr(transparent)]
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct EdgeId(pub u32);

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Endpoint {
    Node(NodeId),
    Group(GroupId),
}

impl From<NodeId> for Endpoint {
    fn from(id: NodeId) -> Self {
        Self::Node(id)
    }
}

impl From<GroupId> for Endpoint {
    fn from(id: GroupId) -> Self {
        Self::Group(id)
    }
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum Direction {
    #[default]
    Down,
    Up,
    Right,
    Left,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Side {
    Top,
    Right,
    Bottom,
    Left,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Axis {
    Vertical,
    Horizontal,
}

impl Side {
    /// Axis of the normal to this face in drawing coordinates.
    pub fn axis(self) -> Axis {
        match self {
            Self::Top | Self::Bottom => Axis::Vertical,
            Self::Left | Self::Right => Axis::Horizontal,
        }
    }
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

/// Top-left corner and outer dimensions in the final drawing axes.
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

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Insets {
    pub top: f64,
    pub right: f64,
    pub bottom: f64,
    pub left: f64,
}

/// `axis` is the travel axis of the incoming leg, or of the departing leg at the first
/// point. A curved leg can move along both coordinates while its endpoint tangents share an axis.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct RoutePoint {
    pub x: f64,
    pub y: f64,
    pub axis: Axis,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct OrderHint {
    pub rank: i32,
    pub cross_key: f64,
}

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct MarkerMetrics {
    pub length: f64,
    pub width: f64,
}

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub enum PortSideHint {
    #[default]
    Auto,
    Side(Side),
    /// Offset from the physical face center, positive toward the right or bottom.
    FixedOffset {
        side: Side,
        offset: f64,
    },
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

#[non_exhaustive]
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct NodeSpec {
    pub size: Size,
    pub parent: Option<GroupId>,
    /// Usable length centered on the face. Zero pins ports to the center.
    pub port_span: Option<f64>,
    /// Usable face length when a child flows perpendicular to the root direction.
    /// An unspecified length inherits `port_span`.
    pub perpendicular_port_span: Option<f64>,
    /// Position of a left/right anchor measured from the top of the box.
    pub anchor_y: Option<f64>,
    pub seq: u32,
    pub order_hint: Option<OrderHint>,
    pub(crate) boundary: Option<crate::flat::BoundaryRank>,
    pub(crate) pin: i8,
    pub(crate) group_proxy: bool,
}

impl NodeSpec {
    pub fn new(size: Size, seq: u32) -> Self {
        Self {
            size,
            parent: None,
            port_span: None,
            perpendicular_port_span: None,
            anchor_y: None,
            seq,
            order_hint: None,
            boundary: None,
            pin: 0,
            group_proxy: false,
        }
    }
}

#[non_exhaustive]
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct GroupSpec {
    pub parent: Option<GroupId>,
    pub direction: Option<Direction>,
    pub padding: Insets,
    pub title: Option<Size>,
    pub title_position: TitlePosition,
    pub min_size: Size,
    pub seq: u32,
}

impl GroupSpec {
    pub fn new(seq: u32) -> Self {
        Self {
            seq,
            ..Self::default()
        }
    }
}

#[non_exhaustive]
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct EdgeSpec {
    pub source: Endpoint,
    pub target: Endpoint,
    pub min_rank_span: u32,
    pub weight: f32,
    pub label: Option<Size>,
    pub start_label: Option<Size>,
    pub end_label: Option<Size>,
    pub start_marker: MarkerMetrics,
    pub end_marker: MarkerMetrics,
    pub source_port: PortSideHint,
    pub target_port: PortSideHint,
}

impl EdgeSpec {
    pub fn new(source: Endpoint, target: Endpoint) -> Self {
        Self {
            source,
            target,
            min_rank_span: 1,
            weight: 1.0,
            label: None,
            start_label: None,
            end_label: None,
            start_marker: MarkerMetrics::default(),
            end_marker: MarkerMetrics::default(),
            source_port: PortSideHint::Auto,
            target_port: PortSideHint::Auto,
        }
    }
}

/// Each dense handle indexes its corresponding array. Parallel edges have distinct handles.
#[non_exhaustive]
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Graph {
    pub direction: Direction,
    pub nodes: Vec<NodeSpec>,
    pub groups: Vec<GroupSpec>,
    pub edges: Vec<EdgeSpec>,
}

#[non_exhaustive]
#[derive(Clone, Debug, PartialEq)]
pub struct EdgeGeometry {
    /// In semantic source-to-target order, including both box-boundary contacts.
    pub points: Vec<RoutePoint>,
    pub ports: (Side, Side),
    pub label_center: Option<Point>,
    pub start_label_center: Option<Point>,
    pub end_label_center: Option<Point>,
    pub reversed_for_ranking: bool,
}

#[non_exhaustive]
#[derive(Clone, Debug, PartialEq)]
pub struct GroupGeometry {
    /// Enclosing group after parent-cycle repair.
    pub parent: Option<GroupId>,
    pub rect: Rect,
    pub title_center: Option<Point>,
    pub effective_direction: Direction,
}

#[non_exhaustive]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct TruncatedRoute {
    pub edge: EdgeId,
    /// Resolved source contact after applying the border-port budget.
    pub source: Endpoint,
    /// Resolved target contact after applying the border-port budget.
    pub target: Endpoint,
}

#[non_exhaustive]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct BorderStitch {
    pub edge: EdgeId,
    /// Index into the edge's [`EdgeGeometry::points`] in semantic source-to-target order.
    pub point: usize,
    pub group: GroupId,
    pub side: Side,
}

/// Work attribution for the selected drawing. Routed crossings use flat polylines or the sampled compound routing style.
#[non_exhaustive]
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct LayoutStats {
    pub components: usize,
    pub scc_cyclic_count: usize,
    pub reversed_edges: usize,
    pub dummy_nodes: usize,
    pub direct_fallback_edges: usize,
    pub topological_crossings: u64,
    pub routed_crossings: u64,
    pub truncated_border_ports: usize,
}

/// All arrays use input order. The union of boxes, labels and points starts at `(0, 0)`.
#[non_exhaustive]
#[derive(Clone, Debug, Default, PartialEq)]
pub struct LayoutOutput {
    pub nodes: Vec<Rect>,
    pub groups: Vec<GroupGeometry>,
    pub edges: Vec<EdgeGeometry>,
    pub truncated_routes: Vec<TruncatedRoute>,
    pub border_stitches: Vec<BorderStitch>,
    pub direction: Direction,
    pub bounds: Size,
    pub stats: LayoutStats,
}
