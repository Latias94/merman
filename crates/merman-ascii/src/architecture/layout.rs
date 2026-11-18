//! Positive difference constraints preserve authored centers, ownership, and nonoverlap.
//!
//! Only nodes have center variables. Frames have independent minimum/maximum variables, so
//! shared centers across different groups do not pretend that the frames themselves align.
//! Admission fixes every sibling separation. Measurement changes only positive edge weights;
//! the two admitted DAGs therefore remain feasible for every set of measured minimum sizes.

use super::reserve;
use super::validate::{Plan, filled};
use crate::Result;
use crate::graph::positioned::{PositionedBounds, PositionedSize};
use crate::resource::ResourceContext;
use merman_model::families::architecture::{ArchitectureDiagramModel, ArchitectureLayoutDirection};
use rustc_hash::FxHashMap;

const SCENE_MARGIN: usize = 4;
const FRAME_GUTTER: usize = 3;

#[derive(Clone, Copy)]
enum Axis {
    X,
    Y,
}

impl Axis {
    fn dimension(self, size: PositionedSize) -> usize {
        match self {
            Self::X => size.width,
            Self::Y => size.height,
        }
    }

    fn gap(self) -> usize {
        match self {
            Self::X => 9,
            Self::Y => 5,
        }
    }
}

#[derive(Clone, Copy)]
enum Spacing {
    FrameMinimum(usize),
    ContainStart { parent: usize, child: usize },
    ContainEnd { child: usize },
    Separate { before: usize, after: usize },
}

struct Constraint {
    to: usize,
    spacing: Spacing,
}

struct AxisPlan {
    axis: Axis,
    centers: Vec<usize>,
    outgoing: Vec<Vec<Constraint>>,
    order: Vec<usize>,
}

pub(super) struct LayoutPlan {
    node_count: usize,
    demand: Vec<[usize; 4]>,
    axes: [AxisPlan; 2],
}

impl Plan {
    pub(super) fn bounds(
        &self,
        minimums: &[PositionedSize],
        resources: &ResourceContext,
    ) -> Result<Vec<PositionedBounds>> {
        self.geometry.bounds(minimums, resources)
    }
}

impl LayoutPlan {
    pub(super) fn prepare(
        model: &ArchitectureDiagramModel,
        ids: &FxHashMap<&str, usize>,
        children: &[Vec<usize>],
        endpoints: &[(usize, usize)],
        resources: &ResourceContext,
    ) -> Result<Option<Self>> {
        let count = children.len() - 1;
        let mut demand = filled::<[usize; 4]>(count, resources)?;
        for (edge, &(from, to)) in model.edges.iter().zip(endpoints) {
            resources.charge_layout_work(1)?;
            for (index, side) in [(from, edge.lhs_dir), (to, edge.rhs_dir)] {
                let side = match side {
                    'L' => 0,
                    'R' => 1,
                    'T' => 2,
                    'B' => 3,
                    _ => return Ok(None),
                };
                demand[index][side] = resources.checked_grid_add(demand[index][side], 1)?;
            }
        }
        // The pinned renderer uses directions to infer placement, but explicitly declared
        // alignments override heuristic pairs (12.3.0 architectureRenderer.ts). Fixed port
        // sides remain hard in the routing seam even when a geometric preference is omitted.
        for (prefer_directions, align_ports) in [(true, true), (true, false), (false, false)] {
            if let Some(axes) = Self::try_axes(
                model,
                ids,
                children,
                endpoints,
                &demand,
                prefer_directions,
                align_ports,
                resources,
            )? {
                return Ok(Some(Self {
                    node_count: model.nodes.len(),
                    demand,
                    axes,
                }));
            }
        }
        Ok(None)
    }

