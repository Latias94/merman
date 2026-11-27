//! Four balanced Brandes-Köpf alignments in solver axes.

use super::{Along, GeoResult, Geometry, Options, Work};

pub(super) fn position<E>(
    graph: &Geometry<'_>,
    options: Options,
    work: &mut Work<'_, E>,
) -> GeoResult<Vec<f64>, E> {
    let n = graph.order.vertices.len();
    let mut gap = work.reserve(n)?;
    work.price(n)?;
    for node in 0..n {
        work.poll(node)?;
        gap.push(graph.half_room(options, node));
    }
    work.finite(&gap)?;
    let conflict = conflicts(graph, work)?;
    let mut blocks = Blocks::new(n, work)?;
    let [mut a, mut b, mut c, mut d] = [
        blocks.place(graph, &gap, &conflict, Along::Up, false, work)?,
        blocks.place(graph, &gap, &conflict, Along::Up, true, work)?,
        blocks.place(graph, &gap, &conflict, Along::Down, false, work)?,
        blocks.place(graph, &gap, &conflict, Along::Down, true, work)?,
    ];

    let arrays = [&mut a, &mut b, &mut c, &mut d];
    let (mut mins, mut maxs) = ([0.0; 4], [0.0; 4]);
    let mut narrowest = 0;
    let mut best_width = f64::INFINITY;
    for (k, xs) in arrays.iter().enumerate() {
        work.price(n)?;
        let (mut lo, mut hi) = (f64::INFINITY, f64::NEG_INFINITY);
        for (i, &x) in xs.iter().enumerate() {
            work.poll(i)?;
            let half = graph.order.vertices[i].size.width / 2.0;
            if x - half < lo {
                lo = x - half;
            }
            if x + half > hi {
                hi = x + half;
            }
        }
        mins[k] = lo;
        maxs[k] = hi;
        if hi - lo < best_width {
            best_width = hi - lo;
            narrowest = k;
        }
    }
    for (k, xs) in arrays.into_iter().enumerate() {
        let delta = if k % 2 == 0 {
            mins[narrowest] - mins[k]
        } else {
            maxs[narrowest] - maxs[k]
        };
        if delta != 0.0 {
            work.price(n)?;
            for (i, x) in xs.iter_mut().enumerate() {
                work.poll(i)?;
                *x += delta;
            }
        }
    }
    // The reference's five comparisons leave the middle two at b and c.
    work.price(n)?;
    for i in 0..n {
        work.poll(i)?;
        let (mut a0, mut b0, mut c0, mut d0) = (a[i], b[i], c[i], d[i]);
        if a0 > b0 {
            std::mem::swap(&mut a0, &mut b0);
        }
        if c0 > d0 {
            std::mem::swap(&mut c0, &mut d0);
        }
        if a0 > c0 {
            std::mem::swap(&mut a0, &mut c0);
        }
        if b0 > d0 {
            std::mem::swap(&mut b0, &mut d0);
        }
        if b0 > c0 {
            std::mem::swap(&mut b0, &mut c0);
        }
        a[i] = (b0 + c0) / 2.0;
    }
    Ok(a)
}

fn conflicts<E>(graph: &Geometry<'_>, work: &mut Work<'_, E>) -> GeoResult<Vec<bool>, E> {
    let mut conflict = work.filled(graph.segments().len(), false)?;
    let positions = &graph.order.positions;
    work.price(graph.rows() - 1)?;
    for r in 1..graph.rows() {
        work.poll(r - 1)?;
        let row = graph.row(r);
        let mut scan = 0;
        let mut k0 = 0;
        work.price(row.len())?;
        for (i, &v) in row.iter().enumerate() {
            work.poll(i)?;
            let inner = if graph.order.vertices[v].kind.is_dummy() {
                let u = graph.other(graph.adjacent(v, Along::Up)[0], Along::Up);
                graph.order.vertices[u].kind.is_dummy().then_some(u)
            } else {
                None
            };
            let k1 = inner.map_or(graph.row(r - 1).len(), |u| positions[u]);
            if inner.is_some() || i + 1 == row.len() {
                let slice = &row[scan..=i];
                work.price(slice.len())?;
                for (j, &w) in slice.iter().enumerate() {
                    work.poll(j)?;
                    let incoming = graph.adjacent(w, Along::Up);
                    work.price(incoming.len())?;
                    for (index, &e) in incoming.iter().enumerate() {
                        work.poll(index)?;
                        let u = graph.other(e, Along::Up);
                        let up = positions[u];
                        if (up < k0 || k1 < up)
                            && !(graph.order.vertices[w].kind.is_dummy()
                                && graph.order.vertices[u].kind.is_dummy())
                        {
                            conflict[e] = true;
                        }
                    }
                }
                scan = i + 1;
                k0 = k1;
            }
        }
    }
    Ok(conflict)
}

struct Constraint {
    from: usize,
    to: usize,
    gap: f64,
}

struct Blocks {
    root: Vec<usize>,
    align: Vec<usize>,
    shift: Vec<f64>,
    constraints: Vec<Constraint>,
    indegree: Vec<usize>,
    starts: Vec<usize>,
    slots: Vec<usize>,
    fill: Vec<usize>,
    topo: Vec<usize>,
    x: Vec<f64>,
}

impl Blocks {
    fn new<E>(n: usize, work: &mut Work<'_, E>) -> GeoResult<Self, E> {
        Ok(Self {
            root: work.filled(n, 0)?,
            align: work.filled(n, 0)?,
            shift: work.filled(n, 0.0)?,
            constraints: work.reserve(n)?,
            indegree: work.filled(n, 0)?,
            starts: work.filled(n + 1, 0)?,
            slots: work.filled(n, 0)?,
            fill: work.filled(n, 0)?,
            topo: work.reserve(n)?,
            x: work.filled(n, 0.0)?,
        })
    }

