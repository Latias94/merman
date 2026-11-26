use super::*;
use crate::{GroupGeometry, Rect, TitlePosition, WidthPolicy};
use std::collections::HashMap;

const SMALL_BLOCK: i64 = 64;
const MAX_COORD: i64 = i32::MAX as i64;

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
struct Block {
    at: [i64; 2],
    size: [i64; 2],
}

impl Block {
    const CELL: Self = Self {
        at: [0; 2],
        size: [1; 2],
    };

    fn end(self, axis: usize) -> i64 {
        self.at[axis] + self.size[axis]
    }

    fn overlaps(self, other: Self) -> bool {
        (0..2).all(|axis| self.at[axis] < other.end(axis) && other.at[axis] < self.end(axis))
    }

    fn checked(self) -> LayoutResult<Self> {
        if (0..2).all(|axis| self.at[axis] >= -MAX_COORD && self.end(axis) <= MAX_COORD) {
            Ok(self)
        } else {
            Err(WorkError::ArithmeticOverflow.into())
        }
    }

    fn small(self) -> bool {
        self.size.into_iter().product::<i64>() <= SMALL_BLOCK
    }

    fn cells(self) -> impl Iterator<Item = [i64; 2]> {
        (self.at[1]..self.end(1)).flat_map(move |y| (self.at[0]..self.end(0)).map(move |x| [x, y]))
    }
}

#[derive(Default)]
struct Occupied {
    items: Vec<usize>,
    cells: HashMap<[i64; 2], usize>,
    large: Vec<usize>,
    strides: HashMap<(Block, usize, i64), i64>,
}

impl Occupied {
    fn put(&mut self, item: usize, block: Block, work: &mut Work<'_>) -> LayoutResult<()> {
        work.visit(1)?;
        push(&mut self.items, item)?;
        if block.small() {
            let area = (block.size[0] * block.size[1]) as usize;
            work.visit(area)?;
            self.cells
                .try_reserve(area)
                .map_err(|_| LayoutError::AllocationLimit)?;
            for cell in block.cells() {
                self.cells.insert(cell, item);
            }
        } else {
            push(&mut self.large, item)?;
        }
        Ok(())
    }

    fn hit(
        &self,
        block: Block,
        blocks: &[Block],
        work: &mut Work<'_>,
    ) -> LayoutResult<Option<Block>> {
        let scan = if block.small() {
            for cell in block.cells() {
                work.visit(1)?;
                if let Some(&item) = self.cells.get(&cell) {
                    return Ok(Some(blocks[item]));
                }
            }
            &self.large
        } else {
            &self.items
        };
        for &item in scan {
            work.visit(1)?;
            if block.overlaps(blocks[item]) {
                return Ok(Some(blocks[item]));
            }
        }
        Ok(None)
    }

    // Each collision jumps past one block in a fixed direction. Repeated fan-out starts can
    // resume the previous search because a component only adds occupancy while it grows.
    fn stride(
        &mut self,
        start: Block,
        axis: usize,
        sign: i64,
        blocks: &[Block],
        work: &mut Work<'_>,
    ) -> LayoutResult<i64> {
        let key = (start, axis, sign);
        let mut distance = self.strides.get(&key).copied().unwrap_or(0);
        loop {
            work.visit(1)?;
            let mut block = start;
            block.at[axis] += distance * sign;
            let Some(hit) = self.hit(block.checked()?, blocks, work)? else {
                self.strides
                    .try_reserve(1)
                    .map_err(|_| LayoutError::AllocationLimit)?;
                self.strides.insert(key, distance);
                return Ok(distance);
            };
            distance = if sign > 0 {
                hit.end(axis) - start.at[axis]
            } else {
                start.end(axis) - hit.at[axis]
            };
        }
    }
}

fn anchor(
    mut end: usize,
    item: usize,
    axis: usize,
    blocks: &[Block],
    hierarchy: &Hierarchy,
    work: &mut Work<'_>,
) -> LayoutResult<i64> {
    let mut at = (blocks[end].size[axis] - 1) / 2;
    while end != item {
        work.visit(1)?;
        at += blocks[end].at[axis];
        end = hierarchy.node_count + hierarchy.parent[end];
    }
    Ok(at)
}

