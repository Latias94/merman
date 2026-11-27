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

impl Direction {
    pub(crate) fn axis(self) -> Axis {
        match self {
            Self::Down | Self::Up => Axis::Vertical,
            Self::Left | Self::Right => Axis::Horizontal,
        }
    }
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

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum KnotKind {
    #[default]
    Free,
    /// A curve turning onto a track must stay inside this end of the available gap.
    GapBoundary,
    /// A terminal sweep can lean outward from its analytically measured contour.
    ContourDeparture,
}

/// `axis` is the travel axis of the incoming leg, or of the departing leg at the first
/// point. A curved leg can move along both coordinates while its endpoint tangents share an axis.
/// Group contacts are recorded separately by `BorderStitch`.
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

    pub const fn gap_boundary(x: f64, y: f64, axis: Axis) -> Self {
        Self {
            kind: KnotKind::GapBoundary,
            ..Self::new(x, y, axis)
        }
    }
}

/// Along-axis handle fraction for an offset cubic. Keeping it below half orders the controls.
pub fn sweep_handle_fraction(across: f64, along: f64) -> f64 {
    let ratio = across.abs() / along.abs().max(1.0);
    (4.0 / (1.15 * ratio + 6.4)).clamp(0.25, 0.46)
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct OrderHint {
    pub rank: i32,
    pub cross_key: f64,
}

/// Measured geometry of a surviving node in the previous editor frame.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct PositionHint {
    /// Center in physical drawing axes, relative to the minimum of the previous component's real
    /// node boxes. Supply it only when the component coordinate frame is still applicable.
    pub center: Point,
    pub size: Size,
    /// Stable signature of its incident edge occurrences, using the caller's persistent identities.
    pub incident_signature: Option<u64>,
}

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct MarkerMetrics {
    /// Distance from the contour contact to the back of the marker along its port normal.
    pub length: f64,
    pub width: f64,
}

/// Minimum straight shaft beyond a marker's reference point, in drawing units.
pub const MIN_MARKER_LEAD: f64 = 6.0;

impl MarkerMetrics {
    pub(crate) fn runway_bound(self) -> f64 {
        self.length + self.length.max(MIN_MARKER_LEAD)
    }
}

/// Analytic outline centered in a measured node box.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub enum ShapeContourHint {
    #[default]
    Rectangle,
    Rounded {
        rx: f64,
        ry: f64,
    },
    Stadium,
    Diamond,
    /// The center of a translated diamond in box-local coordinates.
    OffsetDiamond {
        x: f64,
        y: f64,
    },
    Ellipse,
}

impl ShapeContourHint {
    /// Distance inward from a box face to the outline at a transverse port offset.
    /// Returns zero when the transverse line misses the outline.
    pub fn inset_at(self, size: Size, side: Side, across_offset: f64) -> f64 {
        let (width, height) = (size.width * 0.5, size.height * 0.5);
        let (across, along) = match side.axis() {
            Axis::Vertical => (width, height),
            Axis::Horizontal => (height, width),
        };
        if !across.is_finite()
            || !along.is_finite()
            || !across_offset.is_finite()
            || across <= 0.0
            || along <= 0.0
        {
            return 0.0;
        }
        let (cross_shift, along_shift) = match (self, side) {
            (Self::OffsetDiamond { x, y }, Side::Top) => (x, y),
            (Self::OffsetDiamond { x, y }, Side::Bottom) => (x, -y),
            (Self::OffsetDiamond { x, y }, Side::Left) => (y, x),
            (Self::OffsetDiamond { x, y }, Side::Right) => (y, -x),
            _ => (0.0, 0.0),
        };
        if !cross_shift.is_finite() || !along_shift.is_finite() {
            return 0.0;
        }
        let offset = (across_offset - cross_shift).abs();
        if !offset.is_finite() || offset > across {
            return 0.0;
        }
        let (radius, depth) = match self {
            Self::Rectangle => return 0.0,
            Self::Diamond | Self::OffsetDiamond { .. } => {
                let inset = (offset / across) * along + along_shift;
                return if inset.is_finite() {
                    inset.clamp(-along, 2.0 * along)
                } else {
                    0.0
                };
            }
            Self::Ellipse => (across, along),
            Self::Stadium => (width.min(height), width.min(height)),
            Self::Rounded { rx, ry } => match side.axis() {
                Axis::Vertical => (rx.min(across), ry.min(along)),
                Axis::Horizontal => (ry.min(across), rx.min(along)),
            },
        };
        let corner = offset - (across - radius);
        if !radius.is_finite()
            || !depth.is_finite()
            || radius <= 0.0
            || depth <= 0.0
            || corner <= 0.0
        {
            0.0
        } else {
            let ratio = (corner / radius).min(1.0);
            // Rationalizing the subtraction preserves small offsets near the flat run.
            depth * (ratio * ratio / (1.0 + ((1.0 - ratio) * (1.0 + ratio)).sqrt()))
        }
    }
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
    TopAuto,
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
    /// Outline geometry used for route and label placement inside the measured box.
    pub contour: ShapeContourHint,
    /// Usable length centered on the face. Zero pins ports to the center.
    pub port_span: Option<f64>,
    /// Usable face length when a child flows perpendicular to the root direction.
    /// An unspecified length inherits `port_span`.
    pub perpendicular_port_span: Option<f64>,
    /// Position of a left/right anchor measured from the top of the box.
    pub anchor_y: Option<f64>,
    /// Allows automatic incoming ports on a compact circular terminal to use the top and
    /// bottom faces in horizontal layouts.
    pub split_incoming_ports: bool,
    /// Flow axis for which this rotatable box and its centered faces were measured.
    /// Responsive trials can transpose the box and its face spans for another local axis.
    pub flow_relative_axis: Option<Axis>,
    pub seq: u32,
    pub order_hint: Option<OrderHint>,
    /// Local flow direction of the previous frame, if known.
    pub order_hint_direction: Option<Direction>,
    /// Used alongside a matching `order_hint` and explicit `order_hint_direction`.
    pub position_hint: Option<PositionHint>,
    pub(crate) current_incident_signature: Option<u64>,
    pub(crate) internal_order_hint: Option<OrderHint>,
    pub(crate) boundary: Option<crate::flat::BoundaryRank>,
    pub(crate) pin: i8,
    pub(crate) group_proxy: bool,
}

