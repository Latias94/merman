use super::{
    AXES, ArcRows, DirectedArc, GridRoutingTopology, GridSearchArc, GridSearchTopology,
    GridTopologyError, GridTopologyLimit, GridTopologyVertex, GridVertexKind, allocation_failed,
    append_arc_pair, append_rays, charge_lookup, check_cap, checked_add, estimate, fixed,
    normalize_point, order_vertex_lines, record_order, reserved, sort_arcs, sort_metered, varying,
};
use crate::graph_layout::Axis;
use crate::resources::OperationWorkMeter;
use kurbo::Point;
use merman_model::OperationPhase;

const MAX_ADDED_VERTICES: usize = 32;
const MAX_ADDED_ARCS: usize = 64;

#[derive(Clone, Copy, Debug)]
pub struct GridOverlayInput<'a> {
    pub source: Point,
    pub target: Point,
    pub lane_points: &'a [Point],
}

/// Only added slots and overridden adjacency rows are retained between attempts.
#[derive(Debug, Default)]
pub struct GridEndpointOverlayScratch {
    vertices: Vec<GridTopologyVertex<'static>>,
    edges: Vec<DirectedArc>,
    overridden: Vec<usize>,
    rows: ArcRows,
}

/// The borrow keeps scratch from being reset until this endpoint pair is finished.
pub struct GridRoutingOverlay<'base, 'id> {
    base: &'base GridRoutingTopology<'id>,
    scratch: &'base GridEndpointOverlayScratch,
    vertex_count: usize,
    estimated_bytes: usize,
}

impl<'id> GridRoutingTopology<'id> {
    pub fn overlay<'base>(
        &'base self,
        input: GridOverlayInput<'_>,
        scratch: &'base mut GridEndpointOverlayScratch,
        work: &OperationWorkMeter,
    ) -> Result<GridRoutingOverlay<'base, 'id>, GridTopologyError> {
        work.checkpoint(OperationPhase::Layout)?;
        scratch.vertices.clear();
        scratch.edges.clear();
        scratch.overridden.clear();

        let source = normalize_point(input.source)?;
        let target = normalize_point(input.target)?;
        for (point, owner_id) in [(source, "source"), (target, "target")] {
            scratch.add_record(
                self,
                GridTopologyVertex {
                    point,
                    kind: GridVertexKind::Endpoint,
                    owner_id: Some(owner_id),
                    side: None,
                },
                work,
            )?;
        }
        for &point in input.lane_points {
            scratch.add_if_clear(self, normalize_point(point)?, GridVertexKind::Lane, work)?;
        }
        for point in [
            Point::new(source.x, target.y),
            Point::new(target.x, source.y),
        ] {
            scratch.add_if_clear(self, point, GridVertexKind::Projection, work)?;
        }
        let mut rays = reserved(8, work)?;
        for point in [source, target] {
            append_rays(
                point,
                self.bounds,
                &self.horizontal,
                &self.vertical,
                &mut rays,
                work,
            )?;
        }
        for ray in rays {
            scratch.add_record(self, ray, work)?;
        }
        sort_metered(&mut scratch.vertices, 7, record_order, work)?;

        for axis in AXES {
            scratch.connect_lines(self, axis, work)?;
        }
        let estimated_bytes = checked_add(
            self.estimated_bytes,
            estimate(scratch.vertices.len(), scratch.edges.len(), 0, 0, work)?,
            work,
        )?;
        let vertex_count = checked_add(self.vertices.len(), scratch.vertices.len(), work)?;

        // Keep unchanged arcs in modified base rows. An overlay inserts hops onto the original
        // graph and deliberately keeps any original arc that spans a new vertex.
        let row_count = checked_add(scratch.edges.len(), scratch.vertices.len(), work)?;
        work.charge(row_count)?;
        scratch
            .overridden
            .try_reserve_exact(row_count)
            .map_err(|_| allocation_failed())?;
        scratch
            .overridden
            .extend(scratch.edges.iter().map(|arc| arc.from));
        scratch.overridden.extend(self.vertices.len()..vertex_count);
        sort_metered(&mut scratch.overridden, 1, usize::cmp, work)?;
        scratch.overridden.dedup();
        for &vertex in &scratch.overridden {
            let old_arcs = self.outgoing(vertex);
            work.charge(checked_add(old_arcs.len(), 1, work)?)?;
            scratch
                .edges
                .try_reserve(old_arcs.len())
                .map_err(|_| allocation_failed())?;
            scratch.edges.extend(
                old_arcs
                    .iter()
                    .map(|&arc| DirectedArc { from: vertex, arc }),
            );
        }
        let point_at = |id: usize| {
            if id < self.vertices.len() {
                self.vertices[id].point
            } else {
                scratch.vertices[id - self.vertices.len()].point
            }
        };
        sort_arcs(&mut scratch.edges, point_at, work)?;
        scratch.rows.fill(
            scratch.overridden.iter().copied(),
            scratch.overridden.len(),
            &scratch.edges,
            work,
        )?;
        work.checkpoint(OperationPhase::Layout)?;
        Ok(GridRoutingOverlay {
            base: self,
            scratch,
            vertex_count,
            estimated_bytes,
        })
    }
}

impl GridEndpointOverlayScratch {
    fn add_if_clear(
        &mut self,
        base: &GridRoutingTopology<'_>,
        point: Point,
        kind: GridVertexKind,
        work: &OperationWorkMeter,
    ) -> Result<(), GridTopologyError> {
        if !base.horizontal.contains(point.y, point.x, work)? {
            self.add_record(
                base,
                GridTopologyVertex {
                    point,
                    kind,
                    owner_id: None,
                    side: None,
                },
                work,
            )?;
        }
        Ok(())
    }

