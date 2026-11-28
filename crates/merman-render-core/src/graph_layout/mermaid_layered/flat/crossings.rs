//! Adjacent-rank crossings, grouped barycenters, and stable tie resolution.

use super::order::{Along, FlatOrder, Groups};
use super::work::compare_number;
use super::{CompoundError, Result, Work, add, inconsistent};

const MAX_SWEEPS: usize = 24;
const SMALL_GRAPH: usize = 600;
const TRANSPOSE_PASSES: usize = 8;

pub(super) fn reduce<E>(graph: &mut FlatOrder, work: &mut Work<'_, E>) -> Result<(), E> {
    let max_rank = graph.rows() - 1;
    if graph.segments.is_empty() || max_rank == 0 {
        return Ok(());
    }
    let mut widest = 0;
    work.price(graph.rows())?;
    for (rank, ends) in graph.layer_start.windows(2).enumerate() {
        work.poll(rank)?;
        widest = widest.max(ends[1] - ends[0]);
    }
    if widest < 2 {
        return Ok(());
    }
    let mut counter = CrossingCounter::new(graph, widest, work)?;
    let declared = counter.count(graph, work)?;
    graph.declared_crossings = declared;
    let mut groups = graph
        .groups
        .as_ref()
        .map(|g| GroupOrder::new(g, work))
        .transpose()?;
    let mut bary = work.filled(graph.order.len(), 0.0)?;
    if let Some(groups) = &mut groups {
        work.price(graph.rows())?;
        for r in 0..graph.rows() {
            work.poll(r)?;
            let row = &graph.order[graph.rank_range(r)];
            work.price(row.len())?;
            for (i, &node) in row.iter().enumerate() {
                work.poll(i)?;
                bary[node] = i as f64;
            }
            groups.arrange(graph, r, &bary, work)?;
        }
    }
    let mut best = if groups.is_some() {
        counter.count(graph, work)?
    } else {
        declared
    };
    if best == 0.0 {
        return Ok(());
    }

    // An improvement must beat both the grouped and ungrouped starting orders.
    let threshold = best.min(declared);
    let initial = work.copy(&graph.order)?;
    let mut found_order = work.copy(&initial)?;
    let mut chosen = work.filled(initial.len(), 0usize)?;
    let mut sweep = Sweep {
        bary,
        moving: work.reserve(widest)?,
        spare: Vec::new(),
        groups,
    };
    let small = graph.order.len() <= SMALL_GRAPH;
    let mut chosen_out = usize::MAX;

    for attempt in 0..if small { 2 } else { 1 } {
        if attempt > 0 {
            work.copy_into(&mut graph.order, &initial)?;
            graph.set_positions(work)?;
            work.copy_into(&mut found_order, &initial)?;
        }
        let mut found = threshold;
        let mut stale = 0;
        for i in 0..MAX_SWEEPS {
            if stale == 4 {
                break;
            }
            sweep.run(graph, (i + attempt) % 2 == 0, i % 4 >= 2, work)?;
            let current = counter.count(graph, work)?;
            if current < found {
                found = current;
                work.copy_into(&mut found_order, &graph.order)?;
                stale = 0;
                if found == 0.0 {
                    break;
                }
            }
            stale += 1;
        }
        work.copy_into(&mut graph.order, &found_order)?;
        if small && found > 0.0 {
            graph.set_positions(work)?;
            transpose(graph, work)?;
            let current = counter.count(graph, work)?;
            if current < found {
                found = current;
                work.copy_into(&mut found_order, &graph.order)?;
            }
        }
        let out = if small { out_of_order(graph, work)? } else { 0 };
        if attempt == 0 || found < best || (found == best && out < chosen_out) {
            best = found;
            chosen_out = out;
            std::mem::swap(&mut chosen, &mut found_order);
        }
    }

    work.copy_into(&mut graph.order, &chosen)?;
    graph.set_positions(work)?;
    if graph.groups.is_some() {
        best = counter.count(graph, work)?;
    }
    if small && chosen_out != 0 && !graph.fixed[0] && !graph.fixed[max_rank] {
        let mut pinned = false;
        work.price(graph.real)?;
        for (i, v) in graph.vertices[..graph.real].iter().enumerate() {
            work.poll(i)?;
            pinned |= v.pin != 0;
        }
        if !pinned {
            mirror(graph, work)?;
            if counter.count(graph, work)? > best || out_of_order(graph, work)? >= chosen_out {
                work.copy_into(&mut graph.order, &chosen)?;
                graph.set_positions(work)?;
            }
        }
    }
    graph.crossings = best;
    Ok(())
}