    #[allow(clippy::too_many_arguments)]
    fn try_axes(
        model: &ArchitectureDiagramModel,
        ids: &FxHashMap<&str, usize>,
        children: &[Vec<usize>],
        endpoints: &[(usize, usize)],
        demand: &[[usize; 4]],
        prefer_directions: bool,
        align_ports: bool,
        resources: &ResourceContext,
    ) -> Result<Option<[AxisPlan; 2]>> {
        let mut horizontal = UnionFind::new(model.nodes.len(), resources)?;
        let mut vertical = UnionFind::new(model.nodes.len(), resources)?;
        let mut explicit = filled::<bool>(model.nodes.len(), resources)?;
        for hint in &model.layout_hints {
            let equality = match hint.direction {
                ArchitectureLayoutDirection::Row => &mut vertical,
                ArchitectureLayoutDirection::Column => &mut horizontal,
            };
            for pair in hint.members.windows(2) {
                resources.charge_layout_work(
                    resources.checked_work_add(pair[0].len(), pair[1].len())?,
                )?;
                equality.join(ids[pair[0].as_str()], ids[pair[1].as_str()], resources)?;
                explicit[ids[pair[0].as_str()]] = true;
                explicit[ids[pair[1].as_str()]] = true;
            }
        }
        if align_ports {
            let mut incidents = filled::<[usize; 4]>(model.nodes.len(), resources)?;
            for edge in &model.edges {
                resources.charge_layout_work(
                    resources.checked_work_add(edge.lhs_id.len(), edge.rhs_id.len())?,
                )?;
                for (node, side) in [
                    (ids[edge.lhs_id.as_str()], edge.lhs_dir),
                    (ids[edge.rhs_id.as_str()], edge.rhs_dir),
                ] {
                    let slot = port_index(side)?;
                    incidents[node][slot] = resources.checked_work_add(incidents[node][slot], 1)?;
                }
            }
            for edge in &model.edges {
                resources.charge_layout_work(
                    resources.checked_work_add(edge.lhs_id.len(), edge.rhs_id.len())?,
                )?;
                let (from, to) = (ids[edge.lhs_id.as_str()], ids[edge.rhs_id.as_str()]);
                if explicit[from]
                    || explicit[to]
                    || incidents[from][port_index(edge.lhs_dir)?] != 1
                    || incidents[to][port_index(edge.rhs_dir)?] != 1
                {
                    continue;
                }
                // Only exclusive opposing ports propose a shared center. A fan's private
                // legs must remain free to spread, and authored alignments always prevail.
                match (edge.lhs_dir, edge.rhs_dir) {
                    ('L', 'R') | ('R', 'L') => vertical.join(from, to, resources)?,
                    ('T', 'B') | ('B', 'T') => horizontal.join(from, to, resources)?,
                    _ => {}
                }
            }
        }
        let mut axes = [
            AxisBuilder::new(Axis::X, horizontal, model.groups.len(), resources)?,
            AxisBuilder::new(Axis::Y, vertical, model.groups.len(), resources)?,
        ];
        for axis in &mut axes {
            for (parent, members) in children.iter().enumerate().take(children.len() - 1) {
                resources.charge_layout_work(1)?;
                if parent < model.nodes.len() {
                    continue;
                }
                axis.add(
                    axis.plan.boundary(parent, false),
                    axis.plan.boundary(parent, true),
                    Spacing::FrameMinimum(parent),
                    resources,
                )?;
                for &child in members {
                    axis.add(
                        axis.plan.boundary(parent, false),
                        axis.plan.boundary(child, false),
                        Spacing::ContainStart { parent, child },
                        resources,
                    )?;
                    axis.add(
                        axis.plan.boundary(child, true),
                        axis.plan.boundary(parent, true),
                        Spacing::ContainEnd { child },
                        resources,
                    )?;
                }
            }
        }
        for hint in &model.layout_hints {
            let axis = match hint.direction {
                ArchitectureLayoutDirection::Row => &mut axes[0],
                ArchitectureLayoutDirection::Column => &mut axes[1],
            };
            for pair in hint.members.windows(2) {
                resources.charge_layout_work(
                    resources.checked_work_add(pair[0].len(), pair[1].len())?,
                )?;
                if !axis.separate(ids[pair[0].as_str()], ids[pair[1].as_str()], resources)? {
                    return Ok(None);
                }
            }
        }

        if prefer_directions {
            for (edge, &(from, to)) in model.edges.iter().zip(endpoints) {
                resources.charge_layout_work(1)?;
                if from == to || edge.lhs_dir == edge.rhs_dir {
                    continue;
                }
                // Perpendicular ports contribute one preference on each axis. Opposing
                // ports contribute just their shared axis; same-side ports impose no order.
                let x = match (edge.lhs_dir, edge.rhs_dir) {
                    ('L', _) | (_, 'R') => Some((to, from)),
                    ('R', _) | (_, 'L') => Some((from, to)),
                    _ => None,
                };
                let y = match (edge.lhs_dir, edge.rhs_dir) {
                    ('T', _) => Some((to, from)),
                    ('B', _) => Some((from, to)),
                    (_, 'T') => Some((to, from)),
                    (_, 'B') => Some((from, to)),
                    _ => None,
                };
                for (axis, preference) in axes.iter_mut().zip([x, y]) {
                    if let Some((before, after)) = preference {
                        // An explicit chain or a previous preference may already require
                        // the reverse. This never changes or drops the authored port sides.
                        axis.separate(before, after, resources)?;
                    }
                }
            }
        }

        let metrics = Metrics {
            node_count: model.nodes.len(),
            minimums: None,
            demand,
        };
        let positions = [
            axes[0].snapshot(&metrics, resources)?,
            axes[1].snapshot(&metrics, resources)?,
        ];
        // Freeze a separation for EVERY sibling pair, including pairs that happen not to
        // overlap at abstract sizes. Incidental separation would not survive measurement.
        for siblings in children {
            resources.charge_layout_work(1)?;
            for (offset, &a) in siblings.iter().enumerate() {
                for &b in &siblings[offset + 1..] {
                    resources.charge_layout_work(1)?;
                    let mut preferred = [(0, a, b), (1, a, b), (0, b, a), (1, b, a)];
                    for (axis_index, axis) in axes.iter().enumerate() {
                        let (a_min, a_max) =
                            axis.plan
                                .extent(a, &positions[axis_index], &metrics, resources)?;
                        let (b_min, b_max) =
                            axis.plan
                                .extent(b, &positions[axis_index], &metrics, resources)?;
                        if a_max < b_min || b_max < a_min {
                            let (before, after) = if a_max < b_min { (a, b) } else { (b, a) };
                            if let Some(index) = preferred
                                .iter()
                                .position(|&entry| entry == (axis_index, before, after))
                            {
                                preferred.swap(0, index);
                            }
                            break;
                        }
                    }
                    let mut separated = false;
                    for (axis, before, after) in preferred {
                        if axes[axis].separate(before, after, resources)? {
                            separated = true;
                            break;
                        }
                    }
                    if !separated {
                        return Ok(None);
                    }
                }
            }
        }
        let [horizontal, vertical] = axes;
        Ok(Some([
            horizontal.finish(resources)?,
            vertical.finish(resources)?,
        ]))
    }