struct Placement<'a> {
    hierarchy: &'a Hierarchy,
    blocks: Vec<Block>,
    placed: Vec<bool>,
    links: Vec<Vec<usize>>,
    occupied: Occupied,
}

impl Placement<'_> {
    fn follow(&mut self, from: usize, link: Link, work: &mut Work<'_>) -> LayoutResult<()> {
        work.visit(1)?;
        let a = usize::from(link.items[0] != from);
        let b = 1 - a;
        let to = link.items[b];
        if self.placed[to] {
            return Ok(());
        }
        let (sa, sb) = (link.sides[a], link.sides[b]);
        let (source_axis, source_sign) = normal(sa);
        let (target_axis, target_sign) = normal(sb);
        let (axis, sign) = if sa == sb {
            (1 - source_axis, 1)
        } else {
            (source_axis, source_sign)
        };
        let cross = 1 - axis;
        let here = self.blocks[from];
        let mut there = self.blocks[to];
        there.at[axis] = if sign > 0 {
            here.end(axis)
        } else {
            here.at[axis] - there.size[axis]
        };
        there.at[cross] = here.at[cross]
            + anchor(
                link.ends[a],
                from,
                cross,
                &self.blocks,
                self.hierarchy,
                work,
            )?
            - anchor(link.ends[b], to, cross, &self.blocks, self.hierarchy, work)?;
        if source_axis != target_axis {
            there.at[cross] -= target_sign;
        }
        if source_axis == target_axis && sa != sb && !link.declared {
            let positive = self.occupied.stride(there, cross, 1, &self.blocks, work)?;
            let offset = if positive == 0 {
                0
            } else {
                let negative = self.occupied.stride(there, cross, -1, &self.blocks, work)?;
                if positive <= negative {
                    positive
                } else {
                    -negative
                }
            };
            there.at[cross] += offset;
        } else {
            there.at[axis] += sign
                * self
                    .occupied
                    .stride(there, axis, sign, &self.blocks, work)?;
        }
        self.put(to, there, work)
    }

    fn put(&mut self, item: usize, block: Block, work: &mut Work<'_>) -> LayoutResult<()> {
        self.blocks[item] = block.checked()?;
        self.placed[item] = true;
        self.occupied.put(item, block, work)
    }

    fn scope(&mut self, id: usize, limit: Option<i64>, work: &mut Work<'_>) -> LayoutResult<()> {
        let scope = &self.hierarchy.scopes[id];
        let mut parts = Vec::new();
        let mut order = reserved(scope.items.len())?;
        for (index, link) in scope.links.iter().enumerate() {
            work.visit(1)?;
            for item in link.items {
                push(&mut self.links[item], index)?;
            }
        }
        for &seed in &scope.items {
            work.visit(1)?;
            if self.placed[seed] {
                continue;
            }
            let first = order.len();
            if self.links[seed].is_empty() {
                order.push(seed);
                push(&mut parts, (first, self.blocks[seed]))?;
                continue;
            }
            self.occupied = Occupied::default();
            self.put(seed, self.blocks[seed], work)?;
            let mut hinted = 0;
            let mut ordinary = 0;
            let mut next_link = 0;
            while ordinary < self.occupied.items.len() {
                if hinted < self.occupied.items.len() {
                    let item = self.occupied.items[hinted];
                    work.visit(1)?;
                    for index in 0..self.links[item].len() {
                        let link = scope.links[self.links[item][index]];
                        if !link.declared {
                            break;
                        }
                        self.follow(item, link, work)?;
                    }
                    hinted += 1;
                    continue;
                }
                let item = self.occupied.items[ordinary];
                work.visit(1)?;
                if let Some(&link) = self.links[item].get(next_link) {
                    let link = scope.links[link];
                    next_link += 1;
                    if !link.declared {
                        self.follow(item, link, work)?;
                    }
                } else {
                    ordinary += 1;
                    next_link = 0;
                }
            }
            let mut low = [0; 2];
            let mut high = [0; 2];
            for &item in &self.occupied.items {
                work.visit(1)?;
                let block = self.blocks[item];
                for axis in 0..2 {
                    low[axis] = low[axis].min(block.at[axis]);
                    high[axis] = high[axis].max(block.end(axis));
                }
                order.push(item);
            }
            let extent = Block {
                at: low,
                size: [high[0] - low[0], high[1] - low[1]],
            };
            push(&mut parts, (first, extent))?;
        }
        let mut area = 0.0;
        let mut widest = 1;
        for &(_, part) in &parts {
            work.visit(1)?;
            area += part.size[0] as f64 * part.size[1] as f64;
            widest = widest.max(part.size[0]);
        }
        let target = (area * 1.5).sqrt().ceil() as i64;
        let limit = limit
            .map_or(target, |requested| target.min(requested))
            .max(widest);
        let mut offset = [0, 0];
        let mut shelf = 0;
        let mut width = 0;
        for (index, &(first, part)) in parts.iter().enumerate() {
            work.visit(1)?;
            if offset[0] > 0 && offset[0] + part.size[0] > limit {
                offset[0] = 0;
                offset[1] += shelf;
                shelf = 0;
            }
            let end = parts
                .get(index + 1)
                .map_or(order.len(), |&(first, _)| first);
            for &item in &order[first..end] {
                work.visit(1)?;
                let mut block = self.blocks[item];
                for (axis, at) in block.at.iter_mut().enumerate() {
                    *at += offset[axis] - part.at[axis];
                }
                self.blocks[item] = block.checked()?;
            }
            offset[0] += part.size[0];
            width = width.max(offset[0]);
            shelf = shelf.max(part.size[1]);
        }
        if id != self.hierarchy.root() {
            self.blocks[self.hierarchy.node_count + id] = Block {
                at: [0; 2],
                size: [width.max(1), (offset[1] + shelf).max(1)],
            }
            .checked()?;
        }
        self.occupied = Occupied::default();
        Ok(())
    }
}

