//! Immutable per-container visibility topology for Mermaid 12.4 Grid routing.

use super::sparse_search::{GridSearchArc, GridSearchTopology};
use crate::Error;
use crate::graph_layout::{Axis, Side};
use crate::resources::{OperationWorkError, OperationWorkMeter};
use kurbo::{Point, Rect};
use merman_model::OperationPhase;
use std::cmp::Ordering;

mod intervals;
mod overlay;
mod portals;

pub use intervals::GridIntervalIndex;
pub use overlay::{GridEndpointOverlayScratch, GridOverlayInput};
pub use portals::{GridPairedPortal, grid_paired_portal, grid_portal_ranges};

const CLEARANCE: f64 = 6.0;
const SIDES: [Side; 4] = [Side::Left, Side::Right, Side::Top, Side::Bottom];
const AXES: [Axis; 2] = [Axis::Horizontal, Axis::Vertical];
const MAX_BASE_VERTICES: usize = 50_000;
pub const GRID_ROUTING_ESTIMATED_BYTES: usize = 64 * 1024 * 1024;

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct GridTopologyObstacle<'a> {
    pub id: &'a str,
    pub bounds: Rect,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct GridPortalRange<'a> {
    pub owner_id: &'a str,
    pub side: Side,
    pub low: f64,
    pub high: f64,
    /// Fixed coordinate of the portal line. Without an offset, use the container boundary.
    pub coordinate: Option<f64>,
}

#[derive(Clone, Copy, Debug)]
pub struct GridTopologyInput<'input, 'id> {
    pub container_id: &'id str,
    pub bounds: Rect,
    pub obstacles: &'input [GridTopologyObstacle<'id>],
    pub title_exclusions: &'input [GridTopologyObstacle<'id>],
    pub portal_ranges: &'input [GridPortalRange<'id>],
}

#[derive(Clone, Copy, Debug)]
pub struct GridTopologyCaps {
    pub max_vertices: usize,
    /// Defaults to four times the vertex cap.
    pub max_adjacency_entries: Option<usize>,
    pub max_estimated_bytes: usize,
}

