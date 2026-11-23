use super::FlatGraph;
use super::layers::Layers;
use super::routing::Track;
use crate::scratch::Buffers;
use crate::work::{LayoutResult as Result, Work, filled, reserved};
use crate::{
    Axis, Direction, EdgeGeometry, LayoutError, LayoutOutput, LayoutStats, Point, Rect, RoutePoint,
    Side, Size, WorkError,
};
use std::ops::Range;

const PRECISION: LayoutError =
    LayoutError::InvalidInput("measurements exceed coordinate precision");

pub(crate) fn finite(value: f64) -> Result<f64> {
    if value.is_finite() {
        Ok(value)
    } else {
        Err(WorkError::ArithmeticOverflow.into())
    }
}

/// A positive marker approach or lane separation must survive coordinate rounding.
pub(crate) fn advance(from: f64, distance: f64) -> Result<f64> {
    let to = finite(from + distance)?;
    if distance > 0.0 && to <= from {
        Err(PRECISION)
    } else if to - from < distance {
        finite(to.next_up())
    } else {
        Ok(to)
    }
}

impl Direction {
    /// In the kernel x is across the ranks and y advances from sources towards sinks.
    pub(crate) fn axes(self, size: Size) -> Size {
        match self {
            Self::Down | Self::Up => size,
            Self::Right | Self::Left => Size {
                width: size.height,
                height: size.width,
            },
        }
    }
}

#[derive(Clone, Copy, Default)]
pub(crate) struct Extent {
    pub(crate) min: f64,
    pub(crate) max: f64,
}

impl Extent {
    pub(crate) fn include(&mut self, center: f64, half: f64) -> Result<()> {
        self.min = self.min.min(finite(center - half)?);
        self.max = self.max.max(finite(center + half)?);
        Ok(())
    }
}

#[derive(Clone, Copy, Default)]
pub(crate) struct RowSpace {
    height: f64,
    pub(crate) start_clearance: f64,
    pub(crate) end_clearance: f64,
    /// Maximum label depth in the gap following this row.
    pub(crate) label_height: f64,
    /// Shared shoulders beyond the row's facing sides and marker setbacks.
    pub(crate) departure: f64,
    pub(crate) arrival: f64,
    /// All routes run parallel to the rank axis through this band.
    pub(crate) band_top: f64,
    pub(crate) band_bottom: f64,
}

pub(crate) struct Drawing<'a> {
    pub(crate) graph: &'a FlatGraph<'a>,
    pub(crate) layers: Layers,
    pub(crate) centers: Vec<Point>,
    pub(crate) space: Vec<RowSpace>,
    pub(crate) tracks: Vec<Track>,
}

impl Drawing<'_> {
    fn place_across(&mut self, rows: Range<usize>, work: &mut Work) -> Result<Extent> {
        let mut extent = Extent::default();
        for r in rows {
            let nodes = &self.layers.order[self.layers.rows[r].nodes.clone()];
            let mut width = 0.0;
            for (i, &node) in nodes.iter().enumerate() {
                work.visit(1)?;
                let size = self.graph.direction.axes(self.graph.nodes[node].size);
                if i > 0 {
                    width = advance(width, self.graph.options.spacing.node)?;
                }
                width = advance(width, size.width)?;
                self.space[r].height = self.space[r].height.max(size.height);
            }
            let mut left = -width * 0.5;
            for (i, &node) in nodes.iter().enumerate() {
                work.visit(1)?;
                if i > 0 {
                    left = advance(left, self.graph.options.spacing.node)?;
                }
                let width = self.graph.direction.axes(self.graph.nodes[node].size).width;
                let half = width * 0.5;
                self.centers[node].x = advance(left, half)?;
                extent.include(self.centers[node].x, half)?;
                left = advance(left, width)?;
            }
        }
        Ok(extent)
    }

    fn place_depth(&mut self, rows: Range<usize>, work: &mut Work) -> Result<()> {
        let mut top = 0.0;
        for r in rows.clone() {
            let height = self.space[r].height;
            let center = advance(top, height * 0.5)?;
            if r > rows.start {
                let before = (center - height * 0.5) - self.space[r].arrival;
                if before < self.space[r].end_clearance
                    || (self.graph.options.spacing.edge > 0.0 && before <= 0.0)
                {
                    return Err(PRECISION);
                }
            }
            for &node in &self.layers.order[self.layers.rows[r].nodes.clone()] {
                work.visit(1)?;
                self.centers[node].y = center;
            }
            if r + 1 == rows.end {
                break;
            }
            let bottom = finite(center + height * 0.5)?;
            let steps = self.layers.rows[r + 1].rank - self.layers.rows[r].rank;
            let requested = finite(self.graph.options.spacing.rank * f64::from(steps))?;
            let labels = self.space[r].label_height;
            let mut gap = self.graph.options.spacing.edge;
            if gap == 0.0 {
                // Share any spare rank space between the two shafts and the two ramps.
                gap = (requested
                    - self.space[r].start_clearance
                    - self.space[r + 1].end_clearance
                    - labels)
                    .max(0.0)
                    / 4.0;
            }
            let start = advance(self.space[r].start_clearance, gap)?;
            let end = advance(self.space[r + 1].end_clearance, gap)?;
            let required = finite(start + end + labels + 2.0 * gap)?;
            let ramp = gap + (requested - required).max(0.0) * 0.5;

            let departure = advance(bottom, start)?;
            let band_top = advance(departure, ramp)?;
            let band_bottom = advance(band_top, labels)?;
            let arrival = advance(band_bottom, ramp)?;
            top = advance(arrival, end)?;
            self.space[r].departure = departure;
            self.space[r].band_top = band_top;
            self.space[r].band_bottom = band_bottom;
            self.space[r + 1].arrival = arrival;
        }
        Ok(())
    }
}