    fn place<E>(
        &mut self,
        graph: &Geometry<'_>,
        gap: &[f64],
        conflict: &[bool],
        along: Along,
        right: bool,
        work: &mut Work<'_, E>,
    ) -> GeoResult<Vec<f64>, E> {
        let n = self.root.len();
        self.constraints.clear();
        self.topo.clear();
        work.scaled(n, 6)?;
        for i in 0..n {
            work.poll(i)?;
            self.root[i] = i;
            self.align[i] = i;
            self.shift[i] = 0.0;
            self.indegree[i] = 0;
            self.starts[i + 1] = 0;
            self.x[i] = 0.0;
        }
        self.medians(graph, conflict, along, right, work)?;

        work.price(graph.rows())?;
        for r in 0..graph.rows() {
            work.poll(r)?;
            let row = graph.row(r);
            work.price(row.len().saturating_sub(1))?;
            for (i, pair) in row.windows(2).enumerate() {
                work.poll(i)?;
                let (a, b) = if right {
                    (pair[1], pair[0])
                } else {
                    (pair[0], pair[1])
                };
                let sep = gap[a] + gap[b];
                let gap = if right {
                    sep - self.shift[a] + self.shift[b]
                } else {
                    sep + self.shift[a] - self.shift[b]
                };
                let from = self.root[a];
                let to = self.root[b];
                self.constraints.push(Constraint { from, to, gap });
                self.indegree[to] += 1;
                self.starts[from + 1] += 1;
            }
        }
        self.solve_constraints(work)?;

        let mut xs = work.reserve(n)?;
        work.price(n)?;
        for (i, &root) in self.root.iter().enumerate() {
            work.poll(i)?;
            let x = self.x[root];
            xs.push((if right { -x } else { x }) + self.shift[i]);
        }
        Ok(xs)
    }

    fn medians<E>(
        &mut self,
        graph: &Geometry<'_>,
        conflict: &[bool],
        along: Along,
        right: bool,
        work: &mut Work<'_, E>,
    ) -> GeoResult<(), E> {
        let rows = graph.rows();
        work.price(rows)?;
        for step in 0..rows {
            work.poll(step)?;
            let r = if along == Along::Up {
                step
            } else {
                rows - 1 - step
            };
            let adjacent = if along == Along::Up {
                r.checked_sub(1)
            } else {
                (r + 1 < rows).then_some(r + 1)
            };
            let adjacent_len = adjacent.map_or(0, |r| graph.row(r).len());
            let row = graph.row(r);
            let mut prev = None;
            work.price(row.len())?;
            for i in 0..row.len() {
                work.poll(i)?;
                let v = row[if right { row.len() - 1 - i } else { i }];
                let edges = graph.adjacent(v, along);
                if edges.is_empty() {
                    continue;
                }
                let lo = (edges.len() - 1) / 2;
                let hi = edges.len() / 2;
                let medians = if right { [hi, lo] } else { [lo, hi] };
                for index in medians.into_iter().take(if lo == hi { 1 } else { 2 }) {
                    let e = edges[index];
                    let u = graph.other(e, along);
                    let pos = graph.order.positions[u];
                    let idx = if right { adjacent_len - 1 - pos } else { pos };
                    if self.align[v] == v && prev.is_none_or(|p| p < idx) && !conflict[e] {
                        self.align[u] = v;
                        self.root[v] = self.root[u];
                        self.align[v] = self.root[v];
                        self.shift[v] =
                            self.shift[u] + graph.offset(e, along.other()) - graph.offset(e, along);
                        prev = Some(idx);
                    }
                }
            }
        }
        Ok(())
    }

    fn solve_constraints<E>(&mut self, work: &mut Work<'_, E>) -> GeoResult<(), E> {
        let n = self.root.len();
        work.scaled(n, 3)?;
        for i in 0..n {
            work.poll(i)?;
            self.starts[i + 1] += self.starts[i];
            self.fill[i] = self.starts[i];
            if self.root[i] == i && self.indegree[i] == 0 {
                self.topo.push(i);
            }
        }
        work.price(self.constraints.len())?;
        for (e, constraint) in self.constraints.iter().enumerate() {
            work.poll(e)?;
            let fill = &mut self.fill[constraint.from];
            self.slots[*fill] = e;
            *fill += 1;
        }
        work.price(n)?;
        let mut cursor = 0;
        while let Some(&v) = self.topo.get(cursor) {
            work.poll(cursor)?;
            cursor += 1;
            let outgoing = &self.slots[self.starts[v]..self.starts[v + 1]];
            work.price(outgoing.len())?;
            for (index, &e) in outgoing.iter().enumerate() {
                work.poll(index)?;
                let c = &self.constraints[e];
                let next = self.x[v] + c.gap;
                if next > self.x[c.to] {
                    self.x[c.to] = next;
                }
                self.indegree[c.to] -= 1;
                if self.indegree[c.to] == 0 {
                    self.topo.push(c.to);
                }
            }
        }
        work.price(self.topo.len())?;
        for (index, &v) in self.topo.iter().rev().enumerate() {
            work.poll(index)?;
            let outgoing = &self.slots[self.starts[v]..self.starts[v + 1]];
            let mut min = f64::INFINITY;
            work.price(outgoing.len())?;
            for (index, &e) in outgoing.iter().enumerate() {
                work.poll(index)?;
                let c = &self.constraints[e];
                let candidate = self.x[c.to] - c.gap;
                if candidate < min {
                    min = candidate;
                }
            }
            if min != f64::INFINITY && min > self.x[v] {
                self.x[v] = min;
            }
        }
        Ok(())
    }
}