impl Default for GridTopologyCaps {
    fn default() -> Self {
        Self {
            max_vertices: MAX_BASE_VERTICES,
            max_adjacency_entries: None,
            max_estimated_bytes: GRID_ROUTING_ESTIMATED_BYTES,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, thiserror::Error)]
pub enum GridTopologyLimit {
    #[error("Grid routing vertex_cap exceeded: {actual} > {max}")]
    Vertices { actual: usize, max: usize },
    #[error("Grid routing adjacency_cap exceeded: {actual} > {max}")]
    Adjacency { actual: usize, max: usize },
    #[error("Grid routing estimated_memory_cap exceeded: {actual} > {max}")]
    EstimatedMemory { actual: usize, max: usize },
}

#[derive(Debug, thiserror::Error)]
pub enum GridTopologyError {
    #[error(transparent)]
    Limit(#[from] GridTopologyLimit),
    #[error(transparent)]
    Layout(#[from] Error),
}

impl From<OperationWorkError> for GridTopologyError {
    fn from(value: OperationWorkError) -> Self {
        Self::Layout(value.into())
    }
}

/// Discriminants follow the upstream lexical order.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum GridVertexKind {
    Corner,
    Endpoint,
    Lane,
    Portal,
    Projection,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct GridTopologyVertex<'a> {
    pub point: Point,
    pub kind: GridVertexKind,
    pub owner_id: Option<&'a str>,
    pub side: Option<Side>,
}

/// Owns the sparse data and borrows authored identifiers for their full lifetime.
#[derive(Debug)]
pub struct GridRoutingTopology<'a> {
    bounds: Rect,
    horizontal: GridIntervalIndex,
    vertical: GridIntervalIndex,
    vertices: Vec<GridTopologyVertex<'a>>,
    by_x: Vec<usize>,
    by_y: Vec<usize>,
    rows: ArcRows,
    estimated_bytes: usize,
}

impl<'a> GridRoutingTopology<'a> {
    pub fn build(
        input: GridTopologyInput<'_, 'a>,
        caps: GridTopologyCaps,
        work: &OperationWorkMeter,
    ) -> Result<Self, GridTopologyError> {
        work.checkpoint(OperationPhase::Layout)?;
        let bounds = normalize_rect(input.bounds)?;
        let obstacles = intervals::obstacle_union(input, bounds, caps.max_estimated_bytes, work)?;
        let portal_ranges = ordered_portal_ranges(input.portal_ranges, work)?;
        let mut vertices = seeds(bounds, input.container_id, &obstacles, &portal_ranges, work)?;
        let seed_count = vertices.len();
        let mut budget = intervals::IntervalBudget::new(obstacles.len(), caps.max_estimated_bytes);
        let horizontal = GridIntervalIndex::build(&obstacles, Axis::Horizontal, &mut budget, work)?;
        let vertical = GridIntervalIndex::build(&obstacles, Axis::Vertical, &mut budget, work)?;

        let ray_count = checked_mul(seed_count, SIDES.len(), work)?;
        work.charge(ray_count)?;
        vertices
            .try_reserve_exact(ray_count)
            .map_err(|_| allocation_failed())?;
        for index in 0..seed_count {
            append_rays(
                vertices[index].point,
                bounds,
                &horizontal,
                &vertical,
                &mut vertices,
                work,
            )?;
        }
        canonicalize(&mut vertices, work)?;
        let vertex_count = vertices.len();
        check_cap(
            vertex_count,
            caps.max_vertices,
            |actual, max| GridTopologyLimit::Vertices { actual, max },
            work,
        )?;

        let by_y = order_vertex_lines(&vertices, Axis::Horizontal, work)?;
        let by_x = order_vertex_lines(&vertices, Axis::Vertical, work)?;
        let mut edges = Vec::new();
        for (axis, line_order, index) in [
            (Axis::Horizontal, &by_y, &horizontal),
            (Axis::Vertical, &by_x, &vertical),
        ] {
            for pair in line_order.windows(2) {
                work.charge(1)?;
                let a = vertices[pair[0]].point;
                let b = vertices[pair[1]].point;
                if fixed(axis, a) == fixed(axis, b)
                    && !index.intersects(
                        fixed(axis, a),
                        varying(axis, a),
                        varying(axis, b),
                        work,
                    )?
                {
                    append_arc_pair(&mut edges, [pair[0], pair[1]], [a, b], axis)?;
                }
            }
        }
        let adjacency_entries = edges.len();
        let max_adjacency = caps
            .max_adjacency_entries
            .unwrap_or_else(|| caps.max_vertices.saturating_mul(4));
        check_cap(
            adjacency_entries,
            max_adjacency,
            |actual, max| GridTopologyLimit::Adjacency { actual, max },
            work,
        )?;
        let estimated_bytes = estimate(
            vertex_count,
            adjacency_entries,
            obstacles.len(),
            budget.count,
            work,
        )?;
        check_estimate(estimated_bytes, caps.max_estimated_bytes, work)?;

        sort_arcs(&mut edges, |id| vertices[id].point, work)?;
        let mut rows = ArcRows::default();
        rows.fill(0..vertex_count, vertex_count, &edges, work)?;
        work.checkpoint(OperationPhase::Layout)?;
        Ok(Self {
            bounds,
            horizontal,
            vertical,
            vertices,
            by_x,
            by_y,
            rows,
            estimated_bytes,
        })
    }

    pub const fn estimated_bytes(&self) -> usize {
        self.estimated_bytes
    }

    pub fn adjacency_entries(&self) -> usize {
        self.rows.arcs.len()
    }

    pub fn intervals(&self, axis: Axis) -> &GridIntervalIndex {
        match axis {
            Axis::Horizontal => &self.horizontal,
            Axis::Vertical => &self.vertical,
        }
    }