    fn bounds(
        &self,
        minimums: &[PositionedSize],
        resources: &ResourceContext,
    ) -> Result<Vec<PositionedBounds>> {
        resources.checkpoint()?;
        if minimums.len() != self.demand.len() {
            return Err(super::invalid_projection());
        }
        let metrics = Metrics {
            node_count: self.node_count,
            minimums: Some(minimums),
            demand: &self.demand,
        };
        let positions = [
            self.axes[0].solve(&metrics, resources)?,
            self.axes[1].solve(&metrics, resources)?,
        ];
        let mut bounds = Vec::new();
        resources.charge_layout_work(minimums.len())?;
        reserve(&mut bounds, minimums.len())?;
        for index in 0..minimums.len() {
            resources.charge_layout_work(1)?;
            let (x, right) = self.axes[0].extent(index, &positions[0], &metrics, resources)?;
            let (y, bottom) = self.axes[1].extent(index, &positions[1], &metrics, resources)?;
            let width = resources.checked_grid_add(right - x, 1)?;
            let height = resources.checked_grid_add(bottom - y, 1)?;
            resources.grid_extent(
                resources.checked_grid_add(right, SCENE_MARGIN + 1)?,
                resources.checked_grid_add(bottom, SCENE_MARGIN + 1)?,
            )?;
            bounds.push(PositionedBounds {
                x,
                y,
                width,
                height,
            });
        }
        Ok(bounds)
    }
}

fn port_index(side: char) -> Result<usize> {
    match side {
        'L' => Ok(0),
        'R' => Ok(1),
        'T' => Ok(2),
        'B' => Ok(3),
        _ => Err(super::invalid_projection()),
    }
}

struct Metrics<'a> {
    node_count: usize,
    minimums: Option<&'a [PositionedSize]>,
    demand: &'a [[usize; 4]],
}