#[derive(Clone, Copy, Default)]
struct Track {
    size: f64,
    before: f64,
    after: f64,
    start: f64,
}

#[derive(Default)]
struct Tracks {
    ranges: Vec<[f64; 2]>,
    gutters: Vec<f64>,
}

fn size_on(size: Size, axis: usize) -> f64 {
    [size.width, size.height][axis]
}

fn gap_on(metrics: Metrics, options: &Options, axis: usize) -> f64 {
    [options.spacing.rank, options.spacing.node][axis].max(metrics.margin() + metrics.separation)
}

fn tracks(
    axis: usize,
    graph: &Graph,
    hierarchy: &Hierarchy,
    blocks: &[Block],
    metrics: Metrics,
    options: &Options,
    work: &mut Work<'_>,
) -> LayoutResult<Tracks> {
    let count = blocks.len();
    let endpoints = count.checked_mul(2).ok_or(WorkError::ArithmeticOverflow)?;
    work.charge(endpoints)?;
    let mut coordinates = reserved(endpoints)?;
    for &block in blocks {
        work.visit(1)?;
        coordinates.extend([block.at[axis], block.end(axis) - 1]);
    }
    work.charge_sort(coordinates.len())?;
    coordinates.sort_unstable();
    work.charge(coordinates.len())?;
    coordinates.dedup();
    let mut tracks = filled(coordinates.len(), Track::default())?;
    let mut covered = reserved(count)?;
    for &block in blocks {
        work.visit(2 * (coordinates.len().ilog2() as usize + 1))?;
        covered.push([
            coordinates
                .binary_search(&block.at[axis])
                .expect("projected first grid track"),
            coordinates
                .binary_search(&(block.end(axis) - 1))
                .expect("projected last grid track"),
        ]);
    }
    for ([index, _], node) in covered.iter().copied().zip(&graph.nodes) {
        work.visit(1)?;
        tracks[index].size = tracks[index].size.max(size_on(node.size, axis));
    }
    let gap = gap_on(metrics, options, axis);
    let mut base = reserved(tracks.len() + 1)?;
    base.push(0.0);
    for track in &tracks {
        work.visit(1)?;
        base.push(base.last().copied().unwrap_or_default() + track.size + gap);
    }
    // Padding on coincident parent and child faces accumulates. The track uses the largest
    // request at that coordinate, so neighboring group rectangles stay disjoint.
    let mut insets = filled(graph.groups.len(), [0.0_f64; 2])?;
    for &id in hierarchy.order[1..].iter().rev() {
        work.visit(1)?;
        let group = &graph.groups[id];
        let scope = &hierarchy.scopes[id];
        let item = hierarchy.node_count + id;
        let [first, last] = covered[item];
        let title = group.title.unwrap_or_default();
        let top = is_top(group.title_position);
        let mut pad = match axis {
            0 => [group.padding.left, group.padding.right],
            _ => [group.padding.top, group.padding.bottom],
        }
        .map(|p| p.max(metrics.margin()));
        if group.title.is_some() && axis == 1 {
            let side = usize::from(!top);
            pad[side] = pad[side].max(title.height + metrics.margin() * 2.0);
        }
        if axis == 1 {
            pad[1] += scope.caption.height;
        }
        let mut current = [pad[0] + insets[id][0], pad[1] + insets[id][1]];
        let header = if axis == 0 && group.title.is_some() {
            title.width + metrics.margin() * 4.0
        } else {
            0.0
        };
        let minimum = size_on(group.min_size, axis).max(header).max(if axis == 0 {
            scope.caption.width + metrics.margin() * 2.0
        } else {
            0.0
        });
        let content = base[last + 1] - base[first] - gap;
        current[1] += (minimum - content - current[0] - current[1]).max(0.0);
        if !current.into_iter().all(f64::is_finite) {
            return Err(PRECISION);
        }
        insets[id] = current;
        tracks[first].before = tracks[first].before.max(current[0]);
        tracks[last].after = tracks[last].after.max(current[1]);
        let parent = hierarchy.parent[item];
        if parent != hierarchy.root() {
            let [parent_first, parent_last] = covered[hierarchy.node_count + parent];
            if first == parent_first {
                insets[parent][0] = insets[parent][0].max(current[0]);
            }
            if last == parent_last {
                insets[parent][1] = insets[parent][1].max(current[1]);
            }
        }
    }
    let mut cursor = metrics.margin();
    let mut gutters = reserved(tracks.len().saturating_add(1))?;
    gutters.push(cursor / 2.0);
    for track in &mut tracks {
        work.visit(1)?;
        track.start = cursor + track.before;
        let end = track.start + track.size + track.after;
        gutters.push(end + gap / 2.0);
        cursor = end + gap;
        if !cursor.is_finite() {
            return Err(PRECISION);
        }
    }
    if let Some(last) = gutters.last_mut() {
        *last = cursor - gap + metrics.margin() / 2.0;
    }
    let mut ranges = reserved(count)?;
    for (item, [first, last]) in covered.into_iter().enumerate() {
        work.visit(1)?;
        let head = tracks[first];
        let tail = tracks[last];
        let range = if let Some(node) = graph.nodes.get(item) {
            let center = head.start + head.size / 2.0;
            let half = size_on(node.size, axis) / 2.0;
            [center - half, center + half]
        } else {
            let [before, after] = insets[item - hierarchy.node_count];
            [head.start - before, tail.start + tail.size + after]
        };
        ranges.push(range);
    }
    Ok(Tracks { ranges, gutters })
}

