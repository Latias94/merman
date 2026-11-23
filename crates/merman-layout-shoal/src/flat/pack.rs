//! Translate complete components together, including middle/terminal labels and fixed-side detours.

use super::*;
use crate::work::reserved;
use crate::{EdgeGeometry, Endpoint, LayoutOutput, LayoutStats, PackingPolicy, Point, Rect, Side};

const PRECISION: LayoutError = LayoutError::InvalidInput("layout exceeds coordinate precision");

impl Drawing {
    fn transform(&mut self, direction: Direction, delta: Point) {
        let point = |x: f64, y: f64| {
            let (x, y) = match direction {
                Direction::Down => (x, y),
                Direction::Up => (x, -y),
                Direction::Right => (y, x),
                Direction::Left => (-y, x),
            };
            Point {
                x: x + delta.x,
                y: y + delta.y,
            }
        };
        for rect in &mut self.nodes {
            let far = point(rect.x + rect.width, rect.y + rect.height);
            let near = point(rect.x, rect.y);
            rect.x = near.x.min(far.x);
            rect.y = near.y.min(far.y);
            if matches!(direction, Direction::Right | Direction::Left) {
                std::mem::swap(&mut rect.width, &mut rect.height);
            }
        }
        for (_, e) in &mut self.edges {
            e.ports = (
                physical_side(direction, e.ports.0),
                physical_side(direction, e.ports.1),
            );
            for p in &mut e.points {
                let q = point(p.x, p.y);
                p.x = q.x;
                p.y = q.y;
                if matches!(direction, Direction::Right | Direction::Left) {
                    p.axis = match p.axis {
                        crate::Axis::Vertical => crate::Axis::Horizontal,
                        crate::Axis::Horizontal => crate::Axis::Vertical,
                    };
                }
            }
            for p in [
                &mut e.label_center,
                &mut e.start_label_center,
                &mut e.end_label_center,
            ]
            .into_iter()
            .flatten()
            {
                *p = point(p.x, p.y);
            }
        }
    }

    fn bounds(&self, graph: &Graph, work: &mut Work<'_>) -> LayoutResult<Rect> {
        let (mut min_x, mut min_y, mut max_x, mut max_y) = (
            f64::INFINITY,
            f64::INFINITY,
            f64::NEG_INFINITY,
            f64::NEG_INFINITY,
        );
        let mut include = |x: f64, y: f64| -> LayoutResult<()> {
            if !x.is_finite() || !y.is_finite() {
                return Err(PRECISION);
            }
            min_x = min_x.min(x);
            min_y = min_y.min(y);
            max_x = max_x.max(x);
            max_y = max_y.max(y);
            Ok(())
        };
        for r in &self.nodes {
            work.visit(1)?;
            include(r.x, r.y)?;
            include(r.x + r.width, r.y + r.height)?;
        }
        for (id, geom) in &self.edges {
            let e = &graph.edges[*id as usize];
            for (center, size) in [
                (geom.label_center, e.label),
                (geom.start_label_center, e.start_label),
                (geom.end_label_center, e.end_label),
            ] {
                match (center, size) {
                    (Some(p), Some(s)) => {
                        work.visit(1)?;
                        include(p.x - s.width * 0.5, p.y - s.height * 0.5)?;
                        include(p.x + s.width * 0.5, p.y + s.height * 0.5)?;
                    }
                    (None, None) => {}
                    _ => {
                        return Err(LayoutError::InvalidInput(
                            "missing or unexpected label geometry",
                        ));
                    }
                }
            }
            for p in &geom.points {
                work.visit(1)?;
                include(p.x, p.y)?;
            }
        }
        let (width, height) = (max_x - min_x, max_y - min_y);
        if !width.is_finite() || !height.is_finite() {
            return Err(PRECISION);
        }
        Ok(Rect {
            x: min_x,
            y: min_y,
            width,
            height,
        })
    }
}

pub(super) struct Tile {
    ids: Vec<u32>,
    drawing: Drawing,
    bounds: Rect,
}
impl Tile {
    pub(super) fn new(
        ids: Vec<u32>,
        mut drawing: Drawing,
        graph: &Graph,
        work: &mut Work<'_>,
    ) -> LayoutResult<Self> {
        drawing.transform(graph.direction, Point::default());
        let bounds = drawing.bounds(graph, work)?;
        Ok(Self {
            ids,
            drawing,
            bounds,
        })
    }
}