impl Metrics<'_> {
    fn minimum(&self, index: usize) -> PositionedSize {
        self.minimums.map_or_else(
            || {
                if index < self.node_count {
                    PositionedSize {
                        width: 3,
                        height: 3,
                    }
                } else {
                    PositionedSize {
                        width: 5,
                        height: 4,
                    }
                }
            },
            |sizes| sizes[index],
        )
    }

    fn dimension(&self, index: usize, axis: Axis, resources: &ResourceContext) -> Result<usize> {
        let demand = self.demand[index];
        let slots = match axis {
            Axis::X => demand[2].max(demand[3]),
            Axis::Y => demand[0].max(demand[1]),
        };
        let capacity = resources.checked_grid_add(resources.checked_grid_mul(slots, 2)?, 3)?;
        let dimension = axis.dimension(self.minimum(index)).max(capacity);
        Ok(if index < self.node_count {
            dimension | 1
        } else {
            dimension
        })
    }

    fn radius(&self, index: usize, axis: Axis, resources: &ResourceContext) -> Result<usize> {
        if index < self.node_count {
            Ok(self.dimension(index, axis, resources)? / 2)
        } else {
            Ok(0)
        }
    }

    fn weight(&self, spacing: Spacing, axis: Axis, resources: &ResourceContext) -> Result<usize> {
        match spacing {
            Spacing::FrameMinimum(index) => Ok(self.dimension(index, axis, resources)? - 1),
            Spacing::ContainStart { parent, child } => {
                let inset = match axis {
                    Axis::X => FRAME_GUTTER,
                    Axis::Y => self
                        .minimum(parent)
                        .height
                        .saturating_sub(2)
                        .max(FRAME_GUTTER),
                };
                resources.checked_grid_add(inset, self.radius(child, axis, resources)?)
            }
            Spacing::ContainEnd { child } => {
                resources.checked_grid_add(FRAME_GUTTER, self.radius(child, axis, resources)?)
            }
            Spacing::Separate { before, after } => {
                let fan = self.demand[before]
                    .iter()
                    .chain(&self.demand[after])
                    .copied()
                    .max()
                    .unwrap_or(0)
                    .saturating_sub(1);
                let gap =
                    resources.checked_grid_add(axis.gap(), resources.checked_grid_mul(fan, 2)?)?;
                resources.checked_grid_add(
                    resources.checked_grid_add(self.radius(before, axis, resources)?, gap)?,
                    self.radius(after, axis, resources)?,
                )
            }
        }
    }
}

impl AxisPlan {
    fn boundary(&self, item: usize, maximum: bool) -> usize {
        if item < self.centers.len() {
            self.centers[item]
        } else {
            self.centers.len() + 2 * (item - self.centers.len()) + usize::from(maximum)
        }
    }

    fn extent(
        &self,
        item: usize,
        positions: &[usize],
        metrics: &Metrics<'_>,
        resources: &ResourceContext,
    ) -> Result<(usize, usize)> {
        let radius = metrics.radius(item, self.axis, resources)?;
        let minimum = positions[self.boundary(item, false)] - radius;
        let maximum = resources.checked_grid_add(positions[self.boundary(item, true)], radius)?;
        Ok((minimum, maximum))
    }

    fn solve(&self, metrics: &Metrics<'_>, resources: &ResourceContext) -> Result<Vec<usize>> {
        let mut positions = filled::<usize>(self.outgoing.len(), resources)?;
        for (index, &center) in self.centers.iter().enumerate() {
            resources.charge_layout_work(1)?;
            positions[center] = positions[center].max(
                resources
                    .checked_grid_add(SCENE_MARGIN, metrics.radius(index, self.axis, resources)?)?,
            );
        }
        for index in self.centers.len()..metrics.demand.len() {
            resources.charge_layout_work(1)?;
            positions[self.boundary(index, false)] = SCENE_MARGIN;
        }
        for &from in &self.order {
            resources.charge_layout_work(1)?;
            for edge in &self.outgoing[from] {
                resources.charge_layout_work(1)?;
                let target = resources.checked_grid_add(
                    positions[from],
                    metrics.weight(edge.spacing, self.axis, resources)?,
                )?;
                positions[edge.to] = positions[edge.to].max(target);
            }
        }
        Ok(positions)
    }
}

struct AxisBuilder {
    plan: AxisPlan,
    seen: Vec<usize>,
    stack: Vec<usize>,
    epoch: usize,
}

impl AxisBuilder {
    fn new(
        axis: Axis,
        mut centers: UnionFind,
        groups: usize,
        resources: &ResourceContext,
    ) -> Result<Self> {
        for index in 0..centers.parents.len() {
            let root = centers.find(index, resources)?;
            centers.parents[index] = root;
        }
        let vertices = resources.checked_work_add(
            centers.parents.len(),
            resources.checked_work_mul(groups, 2)?,
        )?;
        let mut stack = Vec::new();
        reserve(&mut stack, vertices)?;
        Ok(Self {
            plan: AxisPlan {
                axis,
                centers: centers.parents,
                outgoing: filled::<Vec<Constraint>>(vertices, resources)?,
                order: Vec::new(),
            },
            seen: filled::<usize>(vertices, resources)?,
            stack,
            epoch: 0,
        })
    }