pub(super) fn is_top(title: TitlePosition) -> bool {
    matches!(
        title,
        TitlePosition::TopLeft | TitlePosition::TopCenter | TitlePosition::TopRight
    )
}

pub(super) fn place(
    graph: &Graph,
    hierarchy: &Hierarchy,
    metrics: Metrics,
    options: &Options,
    work: &mut Work<'_>,
) -> LayoutResult<(LayoutOutput, [Vec<f64>; 2])> {
    let count = hierarchy.parent.len();
    let mut drawing = LayoutOutput {
        direction: graph.direction,
        ..Default::default()
    };
    if count == 0 {
        return Ok((drawing, Default::default()));
    }
    work.charge(count.checked_mul(3).ok_or(WorkError::ArithmeticOverflow)?)?;
    let mut placement = Placement {
        hierarchy,
        blocks: filled(count, Block::CELL)?,
        placed: filled(count, false)?,
        links: filled(count, Vec::new())?,
        occupied: Occupied::default(),
    };
    let limit = if let WidthPolicy::FitWidth { max_width, .. } = options.width_policy {
        let mut max_node = 0.0_f64;
        for node in &graph.nodes {
            work.visit(1)?;
            max_node = max_node.max(node.size.width);
        }
        let gap = gap_on(metrics, options, 0);
        Some(((max_width + gap) / (max_node + gap)).floor() as i64)
    } else {
        None
    };
    for &scope in hierarchy.order.iter().rev() {
        work.visit(1)?;
        let limit = if scope == hierarchy.root() {
            limit
        } else {
            None
        };
        placement.scope(scope, limit, work)?;
    }
    let mut blocks = placement.blocks;
    if options.capture_node_ranks {
        drawing.node_ranks = reserved(graph.nodes.len())?;
        let axis = usize::from(graph.direction.axis() == Axis::Vertical);
        for block in &blocks[..graph.nodes.len()] {
            work.visit(1)?;
            drawing.node_ranks.push(block.at[axis] as u32);
        }
    }
    for &id in &hierarchy.order {
        let origin = if id == hierarchy.root() {
            [0; 2]
        } else {
            blocks[hierarchy.node_count + id].at
        };
        for &item in &hierarchy.scopes[id].items {
            work.visit(1)?;
            for (at, origin) in blocks[item].at.iter_mut().zip(origin) {
                *at += origin;
            }
            blocks[item].checked()?;
        }
    }
    let xs = tracks(0, graph, hierarchy, &blocks, metrics, options, work)?;
    let ys = tracks(1, graph, hierarchy, &blocks, metrics, options, work)?;
    drawing.nodes = reserved(graph.nodes.len())?;
    drawing.groups = reserved(graph.groups.len())?;
    for (item, ([x, right], [y, bottom])) in xs.ranges.into_iter().zip(ys.ranges).enumerate() {
        work.visit(1)?;
        let rect = Rect {
            x,
            y,
            width: right - x,
            height: bottom - y,
        };
        if ![x, y, right, bottom, rect.width, rect.height]
            .into_iter()
            .all(f64::is_finite)
        {
            return Err(PRECISION);
        }
        drawing.bounds.width = drawing.bounds.width.max(right);
        drawing.bounds.height = drawing.bounds.height.max(bottom);
        if item < hierarchy.node_count {
            drawing.nodes.push(rect);
            continue;
        }
        let id = item - hierarchy.node_count;
        let group = &graph.groups[id];
        let parent = group.parent;
        drawing.groups.push(GroupGeometry {
            parent,
            rect,
            title_center: group.title.map(|title| {
                let margin = metrics.margin();
                let x = match group.title_position {
                    TitlePosition::TopLeft | TitlePosition::BottomLeft => {
                        x + margin + title.width / 2.0
                    }
                    TitlePosition::TopRight | TitlePosition::BottomRight => {
                        right - margin - title.width / 2.0
                    }
                    _ => rect.center().x,
                };
                let y = if is_top(group.title_position) {
                    y + margin + title.height / 2.0
                } else {
                    bottom - margin - title.height / 2.0
                };
                Point { x, y }
            }),
            title_struck: false,
            effective_direction: group.direction.unwrap_or(graph.direction),
        });
    }
    drawing.bounds.width += metrics.margin();
    drawing.bounds.height += metrics.margin() + hierarchy.scopes[hierarchy.root()].caption.height;
    if !validate::measured(drawing.bounds) {
        return Err(PRECISION);
    }
    Ok((drawing, [xs.gutters, ys.gutters]))
}