#[derive(Clone, Copy)]
struct Frame {
    left: f64,
    start: f64,
    direction: Direction,
    gap: f64,
}

impl Frame {
    fn map(self, p: Point) -> Result<Point> {
        let x = advance(self.start, finite(p.x - self.left)?)?;
        // A translation that loses half a track gutter can merge two parallel routes.
        let recovered = (x - self.start) + self.left;
        if self.gap > 0.0 && (p.x - recovered).abs() >= self.gap * 0.5 && p.x != recovered {
            return Err(PRECISION);
        }
        let y = p.y;
        Ok(match self.direction {
            Direction::Down => Point { x, y },
            Direction::Up => Point { x, y: -y },
            Direction::Right => Point { x: y, y: x },
            Direction::Left => Point { x: -y, y: x },
        })
    }
}

#[derive(Clone, Copy)]
struct Bounds {
    min_x: f64,
    min_y: f64,
    max_x: f64,
    max_y: f64,
}

impl Default for Bounds {
    fn default() -> Self {
        Self {
            min_x: f64::INFINITY,
            min_y: f64::INFINITY,
            max_x: f64::NEG_INFINITY,
            max_y: f64::NEG_INFINITY,
        }
    }
}

impl Bounds {
    fn include(&mut self, p: Point, size: Size) -> Result<()> {
        let x0 = finite(p.x - size.width * 0.5)?;
        let x1 = finite(p.x + size.width * 0.5)?;
        let y0 = finite(p.y - size.height * 0.5)?;
        let y1 = finite(p.y + size.height * 0.5)?;
        if (size.width > 0.0 && (x0 >= p.x || x1 <= p.x))
            || (size.height > 0.0 && (y0 >= p.y || y1 <= p.y))
        {
            return Err(PRECISION);
        }
        self.min_x = self.min_x.min(x0);
        self.max_x = self.max_x.max(x1);
        self.min_y = self.min_y.min(y0);
        self.max_y = self.max_y.max(y1);
        Ok(())
    }
}

pub(super) fn run(
    graph: &FlatGraph,
    buffers: &mut Buffers,
    work: &mut Work,
) -> Result<LayoutOutput> {
    if graph.nodes.is_empty() {
        return Ok(LayoutOutput {
            direction: graph.direction,
            ..LayoutOutput::default()
        });
    }
    let layers = Layers::build(graph, buffers, work)?;
    let components = layers.components.len();
    work.charge(
        graph
            .nodes
            .len()
            .checked_add(graph.edges.len())
            .ok_or(WorkError::ArithmeticOverflow)?,
    )?;
    let mut drawing = Drawing {
        graph,
        space: filled(layers.rows.len(), RowSpace::default())?,
        layers,
        centers: filled(graph.nodes.len(), Point::default())?,
        tracks: filled(graph.edges.len(), Track::default())?,
    };
    let mut edges = filled(
        graph.edges.len(),
        EdgeGeometry {
            points: Vec::new(),
            ports: graph.direction.ports(),
            label_center: None,
            start_label_center: None,
            end_label_center: None,
            reversed_for_ranking: false,
        },
    )?;
    let mut bounds = Bounds::default();
    let mut start = 0.0;
    for i in 0..drawing.layers.components.len() {
        let rows = drawing.layers.components[i].clone();
        let mut extent = drawing.place_across(rows.clone(), work)?;
        drawing.plan_tracks(rows.clone(), &mut extent, work)?;
        drawing.place_depth(rows.clone(), work)?;
        drawing.assign_ports(rows.clone(), work)?;
        let frame = Frame {
            left: extent.min,
            start,
            direction: graph.direction,
            gap: graph.options.spacing.edge,
        };
        for row in &drawing.layers.rows[rows] {
            for &node in &drawing.layers.order[row.nodes.clone()] {
                work.visit(1)?;
                // Targets lie in later rows and still have their local coordinates.
                for &id in drawing.layers.outgoing.at(node) {
                    work.visit(1)?;
                    let mut route = drawing.route(id)?;
                    work.visit(route.points.len())?;
                    for p in &mut route.points {
                        let mapped = frame.map(Point { x: p.x, y: p.y })?;
                        p.x = mapped.x;
                        p.y = mapped.y;
                        if matches!(graph.direction, Direction::Right | Direction::Left) {
                            p.axis = match p.axis {
                                Axis::Vertical => Axis::Horizontal,
                                Axis::Horizontal => Axis::Vertical,
                            };
                        }
                        bounds.include(mapped, Size::default())?;
                    }
                    if let (Some(p), Some(size)) =
                        (&mut route.label_center, graph.edges[id].spec.label)
                    {
                        *p = frame.map(*p)?;
                        bounds.include(*p, size)?;
                    }
                    edges[id] = route;
                }
                let center = frame.map(drawing.centers[node])?;
                drawing.centers[node] = center;
                bounds.include(center, graph.nodes[node].size)?;
            }
        }
        if i + 1 < drawing.layers.components.len() {
            let end = match graph.direction {
                Direction::Down | Direction::Up => bounds.max_x,
                Direction::Right | Direction::Left => bounds.max_y,
            };
            start = advance(end, graph.options.spacing.component)?;
        }
    }
    normalize(graph, drawing.centers, edges, bounds, components, work)
}

