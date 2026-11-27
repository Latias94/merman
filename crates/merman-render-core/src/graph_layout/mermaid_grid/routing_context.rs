//! Measured container views and invocation-wide Grid topology admission.

use super::forest::{GRID_ROOT_ID, GridForest, GridForestGeometry, GridItemKind};
use super::geometry::reserve;
use super::route_plan::{CLEARANCE, PIXEL_EPSILON, code_units, sort_charge};
use super::routing::{GridRoutingChild, GridRoutingView};
use super::sparse_topology::{
    GRID_ROUTING_ESTIMATED_BYTES, GridPortalRange, GridRoutingTopology, GridTopologyCaps,
    GridTopologyError, GridTopologyInput, GridTopologyLimit, GridTopologyObstacle,
    grid_portal_ranges,
};
use crate::graph_layout::{Axis, Rect, Side};
use crate::resources::OperationWorkMeter;
use crate::{Error, Result};
use kurbo::{Point, Rect as Box2};

const ROOT_OUTER_MARGIN: f64 = 24.0;

pub(super) struct GridRoutingContainer<'a> {
    pub id: &'a str,
    pub item_slots: Vec<usize>,
    pub children: Vec<GridRoutingChild>,
    pub horizontal_corridors: &'a [f64],
    pub vertical_corridors: &'a [f64],
    pub domain: Box2,
    title: Option<(String, Box2)>,
}

impl GridRoutingContainer<'_> {
    pub(super) fn view(&self) -> GridRoutingView<'_> {
        GridRoutingView {
            children: &self.children,
            horizontal_corridors: self.horizontal_corridors,
            vertical_corridors: self.vertical_corridors,
        }
    }
}

pub(super) struct GridRoutingContainers<'a> {
    /// Item slots index their containing view's children. The root has no item slot.
    local_slots: Vec<Option<usize>>,
    containers: Vec<Option<GridRoutingContainer<'a>>>,
}

impl<'a> GridRoutingContainers<'a> {
    pub(super) fn build(
        forest: &GridForest<'a>,
        geometry: &'a GridForestGeometry,
        work: &OperationWorkMeter,
    ) -> Result<Self> {
        let container_count = geometry.containers.len();
        work.charge(container_count)?;
        let mut containers = Vec::new();
        let mut local_slots = Vec::new();
        reserve(&mut containers, container_count)?;
        reserve(&mut local_slots, forest.items.len())?;
        local_slots.resize(forest.items.len(), None);
        for (slot, measured) in geometry.containers.iter().enumerate() {
            work.charge(1)?;
            let Some(measured) = measured else {
                containers.push(None);
                continue;
            };
            let id = forest
                .items
                .get(slot)
                .map_or(GRID_ROOT_ID, |item| item.item.id);
            let title = if let Some(bounds) = measured.title_bounds {
                const SUFFIX: &str = ":title";
                let bytes = id
                    .len()
                    .checked_add(SUFFIX.len())
                    .ok_or_else(|| work.arithmetic_overflow())?;
                work.charge(bytes)?;
                let mut title = String::new();
                title
                    .try_reserve_exact(bytes)
                    .map_err(|_| Error::LayoutAllocationFailed { backend: "grid" })?;
                title.push_str(id);
                title.push_str(SUFFIX);
                Some((title, to_box(bounds)))
            } else {
                None
            };
            let mut children = Vec::new();
            let mut item_slots = Vec::new();
            work.charge(measured.items.len())?;
            reserve(&mut children, measured.items.len())?;
            reserve(&mut item_slots, measured.items.len())?;
            let mut root_cells = None::<Box2>;
            for child in &measured.items {
                work.charge(1)?;
                local_slots[child.item_index] = Some(children.len());
                item_slots.push(child.item_index);
                children.push(GridRoutingChild {
                    bounds: to_box(child.bounds),
                    corridors: child.corridors,
                });
                if slot == forest.items.len() {
                    let cell = to_box(child.cell_bounds);
                    root_cells = Some(root_cells.map_or(cell, |bounds| bounds.union(cell)));
                }
            }
            let domain = if slot == forest.items.len() {
                root_cells.map_or_else(
                    || to_box(measured.content),
                    |bounds| bounds.inflate(ROOT_OUTER_MARGIN, ROOT_OUTER_MARGIN),
                )
            } else {
                let owner = geometry.item(slot).ok_or_else(missing_container)?;
                to_box(owner.bounds).inflate(-CLEARANCE, -CLEARANCE)
            };
            containers.push(Some(GridRoutingContainer {
                id,
                item_slots,
                children,
                horizontal_corridors: &measured.horizontal_corridors,
                vertical_corridors: &measured.vertical_corridors,
                domain,
                title,
            }));
        }
        Ok(Self {
            local_slots,
            containers,
        })
    }