    pub fn vertex_id(
        &self,
        point: Point,
        work: &OperationWorkMeter,
    ) -> crate::Result<Option<usize>> {
        let point = normalize_point(point)?;
        charge_lookup(self.by_x.len(), work)?;
        Ok(self
            .by_x
            .binary_search_by(|&id| point_order(self.vertices[id].point, point))
            .ok()
            .map(|index| self.by_x[index]))
    }

    fn line(&self, axis: Axis, at: f64, work: &OperationWorkMeter) -> crate::Result<&[usize]> {
        let ordered = match axis {
            Axis::Horizontal => &self.by_y,
            Axis::Vertical => &self.by_x,
        };
        charge_lookup(ordered.len(), work)?;
        let start = ordered.partition_point(|&id| fixed(axis, self.vertices[id].point) < at);
        let tail = &ordered[start..];
        charge_lookup(tail.len(), work)?;
        let count = tail.partition_point(|&id| fixed(axis, self.vertices[id].point) == at);
        Ok(&tail[..count])
    }
}

impl GridSearchTopology for GridRoutingTopology<'_> {
    fn vertex_count(&self) -> usize {
        self.vertices.len()
    }

    fn point(&self, vertex: usize) -> Option<Point> {
        self.vertices.get(vertex).map(|vertex| vertex.point)
    }

    fn outgoing(&self, vertex: usize) -> &[GridSearchArc] {
        self.rows.outgoing(vertex)
    }
}

#[derive(Clone, Copy, Debug)]
struct DirectedArc {
    from: usize,
    arc: GridSearchArc,
}

#[derive(Debug, Default)]
struct ArcRows {
    offsets: Vec<usize>,
    arcs: Vec<GridSearchArc>,
}

impl ArcRows {
    fn fill(
        &mut self,
        sources: impl IntoIterator<Item = usize>,
        row_count: usize,
        arcs: &[DirectedArc],
        work: &OperationWorkMeter,
    ) -> crate::Result<()> {
        let offset_count = checked_add(row_count, 1, work)?;
        work.charge(checked_add(offset_count, arcs.len(), work)?)?;
        self.offsets.clear();
        self.arcs.clear();
        self.offsets
            .try_reserve_exact(offset_count)
            .map_err(|_| allocation_failed())?;
        self.arcs
            .try_reserve_exact(arcs.len())
            .map_err(|_| allocation_failed())?;
        let mut remaining = arcs.iter().peekable();
        for source in sources {
            self.offsets.push(self.arcs.len());
            while remaining.peek().is_some_and(|arc| arc.from == source) {
                if let Some(edge) = remaining.next() {
                    self.arcs.push(edge.arc);
                }
            }
        }
        self.offsets.push(self.arcs.len());
        work.checkpoint(OperationPhase::Layout)?;
        Ok(())
    }

    fn outgoing(&self, row: usize) -> &[GridSearchArc] {
        if row < self.offsets.len().saturating_sub(1) {
            &self.arcs[self.offsets[row]..self.offsets[row + 1]]
        } else {
            &[]
        }
    }
}

fn append_arc_pair(
    edges: &mut Vec<DirectedArc>,
    [a, b]: [usize; 2],
    [first, second]: [Point; 2],
    axis: Axis,
) -> crate::Result<()> {
    let length = (varying(axis, first) - varying(axis, second)).abs();
    if !length.is_finite() || length <= 0.0 {
        return Err(invalid("Invalid Grid visibility arc length"));
    }
    edges.try_reserve(2).map_err(|_| allocation_failed())?;
    edges.extend([(a, b), (b, a)].map(|(from, to)| DirectedArc {
        from,
        arc: GridSearchArc {
            to,
            axis,
            length,
            crosses_boundary: false,
        },
    }));
    Ok(())
}

fn sort_arcs(
    edges: &mut [DirectedArc],
    point_at: impl Fn(usize) -> Point,
    work: &OperationWorkMeter,
) -> crate::Result<()> {
    // The base and the endpoint overlay only create visibility arcs. A paired portal is a
    // separate hierarchy transition, not a visibility arc between two containers.
    sort_metered(
        edges,
        1,
        |a, b| {
            a.from
                .cmp(&b.from)
                .then_with(|| axis_ordinal(a.arc.axis).cmp(&axis_ordinal(b.arc.axis)))
                .then_with(|| point_order(point_at(a.arc.to), point_at(b.arc.to)))
                .then_with(|| a.arc.to.cmp(&b.arc.to))
        },
        work,
    )
}