fn shelves(
    tiles: &[Tile],
    gap: f64,
    limit: f64,
    vertical: bool,
    work: &mut Work<'_>,
) -> LayoutResult<(Vec<Point>, Size)> {
    work.charge(tiles.len())?;
    let mut offsets = reserved(tiles.len())?;
    let (mut row, mut col, mut height, mut width) = (0.0_f64, 0.0_f64, 0.0_f64, 0.0_f64);
    for tile in tiles {
        let b = tile.bounds;
        let (w, h) = if vertical {
            (b.height, b.width)
        } else {
            (b.width, b.height)
        };
        if col > 0.0 && col + w > limit {
            row += height + gap;
            col = 0.0;
            height = 0.0;
        }
        let (x, y) = if vertical { (row, col) } else { (col, row) };
        if (w > 0.0 && col + w <= col) || (h > 0.0 && row + h <= row) {
            return Err(PRECISION);
        }
        offsets.push(Point {
            x: x - b.x,
            y: y - b.y,
        });
        width = width.max(col + w);
        height = height.max(h);
        col += w + gap;
    }
    let (w, h) = if vertical {
        (row + height, width)
    } else {
        (width, row + height)
    };
    if !w.is_finite() || !h.is_finite() {
        return Err(PRECISION);
    }
    Ok((
        offsets,
        Size {
            width: w,
            height: h,
        },
    ))
}

pub(super) fn assemble(
    tiles: Vec<Tile>,
    graph: &Graph,
    options: &Options,
    stats: LayoutStats,
    work: &mut Work<'_>,
) -> LayoutResult<LayoutOutput> {
    let gap = options.spacing.component;
    let horizontal = matches!(graph.direction, Direction::Left | Direction::Right);
    let (offsets, bounds) = match options.packing {
        PackingPolicy::Strip => shelves(&tiles, gap, f64::INFINITY, horizontal, work)?,
        PackingPolicy::FitWidth { max_width } => shelves(&tiles, gap, max_width, false, work)?,
        PackingPolicy::AspectTargeted { target_ratio } => {
            let mut area = 0.0;
            let mut widest = 0.0_f64;
            for tile in &tiles {
                work.visit(1)?;
                area += (tile.bounds.width + gap) * (tile.bounds.height + gap);
                widest = widest.max(if horizontal {
                    tile.bounds.height
                } else {
                    tile.bounds.width
                });
            }
            let limit = (area * target_ratio).sqrt().max(widest);
            let wrap = shelves(&tiles, gap, limit, horizontal, work)?;
            let strip = shelves(&tiles, gap, f64::INFINITY, horizontal, work)?;
            let error = |s: Size| {
                let (w, h) = if horizontal {
                    (s.height, s.width)
                } else {
                    (s.width, s.height)
                };
                if w > 0.0 && h > 0.0 {
                    ((w / h).ln() - target_ratio.ln()).abs()
                } else {
                    f64::INFINITY
                }
            };
            if error(strip.1) < error(wrap.1) {
                strip
            } else {
                wrap
            }
        }
    };
    let mut nodes = filled(graph.nodes.len(), Rect::default())?;
    let mut edges = filled(graph.edges.len(), None)?;
    for (mut tile, offset) in tiles.into_iter().zip(offsets) {
        tile.drawing.transform(Direction::Down, offset);
        for (id, r) in tile.ids.into_iter().zip(tile.drawing.nodes) {
            work.visit(1)?;
            if (r.width > 0.0 && r.x + r.width <= r.x) || (r.height > 0.0 && r.y + r.height <= r.y)
            {
                return Err(PRECISION);
            }
            nodes[id as usize] = r;
        }
        for (id, e) in tile.drawing.edges {
            work.visit(e.points.len())?;
            edges[id as usize] = Some(e);
        }
    }
    let mut routes = reserved(edges.len())?;
    for (pending, spec) in edges.into_iter().zip(&graph.edges) {
        let mut route = pending.ok_or(LayoutError::InvalidInput("missing edge occurrence"))?;
        let (Endpoint::Node(from), Endpoint::Node(to)) = (spec.source, spec.target) else {
            return Err(LayoutError::Unsupported("subgraphs"));
        };
        snap_to_faces(&mut route, nodes[from.0 as usize], nodes[to.0 as usize])?;
        routes.push(route);
    }
    Ok(LayoutOutput {
        nodes,
        groups: Vec::new(),
        edges: routes,
        direction: graph.direction,
        bounds,
        stats,
    })
}

/// Perform the final face sum in the same coordinate frame as the returned rectangle. Rotating
/// the knot and the box independently can otherwise differ by one rounding unit.
fn snap_to_faces(edge: &mut EdgeGeometry, source: Rect, target: Rect) -> LayoutResult<()> {
    let [first, .., last] = edge.points.as_mut_slice() else {
        return Err(LayoutError::InvalidInput("missing route endpoints"));
    };
    for (p, side, r) in [(first, edge.ports.0, source), (last, edge.ports.1, target)] {
        match side {
            Side::Top | Side::Bottom => {
                p.x = p.x.clamp(r.x, r.x + r.width);
                p.y = if side == Side::Top {
                    r.y
                } else {
                    r.y + r.height
                };
            }
            Side::Left | Side::Right => {
                p.y = p.y.clamp(r.y, r.y + r.height);
                p.x = if side == Side::Left {
                    r.x
                } else {
                    r.x + r.width
                };
            }
        }
        p.axis = side.axis();
    }
    Ok(())
}