    pub(super) fn container(&self, slot: usize) -> Result<&GridRoutingContainer<'a>> {
        self.containers
            .get(slot)
            .and_then(Option::as_ref)
            .ok_or_else(missing_container)
    }

    pub(super) fn local_slot(&self, owner: usize) -> Result<usize> {
        self.local_slots
            .get(owner)
            .copied()
            .flatten()
            .ok_or_else(missing_container)
    }
}

pub(super) struct GridRoutingBases<'a> {
    pub retained_bytes: usize,
    pub bases: Vec<Option<GridRoutingBase<'a>>>,
}

pub(super) enum GridRoutingBase<'a> {
    Sparse(GridRoutingTopology<'a>),
    Compatibility(GridTopologyLimit),
}

impl<'a> GridRoutingBases<'a> {
    pub(super) fn build(
        forest: &GridForest<'a>,
        geometry: &GridForestGeometry,
        containers: &'a GridRoutingContainers<'_>,
        requested: &[usize],
        work: &OperationWorkMeter,
    ) -> Result<Self> {
        let count = forest.children.len();
        work.charge(count)?;
        let mut bases = Vec::new();
        reserve(&mut bases, count)?;
        bases.resize_with(count, || None);
        let mut needed = Vec::new();
        reserve(&mut needed, requested.len())?;
        let mut max_id_bytes = 0;
        for &slot in requested {
            work.charge(1)?;
            let container = containers.container(slot)?;
            max_id_bytes = max_id_bytes.max(container.id.len());
            needed.push((slot, container));
        }
        sort_charge(needed.len(), max_id_bytes, work)?;
        needed.sort_by(|(_, a), (_, b)| code_units(a.id, b.id));
        needed.dedup_by_key(|(slot, _)| *slot);

        let mut retained_bytes: usize = 0;
        for (slot, container) in needed {
            work.charge(1)?;
            let mut obstacles = Vec::new();
            work.charge(container.item_slots.len())?;
            reserve(&mut obstacles, container.item_slots.len())?;
            let mut portals = Vec::new();
            for (&child_slot, child) in container.item_slots.iter().zip(&container.children) {
                work.charge(1)?;
                let item = &forest.items[child_slot];
                obstacles.push(GridTopologyObstacle {
                    id: item.item.id,
                    bounds: child.bounds,
                });
                if matches!(item.kind, GridItemKind::Group(_)) {
                    append_portals(
                        &mut portals,
                        item.item.id,
                        child.bounds,
                        title_bounds(geometry, child_slot),
                        false,
                        work,
                    )?;
                }
            }
            if let Some(owner) = geometry.item(slot) {
                append_portals(
                    &mut portals,
                    container.id,
                    to_box(owner.bounds),
                    title_bounds(geometry, slot),
                    true,
                    work,
                )?;
            }
            let title = container
                .title
                .as_ref()
                .map(|(id, bounds)| GridTopologyObstacle {
                    id,
                    bounds: *bounds,
                });
            let input = GridTopologyInput {
                container_id: container.id,
                bounds: container.domain,
                obstacles: &obstacles,
                title_exclusions: title.as_slice(),
                portal_ranges: &portals,
            };
            bases[slot] = Some(
                match GridRoutingTopology::build(input, GridTopologyCaps::default(), work) {
                    Ok(topology) => {
                        let actual = retained_bytes
                            .checked_add(topology.estimated_bytes())
                            .ok_or_else(|| work.arithmetic_overflow())?;
                        if actual > GRID_ROUTING_ESTIMATED_BYTES {
                            GridRoutingBase::Compatibility(GridTopologyLimit::EstimatedMemory {
                                actual,
                                max: GRID_ROUTING_ESTIMATED_BYTES,
                            })
                        } else {
                            retained_bytes = actual;
                            GridRoutingBase::Sparse(topology)
                        }
                    }
                    Err(GridTopologyError::Limit(limit)) => GridRoutingBase::Compatibility(limit),
                    Err(GridTopologyError::Layout(error)) => return Err(error),
                },
            );
        }
        Ok(Self {
            retained_bytes,
            bases,
        })
    }
}

fn title_bounds(geometry: &GridForestGeometry, slot: usize) -> Option<Box2> {
    geometry
        .containers
        .get(slot)?
        .as_ref()?
        .title_bounds
        .map(to_box)
}