/// Translate boxes, labels and routes together, including the negative depth in Up/Left.
fn normalize(
    graph: &FlatGraph,
    centers: Vec<Point>,
    mut edges: Vec<EdgeGeometry>,
    bounds: Bounds,
    components: usize,
    work: &mut Work,
) -> Result<LayoutOutput> {
    let origin = Point {
        x: bounds.min_x,
        y: bounds.min_y,
    };
    let mut extent = Size::default();
    work.charge(graph.nodes.len())?;
    let mut nodes = reserved(graph.nodes.len())?;
    for (center, node) in centers.into_iter().zip(graph.nodes) {
        work.visit(1)?;
        let rect = normalized_rect(center, node.size, origin)?;
        extent.width = extent.width.max(rect.x + rect.width);
        extent.height = extent.height.max(rect.y + rect.height);
        nodes.push(rect);
    }
    for (edge, geometry) in graph.edges.iter().zip(&mut edges) {
        work.visit(geometry.points.len() + 1)?;
        for p in &mut geometry.points {
            p.x = finite(p.x - origin.x)?;
            p.y = finite(p.y - origin.y)?;
        }
        let [first, departure, .., arrival, last] = geometry.points.as_mut_slice() else {
            unreachable!("the preview emits two terminal pairs")
        };
        snap_terminal(
            nodes[edge.source],
            first,
            departure,
            geometry.ports.0,
            edge.spec.start_marker.length,
            graph.options.spacing.edge,
        )?;
        snap_terminal(
            nodes[edge.target],
            last,
            arrival,
            geometry.ports.1,
            edge.spec.end_marker.length,
            graph.options.spacing.edge,
        )?;
        for p in &geometry.points {
            extent.width = extent.width.max(p.x);
            extent.height = extent.height.max(p.y);
        }
        if let (Some(center), Some(size)) = (&mut geometry.label_center, edge.spec.label) {
            *center = normalized_rect(*center, size, origin)?.center();
            extent.width = extent.width.max(finite(center.x + size.width * 0.5)?);
            extent.height = extent.height.max(finite(center.y + size.height * 0.5)?);
        }
    }
    Ok(LayoutOutput {
        nodes,
        edges,
        groups: Vec::new(),
        bounds: extent,
        direction: graph.direction,
        stats: LayoutStats {
            components,
            ..LayoutStats::default()
        },
    })
}

fn normalized_rect(center: Point, size: Size, origin: Point) -> Result<Rect> {
    let rect = Rect {
        x: finite((center.x - size.width * 0.5) - origin.x)?,
        y: finite((center.y - size.height * 0.5) - origin.y)?,
        width: size.width,
        height: size.height,
    };
    let right = finite(rect.x + rect.width)?;
    let bottom = finite(rect.y + rect.height)?;
    let center = rect.center();
    if (rect.width > 0.0 && (center.x <= rect.x || center.x >= right))
        || (rect.height > 0.0 && (center.y <= rect.y || center.y >= bottom))
    {
        return Err(PRECISION);
    }
    Ok(rect)
}

/// Compute the contact from the returned rectangle and keep the shaft on its face normal.
fn snap_terminal(
    rect: Rect,
    p: &mut RoutePoint,
    shoulder: &mut RoutePoint,
    side: Side,
    marker: f64,
    gap: f64,
) -> Result<()> {
    let right = rect.x + rect.width;
    let bottom = rect.y + rect.height;
    let room = match side {
        Side::Top | Side::Bottom => {
            p.x = p.x.clamp(rect.x, right);
            shoulder.x = p.x;
            if side == Side::Top {
                p.y = rect.y;
                p.y - shoulder.y
            } else {
                p.y = bottom;
                shoulder.y - p.y
            }
        }
        Side::Left | Side::Right => {
            p.y = p.y.clamp(rect.y, bottom);
            shoulder.y = p.y;
            if side == Side::Left {
                p.x = rect.x;
                p.x - shoulder.x
            } else {
                p.x = right;
                shoulder.x - p.x
            }
        }
    };
    if room < marker || (gap > 0.0 && room == 0.0) {
        return Err(PRECISION);
    }
    Ok(())
}