struct CrossingCounter {
    targets: Vec<usize>,
    tree: Vec<u32>,
    spare: Vec<usize>,
    offsets: Option<OffsetPlaces>,
}

struct OffsetPlaces {
    place: Vec<usize>,
    relative: Vec<usize>,
    counts: Vec<usize>,
    leaving: Vec<usize>,
    mixed: Vec<bool>,
}

impl OffsetPlaces {
    fn new<E>(graph: &FlatOrder, work: &mut Work<'_, E>) -> Result<Self, E> {
        let n = graph.vertices.len();
        let m = graph.segments.len();
        let mut offsets = Self {
            place: work.filled(m, 0)?,
            relative: work.filled(m, 0)?,
            counts: work.filled(n, 1)?,
            leaving: work.copy(&graph.outgoing.slots)?,
            mixed: work.filled(n, false)?,
        };
        let mut arriving = work.copy(&graph.incoming.slots)?;
        let mut spare = Vec::new();
        work.price(n)?;
        for v in 0..n {
            work.poll(v)?;
            let list = &mut arriving[graph.incoming.range(v)];
            let dx = |e: usize| graph.ends[e].offset[Along::Up as usize];
            if sorted_by_offset(list, dx, &mut spare, work)? {
                work.price(list.len() - 1)?;
                for (i, neighbors) in list.windows(2).enumerate() {
                    work.poll(i)?;
                    offsets.counts[v] += usize::from(dx(neighbors[0]) != dx(neighbors[1]));
                    offsets.relative[neighbors[1]] = offsets.counts[v] - 1;
                }
            }
            offsets.mixed[v] = sorted_by_offset(
                &mut offsets.leaving[graph.outgoing.range(v)],
                |e| graph.ends[e].offset[Along::Down as usize],
                &mut spare,
                work,
            )?;
        }
        Ok(offsets)
    }

    fn rank_places<E>(
        &mut self,
        graph: &FlatOrder,
        rank: usize,
        work: &mut Work<'_, E>,
    ) -> Result<usize, E> {
        let mut next = 0;
        let row = &graph.order[graph.rank_range(rank)];
        work.price(row.len())?;
        for (i, &node) in row.iter().enumerate() {
            work.poll(i)?;
            let incoming = graph.incoming.at(node);
            work.price(incoming.len())?;
            for (j, &e) in incoming.iter().enumerate() {
                work.poll(j)?;
                self.place[e] = next + self.relative[e];
            }
            // The tree's prevalidated widest + segment bound also covers this rank's places.
            next += self.counts[node];
        }
        Ok(next)
    }
}

fn sorted_by_offset<E>(
    list: &mut [usize],
    dx: impl Fn(usize) -> f64,
    spare: &mut Vec<usize>,
    work: &mut Work<'_, E>,
) -> Result<bool, E> {
    let Some((&first, rest)) = list.split_first() else {
        return Ok(false);
    };
    work.price(rest.len())?;
    for (i, &e) in rest.iter().enumerate() {
        work.poll(i)?;
        if dx(e) != dx(first) {
            work.sort_with(list, spare, |a, b, _| Ok(compare_number(dx(a), dx(b))))?;
            return Ok(true);
        }
    }
    Ok(false)
}

impl CrossingCounter {
    fn new<E>(graph: &FlatOrder, widest: usize, work: &mut Work<'_, E>) -> Result<Self, E> {
        let m = graph.segments.len();
        let positions = if graph.offsets {
            add(widest, m)?
        } else {
            widest
        };
        let leaves = positions
            .checked_next_power_of_two()
            .filter(|&n| n <= 1 << 30)
            .ok_or(CompoundError::ArithmeticOverflow)?;
        let offsets = graph
            .offsets
            .then(|| OffsetPlaces::new(graph, work))
            .transpose()?;
        Ok(Self {
            targets: work.filled(m, 0)?,
            tree: work.filled(2 * leaves, 0)?,
            spare: Vec::new(),
            offsets,
        })
    }

