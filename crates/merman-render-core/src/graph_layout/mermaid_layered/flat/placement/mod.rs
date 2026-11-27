//! Solver-axis coordinates and route knots. Compound applies direction and padding.

use std::ops::Range;

use super::super::{Routing, Size};
use super::level::LevelOptions;
use super::order::{Along, End, FlatOrder, Kind, Segment};
use super::ranking::RankPlan;
use super::work::{compare_number, js_max, js_min};
use super::{
    CompoundError as GeometryError, LevelPlan, LevelResult, Mode, OrderedLevel, PlacedLevel, Point,
    RankPoint, Result, Work, add, inconsistent,
};

mod adjust;
mod bands;
mod position;
mod tracks;

const IN_LINE: f64 = 0.5;
const TWIN: f64 = 1.0;

type GeoResult<T, E> = Result<T, E>;

#[derive(Clone, Copy)]
struct Options {
    routing: Routing,
    node_sep: f64,
    edge_sep: f64,
    rank_sep: f64,
    port_sep: f64,
    head_room: f64,
}

impl From<LevelOptions> for Options {
    fn from(options: LevelOptions) -> Self {
        Self {
            routing: options.routing,
            node_sep: options.spacing.node,
            edge_sep: options.spacing.edge,
            rank_sep: options.spacing.rank,
            port_sep: options.port_spacing,
            head_room: options.head_room,
        }
    }
}

pub(super) fn finish<E>(
    plan: LevelPlan<'_>,
    ranked: &RankPlan,
    ordered: &mut FlatOrder,
    mode: Mode,
    work: &mut Work<'_, E>,
) -> Result<LevelResult, E> {
    let options = plan.options.into();
    let mut graph = Geometry {
        plan: ranked,
        order: ordered,
        free: [Vec::new(), Vec::new()],
        spread: false,
    };
    graph.sort_adjacency(work)?;
    if mode == Mode::OrderOnly {
        let xs = position::position(&graph, options, work)?;
        work.finite(&xs)?;
        return Ok(LevelResult::Ordered(graph.ordered(&xs, work)?));
    }

    let mut spread = false;
    work.price(graph.real())?;
    for (i, v) in graph.order.vertices[..graph.real()].iter().enumerate() {
        work.poll(i)?;
        spread |= v.kind == Kind::Node && v.span > 0.0;
    }
    graph.spread = spread;
    let sides = if spread {
        adjust::spread(&mut graph, work)?
    } else {
        Vec::new()
    };
    let mut xs = position::position(&graph, options, work)?;
    work.finite(&xs)?;
    let empty = adjust::empty_ranks(&graph, work)?;
    if spread {
        graph.count_free(work)?;
        adjust::align_labels(&graph, &mut xs, &empty, options, work)?;
        adjust::settle(&mut graph, &xs, &empty, &sides, work)?;
        adjust::snap(&graph, &mut xs, &empty, options, work)?;
    }
    adjust::align_ports(&graph, &mut xs, &empty, work)?;
    work.finite(&xs)?;
    let mut on_line = work.filled(xs.len(), false)?;
    if options.routing != Routing::Orthogonal {
        if !empty.is_empty() {
            adjust::straighten(
                &graph,
                &mut xs,
                &empty,
                options.edge_sep,
                &mut on_line,
                work,
            )?;
        }
        adjust::past_labels(&graph, &mut xs, options.edge_sep, &mut on_line, work)?;
    }
    work.finite(&xs)?;
    Ok(LevelResult::Placed(bands::layout_bands(
        &graph, &xs, &empty, &on_line, options, work,
    )?))
}

struct Geometry<'a> {
    plan: &'a RankPlan,
    order: &'a mut FlatOrder,
    free: [Vec<usize>; 2],
    spread: bool,
}