    fn add_record(
        &mut self,
        base: &GridRoutingTopology<'_>,
        mut record: GridTopologyVertex<'static>,
        work: &OperationWorkMeter,
    ) -> Result<(), GridTopologyError> {
        record.point = normalize_point(record.point)?;
        if base.vertex_id(record.point, work)?.is_some() {
            return Ok(());
        }
        work.charge(checked_add(self.vertices.len(), 1, work)?)?;
        if let Some(current) = self
            .vertices
            .iter_mut()
            .find(|vertex| vertex.point == record.point)
        {
            if record_order(&record, current).is_lt() {
                *current = record;
            }
            return Ok(());
        }
        let count = self.vertices.len() + 1;
        check_cap(
            count,
            MAX_ADDED_VERTICES,
            |actual, max| GridTopologyLimit::Vertices { actual, max },
            work,
        )?;
        self.vertices
            .try_reserve(1)
            .map_err(|_| allocation_failed())?;
        self.vertices.push(record);
        Ok(())
    }

    fn connect_lines(
        &mut self,
        base: &GridRoutingTopology<'_>,
        axis: Axis,
        work: &OperationWorkMeter,
    ) -> Result<(), GridTopologyError> {
        let ordered = order_vertex_lines(&self.vertices, axis, work)?;
        let base_count = base.vertices.len();
        let index = base.intervals(axis);
        for (position, &id) in ordered.iter().enumerate() {
            let point = self.vertices[id].point;
            let fixed_coord = fixed(axis, point);
            let coordinate = varying(axis, point);
            let slot = checked_add(base_count, id, work)?;
            let on_line = |candidate: Point| fixed(axis, candidate) == fixed_coord;
            let base_line = base.line(axis, fixed_coord, work)?;
            charge_lookup(base_line.len(), work)?;
            let after = base_line.partition_point(|&base_id| {
                varying(axis, base.vertices[base_id].point) < coordinate
            });
            let left_base = after.checked_sub(1).map(|previous| {
                let id = base_line[previous];
                (id, base.vertices[id].point)
            });
            let left_added = position
                .checked_sub(1)
                .map(|previous| ordered[previous])
                .and_then(|other| {
                    let point = self.vertices[other].point;
                    on_line(point).then_some((other, point))
                });
            let left = match (left_base, left_added) {
                (Some(base), Some((_, added))) if varying(axis, base.1) > varying(axis, added) => {
                    Some(base)
                }
                (_, Some((other, point))) => Some((checked_add(base_count, other, work)?, point)),
                (base, None) => base,
            };
            if let Some((left, before)) = left
                && !index.intersects(fixed_coord, varying(axis, before), coordinate, work)?
            {
                self.connect(base, [left, slot], [before, point], axis, work)?;
            }

            // An added right neighbor will do its own left lookup. Only a base neighbor
            // needs the second, forward lookup here.
            if let Some(&right) = base_line.get(after) {
                let next = base.vertices[right].point;
                let earlier_added = ordered.get(position + 1).is_some_and(|&other| {
                    let candidate = self.vertices[other].point;
                    on_line(candidate) && varying(axis, candidate) < varying(axis, next)
                });
                if !earlier_added
                    && !index.intersects(fixed_coord, coordinate, varying(axis, next), work)?
                {
                    self.connect(base, [slot, right], [point, next], axis, work)?;
                }
            }
        }
        Ok(())
    }

    fn connect(
        &mut self,
        base: &GridRoutingTopology<'_>,
        slots: [usize; 2],
        points: [Point; 2],
        axis: Axis,
        work: &OperationWorkMeter,
    ) -> Result<(), GridTopologyError> {
        work.charge(2)?;
        let base_entries = base.adjacency_entries();
        let new_entries = checked_add(self.edges.len(), 2, work)?;
        check_cap(
            checked_add(base_entries, new_entries, work)?,
            checked_add(base_entries, MAX_ADDED_ARCS, work)?,
            |actual, max| GridTopologyLimit::Adjacency { actual, max },
            work,
        )?;
        append_arc_pair(&mut self.edges, slots, points, axis)?;
        Ok(())
    }
}

impl<'id> GridRoutingOverlay<'_, 'id> {
    pub fn vertex(&self, id: usize) -> Option<GridTopologyVertex<'id>> {
        if id < self.base.vertices.len() {
            self.base.vertices.get(id).copied()
        } else {
            self.scratch
                .vertices
                .get(id - self.base.vertices.len())
                .copied()
        }
    }

    pub fn vertex_id(
        &self,
        point: Point,
        work: &OperationWorkMeter,
    ) -> crate::Result<Option<usize>> {
        let point = normalize_point(point)?;
        if let Some(id) = self.base.vertex_id(point, work)? {
            return Ok(Some(id));
        }
        work.charge(self.scratch.vertices.len())?;
        Ok(self
            .scratch
            .vertices
            .iter()
            .position(|vertex| vertex.point == point)
            .map(|index| self.base.vertices.len() + index))
    }

    pub const fn estimated_bytes(&self) -> usize {
        self.estimated_bytes
    }
}

impl GridSearchTopology for GridRoutingOverlay<'_, '_> {
    fn vertex_count(&self) -> usize {
        self.vertex_count
    }

    fn point(&self, id: usize) -> Option<Point> {
        self.vertex(id).map(|vertex| vertex.point)
    }

    fn outgoing(&self, id: usize) -> &[GridSearchArc] {
        match self.scratch.overridden.binary_search(&id) {
            Ok(row) => self.scratch.rows.outgoing(row),
            Err(_) => self.base.outgoing(id),
        }
    }
}