    fn count<E>(&mut self, graph: &FlatOrder, work: &mut Work<'_, E>) -> Result<f64, E> {
        // Keep the addition order of the Number-valued inversion total.
        let mut total = 0.0;
        let max_rank = graph.rows() - 1;
        work.price(max_rank)?;
        for r in 0..max_rank {
            work.poll(r)?;
            let south_len = match &mut self.offsets {
                Some(places) => places.rank_places(graph, r + 1, work)?,
                None => graph.rank_range(r + 1).len(),
            };
            if south_len < 2 {
                continue;
            }
            let north = &graph.order[graph.rank_range(r)];
            work.price(north.len())?;
            let mut end = 0;
            for (i, &node) in north.iter().enumerate() {
                work.poll(i)?;
                let range = graph.outgoing.range(node);
                let next = end + range.len();
                let targets = &mut self.targets[end..next];
                end = next;
                work.price(range.len())?;
                if let Some(places) = &self.offsets {
                    let leaving = &places.leaving[range];
                    for (i, &e) in leaving.iter().enumerate() {
                        work.poll(i)?;
                        targets[i] = places.place[e];
                    }
                    if places.mixed[node] {
                        // Within each equal-offset run, shared departures don't count as crossings.
                        let dx = |e: usize| graph.ends[e].offset[Along::Down as usize];
                        let mut start = 0;
                        work.price(leaving.len().saturating_sub(1))?;
                        for (i, pair) in leaving.windows(2).enumerate() {
                            work.poll(i)?;
                            if dx(pair[0]) != dx(pair[1]) {
                                sort_targets(&mut targets[start..i + 1], &mut self.spare, work)?;
                                start = i + 1;
                            }
                        }
                        sort_targets(&mut targets[start..], &mut self.spare, work)?;
                        continue;
                    }
                } else {
                    for (i, &e) in graph.outgoing.slots[range].iter().enumerate() {
                        work.poll(i)?;
                        targets[i] = graph.positions[graph.segments[e].to];
                    }
                }
                sort_targets(targets, &mut self.spare, work)?;
            }
            if end < 2 {
                continue;
            }
            let leaves = south_len.next_power_of_two();
            let active = 2 * leaves;
            work.price(active)?;
            for chunk in self.tree[..active].chunks_mut(256) {
                work.price(0)?;
                chunk.fill(0);
            }
            work.scaled(end, leaves.ilog2() as usize + 1)?;
            for (i, &target) in self.targets[..end].iter().enumerate() {
                work.poll(i)?;
                let mut node = target + leaves;
                self.tree[node] += 1;
                while node > 1 {
                    if node % 2 == 0 {
                        total += f64::from(self.tree[node + 1]);
                    }
                    node >>= 1;
                    self.tree[node] += 1;
                }
            }
        }
        Ok(total)
    }
}

fn sort_targets<E>(
    items: &mut [usize],
    spare: &mut Vec<usize>,
    work: &mut Work<'_, E>,
) -> Result<(), E> {
    work.sort_with(items, spare, |a, b, _| Ok(a.cmp(&b)))
}

struct GroupOrder {
    sum: Vec<f64>,
    count: Vec<usize>,
    first: Vec<usize>,
    spare: Vec<usize>,
}

impl GroupOrder {
    fn new<E>(groups: &Groups, work: &mut Work<'_, E>) -> Result<Self, E> {
        let len = groups.parent.len();
        Ok(Self {
            sum: work.filled(len, 0.0)?,
            count: work.filled(len, 0)?,
            first: work.filled(len, 0)?,
            spare: Vec::new(),
        })
    }