impl NodeSpec {
    pub fn new(size: Size, seq: u32) -> Self {
        Self {
            size,
            parent: None,
            contour: ShapeContourHint::Rectangle,
            port_span: None,
            perpendicular_port_span: None,
            anchor_y: None,
            split_incoming_ports: false,
            flow_relative_axis: None,
            seq,
            order_hint: None,
            order_hint_direction: None,
            position_hint: None,
            current_incident_signature: None,
            internal_order_hint: None,
            boundary: None,
            pin: 0,
            group_proxy: false,
        }
    }

    /// Supplies an editor anchor and the current incident signature in the same identity domain.
    /// A node receives full anchor weight when its size and both known signatures match.
    pub fn set_position_hint(
        &mut self,
        hint: PositionHint,
        current_incident_signature: Option<u64>,
    ) {
        self.position_hint = Some(hint);
        self.current_incident_signature = current_incident_signature;
    }

    /// Supplies the current incident signature when capturing a frame for component reuse.
    pub fn set_current_incident_signature(&mut self, signature: Option<u64>) {
        self.current_incident_signature = signature;
    }

    pub(crate) fn effective_order_hint(&self) -> Option<OrderHint> {
        self.internal_order_hint.or(self.order_hint)
    }

    pub(crate) fn editor_order_hint(&self) -> Option<OrderHint> {
        self.order_hint
            .filter(|_| self.boundary.is_none() && !self.group_proxy)
    }

    pub(crate) fn order_hint_in(&self, direction: Direction) -> Option<OrderHint> {
        self.order_hint.filter(|_| {
            self.order_hint_direction
                .is_none_or(|hint| hint == direction)
        })
    }

    pub(crate) fn position_hint_in(&self, direction: Direction, rank: u32) -> Option<(f64, bool)> {
        let hint = self.position_hint?;
        if self.pin != 0
            || self.order_hint_direction != Some(direction)
            || u32::try_from(self.editor_order_hint()?.rank).ok()? != rank
            || !hint.center.x.is_finite()
            || !hint.center.y.is_finite()
            || !crate::validate::measured(hint.size)
        {
            return None;
        }
        let cross = match direction.axis() {
            Axis::Vertical => hint.center.x,
            Axis::Horizontal => hint.center.y,
        };
        let unchanged = hint.size == self.size
            && hint.incident_signature.is_some()
            && hint.incident_signature == self.current_incident_signature;
        Some((cross, unchanged))
    }

    pub(crate) fn align_flow(&mut self, direction: Direction) {
        let axis = direction.axis();
        if self
            .flow_relative_axis
            .is_some_and(|measured| measured != axis)
        {
            std::mem::swap(&mut self.size.width, &mut self.size.height);
            std::mem::swap(&mut self.port_span, &mut self.perpendicular_port_span);
            if let ShapeContourHint::Rounded { rx, ry } = &mut self.contour {
                std::mem::swap(rx, ry);
            }
            if let ShapeContourHint::OffsetDiamond { x, y } = &mut self.contour {
                std::mem::swap(x, y);
            }
            self.flow_relative_axis = Some(axis);
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
    marker_runways: [f64; 2],
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
            marker_runways: [0.0; 2],
        }
    }