fn append_portals<'a>(
    portals: &mut Vec<GridPortalRange<'a>>,
    id: &'a str,
    bounds: Box2,
    title: Option<Box2>,
    interior: bool,
    work: &OperationWorkMeter,
) -> Result<()> {
    let ranges = grid_portal_ranges(id, bounds, title, work)?;
    reserve(portals, ranges.len())?;
    let shift = if interior { CLEARANCE } else { -CLEARANCE };
    for mut range in ranges {
        range.coordinate = Some(match range.side {
            Side::Left => bounds.x0 + shift,
            Side::Right => bounds.x1 - shift,
            Side::Top => bounds.y0 + shift,
            Side::Bottom => bounds.y1 - shift,
        });
        portals.push(range);
    }
    Ok(())
}

fn to_box(rect: Rect) -> Box2 {
    Box2::new(rect.x, rect.y, rect.x + rect.width, rect.y + rect.height)
}

fn missing_container() -> Error {
    Error::InvalidModel {
        message: "GRID_ROUTE_NOT_FOUND: Missing measured Grid routing container".to_owned(),
    }
}

/// Per-container planning occupancy mirrors the source router's trimmed nonterminal spans.
#[derive(Clone, Copy)]
pub(super) struct GridOccupiedSpan {
    axis: Axis,
    start: Point,
    end: Point,
}

impl GridOccupiedSpan {
    fn fixed(self, point: Point) -> f64 {
        match self.axis {
            Axis::Horizontal => point.y,
            Axis::Vertical => point.x,
        }
    }

    fn along(self, point: Point) -> f64 {
        match self.axis {
            Axis::Horizontal => point.x,
            Axis::Vertical => point.y,
        }
    }

    fn trim(&mut self, start: bool) {
        let (end, other) = if start {
            (self.start, self.end)
        } else {
            (self.end, self.start)
        };
        let delta = self.along(other) - self.along(end);
        if delta != 0.0 {
            let end = if start {
                &mut self.start
            } else {
                &mut self.end
            };
            let axis = match self.axis {
                Axis::Horizontal => &mut end.x,
                Axis::Vertical => &mut end.y,
            };
            *axis += delta.signum() * 20.0;
        }
    }

    pub(super) fn conflicts(self, other: Self) -> bool {
        if self.axis != other.axis
            || (self.fixed(self.start) - other.fixed(other.start)).abs() >= 24.0
        {
            return false;
        }
        let span = |segment: Self| {
            let a = segment.along(segment.start);
            let b = segment.along(segment.end);
            (a.min(b), a.max(b))
        };
        let (a0, a1) = span(self);
        let (b0, b1) = span(other);
        a1.min(b1) > a0.max(b0)
    }
}

pub(super) fn grid_occupied_spans(
    points: &[Point],
    work: &OperationWorkMeter,
) -> Result<Vec<GridOccupiedSpan>> {
    let mut spans: Vec<GridOccupiedSpan> = Vec::new();
    work.charge(points.len())?;
    reserve(&mut spans, points.len().saturating_sub(1))?;
    for pair in points.windows(2) {
        work.charge(1)?;
        let (start, end) = (pair[0], pair[1]);
        if ![start.x, start.y, end.x, end.y]
            .into_iter()
            .all(f64::is_finite)
        {
            return Err(Error::InvalidModel {
                message: "Non-finite Grid occupancy polyline".to_owned(),
            });
        }
        let (dx, dy) = ((start.x - end.x).abs(), (start.y - end.y).abs());
        if dx <= PIXEL_EPSILON && dy <= PIXEL_EPSILON {
            continue;
        }
        let axis = if dy <= PIXEL_EPSILON {
            Axis::Horizontal
        } else {
            Axis::Vertical
        };
        if let Some(previous) = spans.last_mut()
            && previous.axis == axis
            && (previous.fixed(previous.end) - previous.fixed(start)).abs() <= PIXEL_EPSILON
            && (previous.fixed(previous.start) - previous.fixed(start)).abs() <= PIXEL_EPSILON
        {
            previous.end = end;
        } else {
            spans.push(GridOccupiedSpan { axis, start, end });
        }
    }
    let Some(first) = spans.first_mut() else {
        return Ok(spans);
    };
    let length = (first.start.x - first.end.x).abs() + (first.start.y - first.end.y).abs();
    if length <= 40.0 && spans.len() == 1 {
        spans.clear();
        return Ok(spans);
    }
    spans[0].trim(true);
    if let Some(last) = spans.last_mut() {
        last.trim(false);
    }
    spans.retain(|span| span.start != span.end);
    Ok(spans)
}