    fn arrange<E>(
        &mut self,
        graph: &mut FlatOrder,
        rank: usize,
        key: &[f64],
        work: &mut Work<'_, E>,
    ) -> Result<(), E> {
        let groups = graph
            .groups
            .as_ref()
            .ok_or(inconsistent("group ordering needs membership"))?;
        let range = graph.rank_range(rank);
        let start = range.start;
        let row = &mut graph.order[range];
        if row.len() < 2 {
            return Ok(());
        }
        work.scaled(row.len(), 3)?;
        for (i, &v) in row.iter().enumerate() {
            work.poll(i)?;
            let mut parent = groups.of[v];
            while let Some(g) = parent {
                work.price(1)?;
                self.sum[g] = 0.0;
                self.count[g] = 0;
                parent = groups.parent[g];
            }
        }
        for (i, &v) in row.iter().enumerate() {
            work.poll(i)?;
            let mut parent = groups.of[v];
            while let Some(g) = parent {
                work.price(1)?;
                if self.count[g] == 0 {
                    self.first[g] = i + start;
                }
                self.sum[g] += key[v];
                self.count[g] += 1;
                parent = groups.parent[g];
            }
        }
        let value = |v: usize, g: Option<usize>| match g {
            Some(g) => (self.sum[g] / self.count[g] as f64, self.first[g]),
            None => (key[v], start + graph.positions[v]),
        };
        work.sort_with(row, &mut self.spare, |a, b, work| {
            let lifted = groups.lift(groups.of[a], groups.of[b], work)?;
            let (va, fa) = value(a, lifted.left);
            let (vb, fb) = value(b, lifted.right);
            Ok(compare_number(va, vb).then(fa.cmp(&fb)))
        })?;
        for (i, &v) in row.iter().enumerate() {
            work.poll(i)?;
            graph.positions[v] = i;
        }
        Ok(())
    }
}

struct Sweep {
    bary: Vec<f64>,
    moving: Vec<usize>,
    spare: Vec<usize>,
    groups: Option<GroupOrder>,
}

impl Sweep {
    fn run<E>(
        &mut self,
        graph: &mut FlatOrder,
        down: bool,
        bias_right: bool,
        work: &mut Work<'_, E>,
    ) -> Result<(), E> {
        let max_rank = graph.rows() - 1;
        work.price(max_rank)?;
        for step in 1..=max_rank {
            work.poll(step - 1)?;
            let rank = if down { step } else { max_rank - step };
            if graph.fixed[rank] {
                continue;
            }
            let range = graph.rank_range(rank);
            let row = &graph.order[range.clone()];
            if row.len() < 2 {
                continue;
            }
            let adjacent = if down {
                &graph.incoming
            } else {
                &graph.outgoing
            };
            let other = if down { Along::Down } else { Along::Up };
            let across = graph
                .rank_range(if down { rank - 1 } else { rank + 1 })
                .len();
            work.price(row.len())?;
            self.moving.clear();
            for (i, &node) in row.iter().enumerate() {
                work.poll(i)?;
                if graph.vertices[node].pin != 0 {
                    self.bary[node] = f64::from(graph.vertices[node].pin) * 1e9;
                    self.moving.push(node);
                    continue;
                }
                let neighbors = adjacent.at(node);
                if neighbors.is_empty() {
                    self.bary[node] = if self.groups.is_some() {
                        i as f64 * across as f64 / row.len() as f64
                    } else {
                        -1.0
                    };
                    continue;
                }
                work.price(neighbors.len())?;
                let mut sum = 0.0;
                for (i, &e) in neighbors.iter().enumerate() {
                    work.poll(i)?;
                    let segment = graph.segments[e];
                    let u = if down { segment.from } else { segment.to };
                    sum += graph.positions[u] as f64;
                    if graph.offsets && graph.vertices[u].size.width > 0.0 {
                        sum += graph.ends[e].offset[other as usize] / graph.vertices[u].size.width;
                    }
                }
                self.bary[node] = sum / neighbors.len() as f64;
                self.moving.push(node);
            }
            if let Some(groups) = &mut self.groups {
                groups.arrange(graph, rank, &self.bary, work)?;
                continue;
            }
            if self.moving.len() < 2 {
                continue;
            }
            let bary = &self.bary;
            let pos = &graph.positions;
            work.sort_with(&mut self.moving, &mut self.spare, |a, b, _| {
                // The source's comparator never orders NaN against a different value.
                let order = if bary[a] == bary[b] {
                    if bias_right {
                        pos[b].cmp(&pos[a])
                    } else {
                        pos[a].cmp(&pos[b])
                    }
                } else {
                    compare_number(bary[a], bary[b])
                };
                Ok(order)
            })?;
            let row = &mut graph.order[range];
            work.scaled(row.len(), 2)?;
            let mut next = self.moving.iter().copied();
            for (i, v) in row.iter_mut().enumerate() {
                work.poll(i)?;
                if bary[*v] != -1.0 {
                    *v = next
                        .next()
                        .ok_or(inconsistent("barycenter lost a moving slot"))?;
                }
            }
            if next.next().is_some() {
                return Err(inconsistent("barycenter collides with an immovable key"));
            }
            for (i, &v) in row.iter().enumerate() {
                work.poll(i)?;
                graph.positions[v] = i;
            }
        }
        Ok(())
    }
}

