//! Rank bands, inter-rank pieces, and solver-axis route knots.

use super::super::level::PlacedLevelEdge;
use super::tracks;
use super::{
    Along, GeoResult, Geometry, GeometryError, Kind, Options, PlacedLevel, Point, Routing, Work,
    js_max, js_min,
};

const MIN_SLOPE: f64 = 0.125;
const MAX_GAP: f64 = 3.0;

#[derive(Clone, Copy)]
pub(super) struct Piece {
    pub from: f64,
    pub to: f64,
    pub rank: usize,
    lower: Option<usize>,
    pub wide: bool,
    pub track: Option<usize>,
    pub levels: usize,
}

impl Piece {
    fn new(from: f64, to: f64, rank: usize, lower: Option<usize>) -> Self {
        Self {
            from,
            to,
            rank,
            lower,
            wide: false,
            track: None,
            levels: 0,
        }
    }
}

struct Pieces {
    items: Vec<Piece>,
    starts: Vec<usize>,
}

fn pieces<E>(
    graph: &Geometry<'_>,
    xs: &[f64],
    empty: &[bool],
    on_line: &[bool],
    stepped: bool,
    work: &mut Work<'_, E>,
) -> GeoResult<Pieces, E> {
    let edge_count = graph.plan.edges.len();
    let mut items = work.reserve(graph.segments().len())?;
    let mut starts = work.reserve(edge_count + 1)?;
    work.price(edge_count)?;
    for (slot, edge) in graph.plan.edges.iter().enumerate() {
        work.poll(slot)?;
        starts.push(items.len());
        if edge.direct {
            continue;
        }
        let dummies = graph.dummy_range(slot);
        work.price(dummies.len())?;
        let mut twin = false;
        for d in dummies.clone() {
            work.poll(d - dummies.start)?;
            let bare = empty
                .get(graph.order.vertices[d].rank)
                .copied()
                .unwrap_or(false);
            if !stepped && bare != on_line[d] {
                twin = true;
            }
        }
        if twin {
            continue;
        }

        let (a, b) = edge.rank_endpoints();
        let segments = &graph.order.edges[slot].segments;
        let mut x = xs[a] + graph.offset(segments.start, Along::Down);
        let mut rank = graph.order.vertices[a].rank;
        work.price(dummies.len() + 1)?;
        for d in dummies.clone() {
            work.poll(d - dummies.start)?;
            let r = graph.order.vertices[d].rank;
            if empty.get(r).copied().unwrap_or(false) {
                continue;
            }
            items.push(Piece::new(x, xs[d], rank, Some(d)));
            x = xs[d];
            rank = r;
        }
        let end = xs[b] + graph.offset(segments.end - 1, Along::Up);
        items.push(Piece::new(x, end, rank, None));
    }
    starts.push(items.len());
    Ok(Pieces { items, starts })
}

#[derive(Clone, Copy, Default)]
struct Band {
    top: f64,
    bottom: f64,
}