fn ordered_portal_ranges<'a>(
    ranges: &[GridPortalRange<'a>],
    work: &OperationWorkMeter,
) -> crate::Result<Vec<GridPortalRange<'a>>> {
    let mut ordered = reserved(ranges.len(), work)?;
    let mut id_length = 0;
    for (index, &range) in ranges.iter().enumerate() {
        work.charge(1)?;
        if !range.low.is_finite()
            || !range.high.is_finite()
            || range
                .coordinate
                .is_some_and(|coordinate| !coordinate.is_finite())
        {
            return Err(invalid("Invalid Grid portal range"));
        }
        id_length = id_length.max(range.owner_id.len());
        ordered.push((index, range));
    }
    sort_metered(
        &mut ordered,
        checked_add(id_length, 1, work)?,
        |(ai, a), (bi, b)| {
            side_ordinal(Some(a.side))
                .cmp(&side_ordinal(Some(b.side)))
                .then_with(|| coordinate_order(a.low, b.low))
                .then_with(|| coordinate_order(a.high, b.high))
                .then_with(|| code_units(a.owner_id, b.owner_id))
                .then_with(|| ai.cmp(bi))
        },
        work,
    )?;
    let mut sorted = reserved(ordered.len(), work)?;
    sorted.extend(ordered.into_iter().map(|(_, range)| range));
    Ok(sorted)
}

fn seeds<'a>(
    bounds: Rect,
    container: &'a str,
    obstacles: &[GridTopologyObstacle<'a>],
    portals: &[GridPortalRange<'a>],
    work: &OperationWorkMeter,
) -> crate::Result<Vec<GridTopologyVertex<'a>>> {
    let corners = checked_mul(checked_add(obstacles.len(), 1, work)?, 4, work)?;
    let mut records = reserved(
        checked_add(corners, checked_mul(portals.len(), 2, work)?, work)?,
        work,
    )?;
    add_corners(bounds, container, &mut records);
    for obstacle in obstacles {
        work.checkpoint(OperationPhase::Layout)?;
        add_corners(obstacle.bounds, obstacle.id, &mut records);
    }
    for portal in portals {
        work.checkpoint(OperationPhase::Layout)?;
        let axis = portal.side.axis();
        let normal = portal
            .coordinate
            .unwrap_or_else(|| side_coordinate(bounds, portal.side));
        for tangent in [portal.low, portal.high] {
            records.push(GridTopologyVertex {
                point: fixed_point(axis, normal, tangent),
                kind: GridVertexKind::Portal,
                owner_id: Some(portal.owner_id),
                side: Some(portal.side),
            });
        }
    }
    canonicalize(&mut records, work)?;
    Ok(records)
}

fn add_corners<'a>(bounds: Rect, owner: &'a str, records: &mut Vec<GridTopologyVertex<'a>>) {
    for (x, y) in [
        (bounds.x0, bounds.y0),
        (bounds.x1, bounds.y0),
        (bounds.x0, bounds.y1),
        (bounds.x1, bounds.y1),
    ] {
        records.push(GridTopologyVertex {
            point: Point::new(x, y),
            kind: GridVertexKind::Corner,
            owner_id: Some(owner),
            side: None,
        });
    }
}

fn append_rays(
    point: Point,
    bounds: Rect,
    horizontal: &GridIntervalIndex,
    vertical: &GridIntervalIndex,
    vertices: &mut Vec<GridTopologyVertex<'_>>,
    work: &OperationWorkMeter,
) -> crate::Result<()> {
    for side in SIDES {
        let axis = side.axis();
        let is_start = matches!(side, Side::Left | Side::Top);
        let index = match axis {
            Axis::Horizontal => horizontal,
            Axis::Vertical => vertical,
        };
        let ray = index
            .nearest_boundary(fixed(axis, point), varying(axis, point), is_start, work)?
            .unwrap_or_else(|| side_coordinate(bounds, side));
        if ray != varying(axis, point) {
            vertices.push(GridTopologyVertex {
                point: fixed_point(axis, ray, fixed(axis, point)),
                kind: GridVertexKind::Projection,
                owner_id: None,
                side: Some(side),
            });
        }
    }
    Ok(())
}