    fn add(
        &mut self,
        from: usize,
        to: usize,
        spacing: Spacing,
        resources: &ResourceContext,
    ) -> Result<()> {
        resources.charge_layout_work(1)?;
        reserve(&mut self.plan.outgoing[from], 1)?;
        self.plan.outgoing[from].push(Constraint { to, spacing });
        Ok(())
    }

    fn separate(
        &mut self,
        before: usize,
        after: usize,
        resources: &ResourceContext,
    ) -> Result<bool> {
        let from = self.plan.boundary(before, true);
        let to = self.plan.boundary(after, false);
        resources.charge_layout_work(1)?;
        if from == to {
            return Ok(false);
        }
        if self.epoch == usize::MAX {
            resources.charge_layout_work(self.seen.len())?;
            self.seen.fill(0);
            self.epoch = 0;
        }
        self.epoch += 1;
        self.stack.clear();
        self.stack.push(to);
        self.seen[to] = self.epoch;
        while let Some(index) = self.stack.pop() {
            resources.charge_layout_work(1)?;
            if index == from {
                return Ok(false);
            }
            for edge in &self.plan.outgoing[index] {
                resources.charge_layout_work(1)?;
                if self.seen[edge.to] != self.epoch {
                    self.seen[edge.to] = self.epoch;
                    self.stack.push(edge.to);
                }
            }
        }
        self.add(from, to, Spacing::Separate { before, after }, resources)?;
        Ok(true)
    }

    fn topological_order(&mut self, resources: &ResourceContext) -> Result<()> {
        let count = self.plan.outgoing.len();
        let mut degrees = filled::<usize>(count, resources)?;
        for edges in &self.plan.outgoing {
            resources.charge_layout_work(1)?;
            for edge in edges {
                resources.charge_layout_work(1)?;
                degrees[edge.to] = resources.checked_work_add(degrees[edge.to], 1)?;
            }
        }
        self.stack.clear();
        self.plan.order.clear();
        reserve(&mut self.plan.order, count)?;
        for (index, &degree) in degrees.iter().enumerate() {
            resources.charge_layout_work(1)?;
            if degree == 0 {
                self.stack.push(index);
            }
        }
        while let Some(index) = self.stack.pop() {
            resources.charge_layout_work(1)?;
            self.plan.order.push(index);
            for edge in &self.plan.outgoing[index] {
                resources.charge_layout_work(1)?;
                degrees[edge.to] -= 1;
                if degrees[edge.to] == 0 {
                    self.stack.push(edge.to);
                }
            }
        }
        if self.plan.order.len() != count {
            return Err(super::invalid_projection());
        }
        Ok(())
    }

    fn snapshot(
        &mut self,
        metrics: &Metrics<'_>,
        resources: &ResourceContext,
    ) -> Result<Vec<usize>> {
        self.topological_order(resources)?;
        self.plan.solve(metrics, resources)
    }

    fn finish(mut self, resources: &ResourceContext) -> Result<AxisPlan> {
        self.topological_order(resources)?;
        Ok(self.plan)
    }
}

struct UnionFind {
    parents: Vec<usize>,
    ranks: Vec<usize>,
}

impl UnionFind {
    fn new(count: usize, resources: &ResourceContext) -> Result<Self> {
        let mut parents = filled::<usize>(count, resources)?;
        for (index, parent) in parents.iter_mut().enumerate() {
            resources.charge_layout_work(1)?;
            *parent = index;
        }
        Ok(Self {
            parents,
            ranks: filled::<usize>(count, resources)?,
        })
    }

    fn find(&mut self, mut index: usize, resources: &ResourceContext) -> Result<usize> {
        while self.parents[index] != index {
            resources.charge_layout_work(1)?;
            self.parents[index] = self.parents[self.parents[index]];
            index = self.parents[index];
        }
        resources.charge_layout_work(1)?;
        Ok(index)
    }

    fn join(&mut self, a: usize, b: usize, resources: &ResourceContext) -> Result<()> {
        let (mut a, mut b) = (self.find(a, resources)?, self.find(b, resources)?);
        if a == b {
            return Ok(());
        }
        if self.ranks[a] < self.ranks[b] {
            std::mem::swap(&mut a, &mut b);
        }
        self.parents[b] = a;
        if self.ranks[a] == self.ranks[b] {
            self.ranks[a] = resources.checked_work_add(self.ranks[a], 1)?;
        }
        Ok(())
    }
}