pub(super) fn layout_bands<E>(
    graph: &Geometry<'_>,
    xs: &[f64],
    empty: &[bool],
    on_line: &[bool],
    options: Options,
    work: &mut Work<'_, E>,
) -> GeoResult<PlacedLevel, E> {
    let stepped = options.routing == Routing::Orthogonal;
    let mut pieces = if stepped || (graph.spread && options.routing != Routing::Polyline) {
        Some(pieces(graph, xs, empty, on_line, stepped, work)?)
    } else {
        None
    };
    let rows = graph.rows();
    let max_rank = rows - 1;
    let mut run = work.filled(rows, 0.0)?;
    work.price(graph.segments().len())?;
    for (e, segment) in graph.segments().iter().enumerate() {
        work.poll(e)?;
        let across = (xs[segment.from] + graph.offset(e, Along::Down)
            - xs[segment.to]
            - graph.offset(e, Along::Up))
        .abs();
        let rank = graph.order.vertices[segment.from].rank;
        if across > run[rank] {
            run[rank] = across;
        }
    }
    let step = graph.plan.step as usize;
    let start_ranks = if port_only(graph, 0, Kind::StartPort, work)? {
        step
    } else {
        0
    };
    let end_ranks = if port_only(graph, max_rank, Kind::EndPort, work)? {
        step
    } else {
        0
    };
    let normal_gap = options.rank_sep / step as f64;
    let port_gap = options.port_sep / step as f64;
    let mut gaps = work.filled(rows, 0.0)?;
    work.price(max_rank)?;
    for r in 0..max_rank {
        work.poll(r)?;
        let usual = if r < start_ranks || r >= max_rank.saturating_sub(end_ranks) {
            port_gap
        } else {
            normal_gap
        };
        gaps[r] = if pieces.is_some() {
            usual
        } else {
            js_max(
                usual,
                js_min(MAX_GAP * options.rank_sep / step as f64, run[r] * MIN_SLOPE),
            )
        };
    }
    let mut between = work.filled(rows, true)?;
    let mut has_arrivals = work.filled(rows, false)?;
    let mut has_departures = work.filled(rows, false)?;
    work.price(graph.real())?;
    for node in 0..graph.real() {
        work.poll(node)?;
        let v = graph.order.vertices[node];
        let rank = v.rank;
        between[rank] = false;
        if graph.spread && v.kind == Kind::Node {
            has_arrivals[rank] |= !graph.adjacent(node, Along::Up).is_empty();
            has_departures[rank] |= !graph.adjacent(node, Along::Down).is_empty();
        }
    }
    if !stepped {
        work.price(max_rank.saturating_sub(1))?;
        for r in 1..max_rank {
            work.poll(r - 1)?;
            let heads = graph.order.label_heads[r];
            if !between[r] || !matches!(heads, 1 | 2) {
                continue;
            }
            let shift = js_min(
                options.head_room / 2.0,
                gaps[if heads == 1 { r - 1 } else { r }] / 2.0,
            );
            gaps[r - 1] += if heads == 1 { -shift } else { shift };
            gaps[r] += if heads == 1 { shift } else { -shift };
        }
    }

    let (margin, least, most) = if stepped {
        (20.0, 10.0, 14.0)
    } else {
        (28.0, 12.0, 12.0)
    };
    let mut below = Vec::new();
    let mut track_count = Vec::new();
    if let Some(pieces) = &mut pieces {
        let mut room = work.filled(rows, 0.0)?;
        below = work.filled(rows, 0)?;
        work.price(max_rank)?;
        for r in 0..max_rank {
            work.poll(r)?;
            if empty.get(r).copied().unwrap_or(false) {
                continue;
            }
            let mut next = r + 1;
            room[r] = gaps[r];
            while next < max_rank && empty.get(next).copied().unwrap_or(false) {
                work.price(1)?;
                room[r] += gaps[next];
                next += 1;
            }
            below[r] = next;
        }
        work.price(pieces.items.len())?;
        for (i, piece) in pieces.items.iter_mut().enumerate() {
            work.poll(i)?;
            let across = (piece.from - piece.to).abs();
            if !across.is_finite() {
                return Err(GeometryError::NonFiniteGeometry);
            }
            piece.wide = across > 2.0 * room[piece.rank];
        }
        let track_bands = tracks::assign(&mut pieces.items, &room, stepped, work)?;
        track_count = track_bands.count;
        work.price(max_rank)?;
        for r in 0..max_rank {
            work.poll(r)?;
            if track_count[r] > 0 {
                gaps[r] += js_max(
                    0.0,
                    2.0 * margin + (track_count[r] - 1) as f64 * least - room[r],
                );
            } else if track_bands.tall[r] > 0.0 {
                gaps[r] += js_max(
                    0.0,
                    js_min(MAX_GAP * options.rank_sep, track_bands.tall[r]) - room[r],
                );
            }
        }
    }

    let mut bands = work.reserve(rows)?;
    let mut y = 0.0;
    work.price(rows)?;
    for (r, &under) in gaps.iter().enumerate() {
        work.poll(r)?;
        let mut h = 0.0;
        let row = graph.row(r);
        work.price(row.len())?;
        for (i, &node) in row.iter().enumerate() {
            work.poll(i)?;
            let height = graph.order.vertices[node].size.height;
            if height > h {
                h = height;
            }
        }
        bands.push(Band {
            top: y,
            bottom: y + h,
        });
        y += h + under;
    }
    let (mut min_x, mut max_x) = (f64::INFINITY, f64::NEG_INFINITY);
    work.price(xs.len())?;
    for (i, &x) in xs.iter().enumerate() {
        work.poll(i)?;
        let half = graph.order.vertices[i].size.width / 2.0;
        if x - half < min_x {
            min_x = x - half;
        }
        if x + half > max_x {
            max_x = x + half;
        }
    }
    let width = max_x - min_x;
    if !width.is_finite() || !y.is_finite() {
        return Err(GeometryError::NonFiniteGeometry);
    }

    let mut centers = work.reserve(graph.real())?;
    work.price(graph.real())?;
    for (node, &x) in xs.iter().enumerate().take(graph.real()) {
        work.poll(node)?;
        let vert = graph.order.vertices[node];
        let band = bands[vert.rank];
        let y = if vert.kind == Kind::Node && has_departures[vert.rank] && !has_arrivals[vert.rank]
        {
            band.bottom - vert.size.height / 2.0
        } else if vert.kind == Kind::Node && has_arrivals[vert.rank] && !has_departures[vert.rank] {
            band.top + vert.size.height / 2.0
        } else {
            (band.top + band.bottom) / 2.0
        };
        let x = x - min_x;
        if !x.is_finite() || !y.is_finite() {
            return Err(GeometryError::NonFiniteGeometry);
        }
        centers.push(Point { x, y });
    }

    let routes = Routes {
        graph,
        xs,
        on_line,
        stepped,
        pieces,
        bands,
        below,
        track_count,
        margin,
        most_pitch: most,
        between,
        min_x,
    };
    let mut edges = work.reserve(graph.plan.edges.len())?;
    work.price(graph.plan.edges.len())?;
    for slot in 0..graph.plan.edges.len() {
        work.poll(slot)?;
        edges.push(routes.path(slot, &centers, work)?);
    }
    Ok(PlacedLevel {
        size: super::Size { width, height: y },
        centers,
        edges,
    })
}