fn canonicalize(
    records: &mut Vec<GridTopologyVertex<'_>>,
    work: &OperationWorkMeter,
) -> crate::Result<()> {
    let mut id_length = 0;
    for record in records.iter_mut() {
        work.charge(1)?;
        record.point = normalize_point(record.point)?;
        id_length = id_length.max(record.owner_id.map_or(0, str::len));
    }
    let comparison_work = checked_add(id_length, 1, work)?;
    // Sort by point to collect duplicates, then select the same first record as the reference's
    // kind/side/point/owner sort. The final order determines vertex slots.
    sort_metered(
        records,
        comparison_work,
        |a, b| point_order(a.point, b.point).then_with(|| record_order(a, b)),
        work,
    )?;
    work.charge(records.len())?;
    records.dedup_by(|b, a| a.point == b.point);
    sort_metered(records, comparison_work, record_order, work)
}

fn record_order(a: &GridTopologyVertex<'_>, b: &GridTopologyVertex<'_>) -> Ordering {
    a.kind
        .cmp(&b.kind)
        .then_with(|| side_ordinal(a.side).cmp(&side_ordinal(b.side)))
        .then_with(|| point_order(a.point, b.point))
        .then_with(|| {
            code_units(
                a.owner_id.unwrap_or_default(),
                b.owner_id.unwrap_or_default(),
            )
        })
}

