//! Translate complete components together, including middle/terminal labels and fixed-side detours.

use super::*;
use crate::work::reserved;
use crate::{
    EdgeGeometry, Endpoint, LayoutOutput, LayoutStats, PackingPolicy, Point, Rect, Side,
    WidthPolicy,
};

mod skyline;

const PRECISION: LayoutError = LayoutError::InvalidInput("layout exceeds coordinate precision");
type Placement = (Vec<Point>, Size);

/// Keep a collapsed footprint in the packer's occupied interval even with no padding.
fn reserve_zero_extent(start: f64, padded_end: f64, extent: f64) -> f64 {
    if extent == 0.0 && padded_end == start {
        start.next_up()
    } else {
        padded_end
    }
}

impl Drawing {
    fn transform(&mut self, direction: Direction, origin: Point, at: Point) {
        let point = |x: f64, y: f64| {
            let (x, y) = match direction {
                Direction::Down => (x, y),
                Direction::Up => (x, -y),
                Direction::Right => (y, x),
                Direction::Left => (-y, x),
            };
            Point {
                // Normalize first so a small packing step survives a large local origin.
                x: (x - origin.x) + at.x,
                y: (y - origin.y) + at.y,
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

    fn bounds(
        &self,
        graph: &Graph,
        include_markers: bool,
        work: &mut Work<'_>,
    ) -> LayoutResult<Rect> {
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
            if include_markers {
                for (contact, side, marker) in [
                    (geom.points.first(), geom.ports.0, e.start_marker),
                    (geom.points.last(), geom.ports.1, e.end_marker),
                ] {
                    if marker.length == 0.0 || marker.width == 0.0 {
                        continue;
                    }
                    work.visit(1)?;
                    let contact = contact.ok_or(PRECISION)?;
                    let (x, y) = (contact.x, contact.y);
                    let half = marker.width * 0.5;
                    let [near, far] = match side {
                        Side::Top => [(x - half, y - marker.length), (x + half, y)],
                        Side::Bottom => [(x - half, y), (x + half, y + marker.length)],
                        Side::Left => [(x - marker.length, y - half), (x, y + half)],
                        Side::Right => [(x, y - half), (x + marker.length, y + half)],
                    };
                    include(near.0, near.1)?;
                    include(far.0, far.1)?;
                }
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
    ranks: Vec<u32>,
    drawing: Drawing,
    bounds: Rect,
}
impl Tile {
    pub(super) fn new(
        ids: Vec<u32>,
        ranks: Vec<u32>,
        mut drawing: Drawing,
        graph: &Graph,
        width_policy: WidthPolicy,
        work: &mut Work<'_>,
    ) -> LayoutResult<Self> {
        drawing.transform(graph.direction, Point::default(), Point::default());
        let bounds = drawing.bounds(
            graph,
            matches!(width_policy, WidthPolicy::FitWidth { .. }),
            work,
        )?;
        Ok(Self {
            ids,
            ranks,
            drawing,
            bounds,
        })
    }

    /// Include empty swimlanes in the single tile and return its cross-axis translation.
    pub(super) fn cover_columns(&mut self, span: f64, direction: Direction) -> LayoutResult<f64> {
        let (start, length) = match direction.axis() {
            crate::Axis::Vertical => (&mut self.bounds.x, &mut self.bounds.width),
            crate::Axis::Horizontal => (&mut self.bounds.y, &mut self.bounds.height),
        };
        let end = (*start + *length).max(span);
        *start = start.min(0.0);
        *length = end - *start;
        if !length.is_finite() {
            return Err(PRECISION);
        }
        Ok(-*start)
    }
}

fn shelves(
    tiles: &[Tile],
    gap: f64,
    limit: f64,
    vertical: bool,
    work: &mut Work<'_>,
) -> LayoutResult<Placement> {
    work.charge(tiles.len())?;
    let mut positions = reserved(tiles.len())?;
    let (mut row, mut col, mut height, mut width) = (0.0_f64, 0.0_f64, 0.0_f64, 0.0_f64);
    for tile in tiles {
        let b = tile.bounds;
        let (w, h) = if vertical {
            (b.height, b.width)
        } else {
            (b.width, b.height)
        };
        if col > 0.0 && col + w > limit {
            row = reserve_zero_extent(row, row + (height + gap), height);
            col = 0.0;
            height = 0.0;
        }
        let (x, y) = if vertical { (row, col) } else { (col, row) };
        if (w > 0.0 && col + w <= col) || (h > 0.0 && row + h <= row) {
            return Err(PRECISION);
        }
        positions.push(Point { x, y });
        width = width.max(col + w);
        height = height.max(h);
        col = reserve_zero_extent(col, col + (w + gap), w);
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
        positions,
        Size {
            width: w,
            height: h,
        },
    ))
}

fn aspect_measure(
    tiles: &[Tile],
    gap: f64,
    vertical: bool,
    work: &mut Work<'_>,
) -> LayoutResult<(f64, f64)> {
    let mut area = 0.0;
    let mut widest = 0.0_f64;
    for tile in tiles {
        work.visit(1)?;
        let b = tile.bounds;
        area += (b.width + gap) * (b.height + gap);
        widest = widest.max(if vertical { b.height } else { b.width });
    }
    Ok((area, widest))
}

fn aspect_shelves(
    tiles: &[Tile],
    gap: f64,
    limit: f64,
    vertical: bool,
    log_cross_ratio: f64,
    work: &mut Work<'_>,
) -> LayoutResult<Placement> {
    let wrap = shelves(tiles, gap, limit, vertical, work)?;
    let strip = shelves(tiles, gap, f64::INFINITY, vertical, work)?;
    let error = |s: Size| {
        let (w, h) = if vertical {
            (s.height, s.width)
        } else {
            (s.width, s.height)
        };
        if w > 0.0 && h > 0.0 {
            ((w / h).ln() - log_cross_ratio).abs()
        } else {
            f64::INFINITY
        }
    };
    Ok(if error(strip.1) < error(wrap.1) {
        strip
    } else {
        wrap
    })
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
    let (positions, bounds) = match options.packing {
        PackingPolicy::Strip => shelves(&tiles, gap, f64::INFINITY, horizontal, work)?,
        PackingPolicy::FitWidth { max_width } => shelves(&tiles, gap, max_width, false, work)?,
        PackingPolicy::AspectTargeted { target_ratio } => {
            let (area, widest) = aspect_measure(&tiles, gap, horizontal, work)?;
            let limit = (area * target_ratio).sqrt().max(widest);
            aspect_shelves(&tiles, gap, limit, horizontal, target_ratio.ln(), work)?
        }
        PackingPolicy::BalancedAspect { target_ratio } => {
            skyline::balanced(&tiles, gap, target_ratio, horizontal, work)?
        }
    };
    let mut nodes = filled(graph.nodes.len(), Rect::default())?;
    let mut node_ranks = if options.capture_node_ranks {
        work.charge(graph.nodes.len())?;
        filled(graph.nodes.len(), 0)?
    } else {
        Vec::new()
    };
    let mut edges = filled(graph.edges.len(), None)?;
    for (mut tile, at) in tiles.into_iter().zip(positions) {
        let origin = Point {
            x: tile.bounds.x,
            y: tile.bounds.y,
        };
        tile.drawing.transform(Direction::Down, origin, at);
        for (&id, rank) in tile.ids.iter().zip(tile.ranks) {
            work.visit(1)?;
            node_ranks[id as usize] = rank;
        }
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
        node_ranks,
        groups: Vec::new(),
        edges: routes,
        truncated_routes: Vec::new(),
        border_stitches: Vec::new(),
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{NodeSpec, NoopWorkControl, Spacing, layout};

    #[test]
    fn width_limited_shelves_advance_past_zero_height_rows() {
        let size = Size {
            width: 20.0,
            height: 0.0,
        };
        let graph = Graph {
            nodes: (0..3).map(|id| NodeSpec::new(size, id)).collect(),
            ..Default::default()
        };
        let options = Options {
            spacing: Spacing {
                component: 0.0,
                ..Default::default()
            },
            packing: PackingPolicy::FitWidth {
                max_width: size.width,
            },
            ..Default::default()
        };
        let out = layout(&graph, &options).unwrap();
        assert!(out.nodes.windows(2).all(|pair| pair[0].y < pair[1].y));
        assert!(
            out.nodes
                .iter()
                .all(|node| node.x == 0.0 && node.height == 0.0)
        );
        assert_eq!(out.bounds.width, size.width);
        assert_eq!(out.bounds.height, out.nodes[2].y);
    }

    #[test]
    fn zero_width_shelf_tiles_keep_distinct_origins_after_translation() {
        let options = Options {
            spacing: Spacing {
                component: 0.0,
                ..Default::default()
            },
            packing: PackingPolicy::Strip,
            ..Default::default()
        };
        let size = Size {
            width: 0.0,
            height: 20.0,
        };
        let graph = Graph {
            nodes: (0..3).map(|id| NodeSpec::new(size, id)).collect(),
            ..Default::default()
        };
        let mut control = NoopWorkControl;
        let mut work = Work::new(&mut control);
        let mut tiles = Vec::new();
        for id in 0..3 {
            let origin = f64::from(id) * 2.0 - 10.0;
            let bounds = Rect {
                x: origin,
                y: -15.0,
                width: size.width,
                height: size.height,
            };
            tiles.push(Tile {
                ids: vec![id],
                ranks: Vec::new(),
                drawing: Drawing {
                    nodes: vec![bounds],
                    edges: Vec::new(),
                },
                bounds,
            });
        }
        let out = assemble(tiles, &graph, &options, LayoutStats::default(), &mut work).unwrap();
        assert!(out.nodes.windows(2).all(|pair| pair[0].x < pair[1].x));
        assert!(
            out.nodes
                .iter()
                .all(|node| node.y == 0.0 && node.width == 0.0)
        );
        assert_eq!(out.bounds.width, out.nodes[2].x);
        assert_eq!(out.bounds.height, size.height);
    }
}