fn transpose<E>(graph: &mut FlatOrder, work: &mut Work<'_, E>) -> Result<(), E> {
    for _ in 0..TRANSPOSE_PASSES {
        let mut improved = false;
        work.price(graph.rows())?;
        for rank in 0..graph.rows() {
            work.poll(rank)?;
            if graph.fixed[rank] {
                continue;
            }
            let range = graph.rank_range(rank);
            let n = range.len().saturating_sub(1);
            work.price(n)?;
            for offset in 0..n {
                work.poll(offset)?;
                let i = range.start + offset;
                let (a, b) = (graph.order[i], graph.order[i + 1]);
                if graph.vertices[a].pin != graph.vertices[b].pin
                    || graph.groups.as_ref().is_some_and(|g| g.of[a] != g.of[b])
                {
                    continue;
                }
                if between(graph, b, a, work)? < between(graph, a, b, work)? {
                    graph.order.swap(i, i + 1);
                    graph.positions[b] = offset;
                    graph.positions[a] = offset + 1;
                    improved = true;
                }
            }
        }
        if !improved {
            break;
        }
    }
    Ok(())
}

fn between<E>(graph: &FlatOrder, a: usize, b: usize, work: &mut Work<'_, E>) -> Result<f64, E> {
    let mut crossings = 0.0;
    for (up, adjacent) in [(true, &graph.incoming), (false, &graph.outgoing)] {
        let left = adjacent.at(a);
        let right = adjacent.at(b);
        if left.is_empty() || right.is_empty() {
            continue;
        }
        work.scaled(left.len(), right.len())?;
        let at = |e: usize| {
            let segment = graph.segments[e];
            let (u, other) = if up {
                (segment.from, Along::Down)
            } else {
                (segment.to, Along::Up)
            };
            let pos = graph.positions[u] as f64;
            let width = graph.vertices[u].size.width;
            if graph.offsets && width > 0.0 {
                pos + graph.ends[e].offset[other as usize] / width
            } else {
                pos
            }
        };
        for (i, &e) in left.iter().enumerate() {
            work.poll(i)?;
            let x = at(e);
            for (j, &e) in right.iter().enumerate() {
                work.poll(j)?;
                if x > at(e) {
                    crossings += 1.0;
                }
            }
        }
    }
    Ok(crossings)
}

fn out_of_order<E>(graph: &FlatOrder, work: &mut Work<'_, E>) -> Result<usize, E> {
    let mut total = 0;
    work.price(graph.rows())?;
    for rank in 0..graph.rows() {
        work.poll(rank)?;
        let row = &graph.order[graph.rank_range(rank)];
        work.price(row.len())?;
        for (i, &a) in row.iter().enumerate() {
            work.poll(i)?;
            if a >= graph.real {
                continue;
            }
            let rest = &row[i + 1..];
            work.price(rest.len())?;
            for (j, &b) in rest.iter().enumerate() {
                work.poll(j)?;
                total += usize::from(b < a);
            }
        }
    }
    Ok(total)
}

fn mirror<E>(graph: &mut FlatOrder, work: &mut Work<'_, E>) -> Result<(), E> {
    work.price(graph.rows())?;
    for rank in 0..graph.rows() {
        work.poll(rank)?;
        let range = graph.rank_range(rank);
        let row = &mut graph.order[range];
        work.price(row.len() / 2)?;
        for i in 0..row.len() / 2 {
            work.poll(i)?;
            row.swap(i, row.len() - 1 - i);
        }
    }
    graph.set_positions(work)
}