fn port_only<E>(
    graph: &Geometry<'_>,
    rank: usize,
    kind: Kind,
    work: &mut Work<'_, E>,
) -> GeoResult<bool, E> {
    let row = graph.row(rank);
    work.price(row.len())?;
    let mut found = false;
    for (i, &node) in row.iter().enumerate() {
        work.poll(i)?;
        let k = graph.order.vertices[node].kind;
        if k == kind {
            found = true;
        } else if k != Kind::Bend {
            return Ok(false);
        }
    }
    Ok(found)
}

struct Routes<'a, 'g> {
    graph: &'a Geometry<'g>,
    xs: &'a [f64],
    on_line: &'a [bool],
    stepped: bool,
    pieces: Option<Pieces>,
    bands: Vec<Band>,
    below: Vec<usize>,
    track_count: Vec<usize>,
    margin: f64,
    most_pitch: f64,
    between: Vec<bool>,
    min_x: f64,
}

impl Routes<'_, '_> {
    fn track_y(&self, piece: Piece, track: usize) -> f64 {
        let top = self.bands[piece.rank].bottom;
        let bottom = self.bands[self.below[piece.rank]].top;
        let count = if self.stepped {
            self.track_count[piece.rank]
        } else {
            piece.levels
        };
        let pitch = if count > 1 {
            js_min(
                self.most_pitch,
                (bottom - top - 2.0 * self.margin) / (count - 1) as f64,
            )
        } else {
            0.0
        };
        (top + bottom) / 2.0 + (track as f64 - (count - 1) as f64 / 2.0) * pitch
    }