    /// Records the measured contact-to-turn distances for both marker footprints.
    /// Set this after assigning the marker metrics. Values must be finite and nonnegative;
    /// zero (or any value below the marker's minimum lead) derives a fallback from the footprint.
    pub fn set_marker_runways(&mut self, runways: [f64; 2]) {
        self.marker_runways = runways;
    }

    pub(crate) fn raw_marker_runways(&self) -> [f64; 2] {
        self.marker_runways
    }

    pub(crate) fn marker_runways(&self) -> [f64; 2] {
        self.resolve_marker_runways(MarkerMetrics::runway_bound)
    }

    pub(crate) fn sweep_runways(&self) -> [f64; 2] {
        self.resolve_marker_runways(|marker| marker.length + MIN_MARKER_LEAD)
    }

    fn resolve_marker_runways(&self, fallback: impl Fn(MarkerMetrics) -> f64) -> [f64; 2] {
        let resolve = |marker: MarkerMetrics, measured: f64| {
            if marker == MarkerMetrics::default() {
                0.0
            } else if measured.is_finite() && measured >= marker.length.max(MIN_MARKER_LEAD) {
                measured
            } else {
                fallback(marker)
            }
        };
        [
            resolve(self.start_marker, self.marker_runways[0]),
            resolve(self.end_marker, self.marker_runways[1]),
        ]
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
    /// Root groups that form swimlanes, in physical left-to-right or top-to-bottom order.
    /// Each root group must appear exactly once and every node must belong to a lane through
    /// its parent chain. Callers can assign loose items to a synthetic root group in this list.
    /// An empty list uses ordinary graph layout. Lanes use [`crate::WidthPolicy::Natural`] and
    /// center their measured title on the start side of the graph's flow. The padding on that
    /// side is the minimum thickness reserved for the header. Nested groups use their usual
    /// direction, title placement and intermediate border contacts.
    /// Edges to a lane shell are routed against its final box and do not rank lane contents.
    pub lane_order: Vec<GroupId>,
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
    /// Stroke radius certified clear for the canonical cubic in a flat rank corridor.
    /// Zero asks the painter to check obstacles. Applies to unchanged box contacts.
    pub corridor_clearance: f64,
    pub(crate) shared_bus: bool,
    pub reversed_for_ranking: bool,
}

#[non_exhaustive]
#[derive(Clone, Debug, PartialEq)]
pub struct GroupGeometry {
    /// Enclosing group after parent-cycle repair.
    pub parent: Option<GroupId>,
    pub rect: Rect,
    pub title_center: Option<Point>,
    /// A route still passes through the title's footprint after automatic placement.
    pub title_struck: bool,
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
    /// Local ranks in node input order when [`crate::Options::capture_node_ranks`] is enabled.
    /// Disconnected components and compound containers have their own rank origins.
    /// Direct contents of swimlanes share a rank origin even across disconnected components.
    pub node_ranks: Vec<u32>,
    pub groups: Vec<GroupGeometry>,
    pub edges: Vec<EdgeGeometry>,
    pub truncated_routes: Vec<TruncatedRoute>,
    pub border_stitches: Vec<BorderStitch>,
    pub direction: Direction,
    pub bounds: Size,
    pub stats: LayoutStats,
}

/// Geometry in input order, with the same coordinate origin as [`LayoutOutput`].
#[non_exhaustive]
#[derive(Clone, Debug, PartialEq)]
pub struct LayoutGeometry {
    pub nodes: Vec<Rect>,
    /// Local ranks in node input order when [`crate::Options::capture_node_ranks`] is enabled.
    pub node_ranks: Vec<u32>,
    pub groups: Vec<GroupGeometry>,
    pub edges: Vec<EdgeGeometry>,
    pub truncated_routes: Vec<TruncatedRoute>,
    pub border_stitches: Vec<BorderStitch>,
    pub direction: Direction,
    pub bounds: Size,
}

impl From<LayoutOutput> for LayoutGeometry {
    fn from(output: LayoutOutput) -> Self {
        let LayoutOutput {
            nodes,
            node_ranks,
            groups,
            edges,
            truncated_routes,
            border_stitches,
            direction,
            bounds,
            stats: _,
        } = output;
        Self {
            nodes,
            node_ranks,
            groups,
            edges,
            truncated_routes,
            border_stitches,
            direction,
            bounds,
        }
    }
}