fn order_vertex_lines(
    vertices: &[GridTopologyVertex<'_>],
    axis: Axis,
    work: &OperationWorkMeter,
) -> crate::Result<Vec<usize>> {
    let mut ordered = reserved(vertices.len(), work)?;
    ordered.extend(0..vertices.len());
    sort_metered(
        &mut ordered,
        1,
        |&a, &b| line_order(axis, vertices[a].point, vertices[b].point).then_with(|| a.cmp(&b)),
        work,
    )?;
    Ok(ordered)
}

fn line_order(axis: Axis, a: Point, b: Point) -> Ordering {
    coordinate_order(fixed(axis, a), fixed(axis, b))
        .then_with(|| coordinate_order(varying(axis, a), varying(axis, b)))
}

fn point_order(a: Point, b: Point) -> Ordering {
    line_order(Axis::Vertical, a, b)
}

fn coordinate_order(a: f64, b: f64) -> Ordering {
    if a == b {
        Ordering::Equal
    } else {
        a.total_cmp(&b)
    }
}

fn code_units(a: &str, b: &str) -> Ordering {
    a.encode_utf16().cmp(b.encode_utf16())
}

fn axis_ordinal(axis: Axis) -> u8 {
    match axis {
        Axis::Horizontal => 0,
        Axis::Vertical => 1,
    }
}

fn side_ordinal(side: Option<Side>) -> u8 {
    match side {
        Some(Side::Left) => 0,
        Some(Side::Right) => 1,
        Some(Side::Top) => 2,
        Some(Side::Bottom) => 3,
        None => 4,
    }
}

fn side_coordinate(bounds: Rect, side: Side) -> f64 {
    match side {
        Side::Left => bounds.x0,
        Side::Right => bounds.x1,
        Side::Top => bounds.y0,
        Side::Bottom => bounds.y1,
    }
}

fn fixed(axis: Axis, point: Point) -> f64 {
    match axis {
        Axis::Horizontal => point.y,
        Axis::Vertical => point.x,
    }
}

fn varying(axis: Axis, point: Point) -> f64 {
    match axis {
        Axis::Horizontal => point.x,
        Axis::Vertical => point.y,
    }
}

fn fixed_point(axis: Axis, varying: f64, fixed: f64) -> Point {
    match axis {
        Axis::Horizontal => Point::new(varying, fixed),
        Axis::Vertical => Point::new(fixed, varying),
    }
}

fn normalize_point(point: Point) -> crate::Result<Point> {
    if !point.x.is_finite() || !point.y.is_finite() {
        return Err(invalid(
            "Grid routing geometry must contain only finite coordinates",
        ));
    }
    Ok(Point::new(zero(point.x), zero(point.y)))
}

fn normalize_rect(bounds: Rect) -> crate::Result<Rect> {
    let a = normalize_point(Point::new(bounds.x0, bounds.y0))?;
    let b = normalize_point(Point::new(bounds.x1, bounds.y1))?;
    if a.x == b.x || a.y == b.y {
        return Err(invalid("Grid routing rectangles must have positive area"));
    }
    Ok(Rect::new(
        a.x.min(b.x),
        a.y.min(b.y),
        a.x.max(b.x),
        a.y.max(b.y),
    ))
}

fn zero(value: f64) -> f64 {
    if value == 0.0 { 0.0 } else { value }
}

fn estimate(
    vertices: usize,
    arcs: usize,
    obstacles: usize,
    intervals: usize,
    work: &OperationWorkMeter,
) -> crate::Result<usize> {
    [(vertices, 64), (arcs, 56), (obstacles, 48), (intervals, 24)]
        .into_iter()
        .try_fold(0usize, |bytes, (count, each)| {
            bytes.checked_add(count.checked_mul(each)?)
        })
        .ok_or_else(|| work.arithmetic_overflow().into())
}

fn check_estimate(
    actual: usize,
    max: usize,
    work: &OperationWorkMeter,
) -> Result<(), GridTopologyError> {
    check_cap(
        actual,
        max,
        |actual, max| GridTopologyLimit::EstimatedMemory { actual, max },
        work,
    )
}

fn check_cap(
    actual: usize,
    max: usize,
    error: impl FnOnce(usize, usize) -> GridTopologyLimit,
    work: &OperationWorkMeter,
) -> Result<(), GridTopologyError> {
    work.checkpoint(OperationPhase::Layout)?;
    if actual > max {
        Err(error(actual, max).into())
    } else {
        Ok(())
    }
}

fn checked_add(a: usize, b: usize, work: &OperationWorkMeter) -> crate::Result<usize> {
    a.checked_add(b)
        .ok_or_else(|| work.arithmetic_overflow().into())
}

fn checked_mul(a: usize, b: usize, work: &OperationWorkMeter) -> crate::Result<usize> {
    a.checked_mul(b)
        .ok_or_else(|| work.arithmetic_overflow().into())
}

fn reserved<T>(capacity: usize, work: &OperationWorkMeter) -> crate::Result<Vec<T>> {
    work.charge(capacity)?;
    let mut result = Vec::new();
    result
        .try_reserve_exact(capacity)
        .map_err(|_| allocation_failed())?;
    Ok(result)
}

fn charge_lookup(count: usize, work: &OperationWorkMeter) -> crate::Result<()> {
    work.charge(count.checked_ilog2().map_or(0, |depth| depth as usize + 2))?;
    Ok(())
}

fn sort_metered<T>(
    values: &mut [T],
    comparison_work: usize,
    compare: impl FnMut(&T, &T) -> Ordering,
    work: &OperationWorkMeter,
) -> crate::Result<()> {
    let levels = values
        .len()
        .checked_ilog2()
        .map_or(0, |depth| depth as usize + 1);
    work.charge(checked_mul(
        checked_mul(values.len(), levels, work)?,
        comparison_work,
        work,
    )?)?;
    values.sort_unstable_by(compare);
    work.checkpoint(OperationPhase::Layout)?;
    Ok(())
}

fn allocation_failed() -> Error {
    Error::LayoutAllocationFailed { backend: "grid" }
}

fn invalid(message: &'static str) -> Error {
    Error::InvalidModel {
        message: message.into(),
    }
}