impl Geometry<'_> {
    fn real(&self) -> usize {
        self.order.real
    }

    fn rows(&self) -> usize {
        self.order.rows()
    }

    fn row(&self, rank: usize) -> &[usize] {
        &self.order.order[self.order.rank_range(rank)]
    }

    fn dummy_range(&self, edge: usize) -> Range<usize> {
        self.order.edges[edge].dummies.clone()
    }

    fn segments(&self) -> &[Segment] {
        &self.order.segments
    }

    fn adjacent(&self, node: usize, along: Along) -> &[usize] {
        match along {
            Along::Up => self.order.incoming.at(node),
            Along::Down => self.order.outgoing.at(node),
        }
    }

    fn free(&self, node: usize, along: Along) -> usize {
        if self.order.vertices[node].kind == Kind::Node {
            self.free[along as usize][node]
        } else {
            0
        }
    }

    fn count_free<E>(&mut self, work: &mut Work<'_, E>) -> Result<(), E> {
        let n = self.order.vertices.len();
        let mut up = work.filled(n, 0)?;
        let mut down = work.filled(n, 0)?;
        work.price(self.order.segments.len())?;
        for (i, s) in self.order.segments.iter().enumerate() {
            work.poll(i)?;
            up[s.to] += usize::from(self.end(i, Along::Up) != End::Held);
            down[s.from] += usize::from(self.end(i, Along::Down) != End::Held);
        }
        self.free = [up, down];
        Ok(())
    }

    fn end(&self, edge: usize, along: Along) -> End {
        self.order.ends[edge].kind[along as usize]
    }

    fn offset(&self, edge: usize, along: Along) -> f64 {
        self.order.ends[edge].offset[along as usize]
    }

    fn set_offset(&mut self, edge: usize, along: Along, offset: f64) {
        self.order.ends[edge].offset[along as usize] = offset;
    }

    fn other(&self, edge: usize, along: Along) -> usize {
        let s = self.segments()[edge];
        match along {
            Along::Up => s.from,
            Along::Down => s.to,
        }
    }

    fn other_point(&self, xs: &[f64], edge: usize, along: Along) -> f64 {
        xs[self.other(edge, along)] + self.offset(edge, along.other())
    }

    fn span(&self, node: usize) -> f64 {
        let v = self.order.vertices[node];
        js_min(v.span, v.size.width)
    }

    fn half_room(&self, options: Options, node: usize) -> f64 {
        let v = self.order.vertices[node];
        v.size.width / 2.0
            + if v.kind == Kind::Node {
                options.node_sep
            } else {
                options.edge_sep
            } / 2.0
    }

    fn sort_adjacency<E>(&mut self, work: &mut Work<'_, E>) -> Result<(), E> {
        let positions = &self.order.positions;
        let segments = &self.order.segments;
        let mut spare = Vec::new();
        for (along, list) in [
            (Along::Up, &mut self.order.incoming),
            (Along::Down, &mut self.order.outgoing),
        ] {
            work.price(list.starts.len() - 1)?;
            for (v, range) in list.starts.windows(2).enumerate() {
                work.poll(v)?;
                let key = |e: usize| {
                    let s = segments[e];
                    (positions[if along == Along::Up { s.from } else { s.to }], e)
                };
                work.sort_with(
                    &mut list.slots[range[0]..range[1]],
                    &mut spare,
                    |a, b, _| Ok(key(a).cmp(&key(b))),
                )?;
            }
        }
        Ok(())
    }

    fn ordered<E>(&self, xs: &[f64], work: &mut Work<'_, E>) -> Result<OrderedLevel, E> {
        let at = |node: usize| RankPoint {
            rank: self.order.vertices[node].rank,
            cross: xs[node],
        };
        let mut nodes = work.reserve(self.real())?;
        work.price(self.real())?;
        for i in 0..self.real() {
            work.poll(i)?;
            nodes.push(at(i));
        }
        let mut edges = work.reserve(self.plan.edges.len())?;
        work.price(self.plan.edges.len())?;
        for (i, edge) in self.plan.edges.iter().enumerate() {
            work.poll(i)?;
            let dummies = self.dummy_range(i);
            let mut knots = work.reserve(add(dummies.len(), 2)?)?;
            let (a, b) = edge.rank_endpoints();
            knots.push(at(a));
            work.price(dummies.len())?;
            for (i, d) in dummies.enumerate() {
                work.poll(i)?;
                knots.push(at(d));
            }
            knots.push(at(b));
            edges.push(knots);
        }
        Ok(OrderedLevel {
            nodes,
            edges,
            crossings: self.order.crossings,
            declared_crossings: self.order.declared_crossings,
        })
    }
}