    fn bend(&self, points: &mut Vec<Point>, node: usize) {
        let v = self.graph.order.vertices[node];
        let Band { top, bottom } = self.bands[v.rank];
        let x = self.xs[node] - self.min_x;
        let inset = if v.kind == Kind::Bend && self.between[v.rank] && !self.stepped {
            (bottom - top) / 4.0
        } else {
            0.0
        };
        points.push(Point { x, y: top + inset });
        if bottom > top {
            points.push(Point {
                x,
                y: bottom - inset,
            });
        }
    }

    fn path<E>(
        &self,
        slot: usize,
        centers: &[Point],
        work: &mut Work<'_, E>,
    ) -> GeoResult<PlacedLevelEdge, E> {
        let graph = self.graph;
        let edge = graph.plan.edges[slot];
        let expansion = &graph.order.edges[slot];
        let (a, b) = edge.rank_endpoints();
        let mut first_dx = expansion.original[Along::Down as usize].dx;
        let mut last_dx = expansion.original[Along::Up as usize].dx;
        if graph.spread && !expansion.segments.is_empty() {
            first_dx = graph.offset(expansion.segments.start, Along::Down);
            last_dx = graph.offset(expansion.segments.end - 1, Along::Up);
        }
        let pieces = self
            .pieces
            .as_ref()
            .map_or(&[][..], |p| &p.items[p.starts[slot]..p.starts[slot + 1]]);
        let dummies = graph.dummy_range(slot);
        let interior = if pieces.is_empty() {
            dummies.len().checked_mul(2)
        } else {
            pieces.len().checked_mul(4)
        };
        let cap = interior
            .and_then(|n| n.checked_add(2))
            .ok_or(GeometryError::ArithmeticOverflow)?;
        let mut points = work.reserve(cap)?;
        points.push(Point {
            x: centers[a].x + first_dx,
            y: centers[a].y + graph.order.vertices[a].size.height / 2.0,
        });
        if pieces.is_empty() {
            work.price(dummies.len())?;
            for node in dummies.clone() {
                work.poll(node - dummies.start)?;
                if !self.on_line[node] {
                    self.bend(&mut points, node);
                }
            }
        } else {
            work.price(pieces.len())?;
            for (i, &piece) in pieces.iter().enumerate() {
                work.poll(i)?;
                if let Some(track) = piece.track {
                    if self.stepped || piece.wide {
                        let y = self.track_y(piece, track);
                        points.push(Point {
                            x: piece.from - self.min_x,
                            y,
                        });
                        points.push(Point {
                            x: piece.to - self.min_x,
                            y,
                        });
                    } else if track > 0 {
                        points.push(Point {
                            x: piece.from - self.min_x,
                            y: self.track_y(piece, track - 1),
                        });
                    }
                }
                if let Some(node) = piece.lower {
                    self.bend(&mut points, node);
                }
            }
        }
        points.push(Point {
            x: centers[b].x + last_dx,
            y: centers[b].y - graph.order.vertices[b].size.height / 2.0,
        });
        if edge.reversed {
            work.price(points.len() / 2)?;
            for i in 0..points.len() / 2 {
                work.poll(i)?;
                let last = points.len() - 1 - i;
                points.swap(i, last);
            }
        }
        let label = if let Some(node) = expansion.label_node {
            let band = self.bands[graph.order.vertices[node].rank];
            Point {
                x: self.xs[node] - self.min_x,
                y: (band.top + band.bottom) / 2.0,
            }
        } else if expansion.label.width > 0.0 {
            let last = points[points.len() - 1];
            Point {
                x: (points[0].x + last.x) / 2.0,
                y: (points[0].y + last.y) / 2.0,
            }
        } else {
            Point::default()
        };
        work.price(points.len())?;
        for (i, p) in points.iter().enumerate() {
            work.poll(i)?;
            if !p.x.is_finite() || !p.y.is_finite() {
                return Err(GeometryError::NonFiniteGeometry);
            }
        }
        if !label.x.is_finite() || !label.y.is_finite() {
            return Err(GeometryError::NonFiniteGeometry);
        }
        Ok(PlacedLevelEdge {
            points,
            label,
            reversed_for_ranking: edge.reversed,
        })
    }
}
