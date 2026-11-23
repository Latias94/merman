use crate::layers::Layers;
use crate::routing::Track;
use crate::work::{Result, Work, filled};
use crate::{Bounds, Direction, Graph, Layout, LayoutError, Point, Route, Size};
use std::ops::Range;

/// Space for distinct tracks and a positive approach even with zero caller spacing.
pub(crate) const GUTTER: f64 = 4.0;

pub(crate) fn finite<E>(value: f64) -> Result<f64, E> {
    if value.is_finite() {
        Ok(value)
    } else {
        Err(LayoutError::Overflow)
    }
}

/// A positive marker approach or lane separation must survive coordinate rounding.
pub(crate) fn advance<E>(from: f64, distance: f64) -> Result<f64, E> {
    let to = finite(from + distance)?;
    if distance > 0.0 && to <= from {
        Err(LayoutError::Overflow)
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

impl Size {
    fn valid(self) -> bool {
        valid_length(self.width) && valid_length(self.height)
    }
}

fn valid_length(length: f64) -> bool {
    length.is_finite() && length >= 0.0
}

fn validate<E>(graph: &Graph, work: &mut Work<E>) -> Result<(), E> {
    if !valid_length(graph.node_spacing) || !valid_length(graph.rank_spacing) {
        return Err(LayoutError::InvalidInput(
            "spacing must be finite and nonnegative",
        ));
    }
    for node in &graph.nodes {
        work.visit(1)?;
        if !node.valid() {
            return Err(LayoutError::InvalidInput(
                "node dimensions must be finite and nonnegative",
            ));
        }
    }
    for edge in &graph.edges {
        work.visit(1)?;
        if edge.source.0 >= graph.nodes.len() || edge.target.0 >= graph.nodes.len() {
            return Err(LayoutError::InvalidInput(
                "edge endpoint is outside the node array",
            ));
        }
        if edge.label.is_some_and(|size| !size.valid()) {
            return Err(LayoutError::InvalidInput(
                "label dimensions must be finite and nonnegative",
            ));
        }
        if !valid_length(edge.start_clearance) || !valid_length(edge.end_clearance) {
            return Err(LayoutError::InvalidInput(
                "marker clearance must be finite and nonnegative",
            ));
        }
        if edge.source == edge.target {
            return Err(LayoutError::Unsupported("self-loops"));
        }
    }
    Ok(())
}

#[derive(Clone, Copy, Default)]
pub(crate) struct Extent {
    pub(crate) min: f64,
    pub(crate) max: f64,
}

impl Extent {
    pub(crate) fn include<E>(&mut self, center: f64, half: f64) -> Result<(), E> {
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
    pub(crate) graph: &'a Graph,
    pub(crate) layers: Layers,
    pub(crate) centers: Vec<Point>,
    pub(crate) space: Vec<RowSpace>,
    pub(crate) tracks: Vec<Track>,
}

impl Drawing<'_> {
    fn place_across<E>(&mut self, rows: Range<usize>, work: &mut Work<E>) -> Result<Extent, E> {
        let mut extent = Extent::default();
        for r in rows {
            let nodes = &self.layers.order[self.layers.rows[r].nodes.clone()];
            let mut width = 0.0;
            for (i, &node) in nodes.iter().enumerate() {
                work.visit(1)?;
                let size = self.graph.direction.axes(self.graph.nodes[node]);
                if i > 0 {
                    width = advance(width, self.graph.node_spacing)?;
                }
                width = advance(width, size.width)?;
                self.space[r].height = self.space[r].height.max(size.height);
            }
            let mut left = -width * 0.5;
            for (i, &node) in nodes.iter().enumerate() {
                work.visit(1)?;
                if i > 0 {
                    left = advance(left, self.graph.node_spacing)?;
                }
                let width = self.graph.direction.axes(self.graph.nodes[node]).width;
                let half = width * 0.5;
                self.centers[node].x = advance(left, half)?;
                extent.include(self.centers[node].x, half)?;
                left = advance(left, width)?;
            }
        }
        Ok(extent)
    }

    fn place_depth<E>(&mut self, rows: Range<usize>, work: &mut Work<E>) -> Result<(), E> {
        let mut top = 0.0;
        for r in rows.clone() {
            let height = self.space[r].height;
            let center = advance(top, height * 0.5)?;
            if r > rows.start {
                let before = (center - height * 0.5) - self.space[r].arrival;
                if before <= 0.0 || before < self.space[r].end_clearance {
                    return Err(LayoutError::Overflow);
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
            let requested = finite(self.graph.rank_spacing * steps as f64)?;
            let start = advance(self.space[r].start_clearance, GUTTER)?;
            let end = advance(self.space[r + 1].end_clearance, GUTTER)?;
            let labels = self.space[r].label_height;
            let required = finite(start + end + labels + 2.0 * GUTTER)?;
            let ramp = GUTTER + (requested - required).max(0.0) * 0.5;

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
}

impl Frame {
    fn map<E>(self, p: Point) -> Result<Point, E> {
        let x = advance(self.start, finite(p.x - self.left)?)?;
        // A translation that loses half a track gutter can merge two parallel routes.
        let recovered = (x - self.start) + self.left;
        if (p.x - recovered).abs() >= GUTTER * 0.5 {
            return Err(LayoutError::Overflow);
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

impl Bounds {
    fn include<E>(&mut self, p: Point, size: Size) -> Result<(), E> {
        let x0 = finite(p.x - size.width * 0.5)?;
        let x1 = finite(p.x + size.width * 0.5)?;
        let y0 = finite(p.y - size.height * 0.5)?;
        let y1 = finite(p.y + size.height * 0.5)?;
        if (size.width > 0.0 && (x0 >= p.x || x1 <= p.x))
            || (size.height > 0.0 && (y0 >= p.y || y1 <= p.y))
        {
            return Err(LayoutError::Overflow);
        }
        self.min_x = self.min_x.min(x0);
        self.max_x = self.max_x.max(x1);
        self.min_y = self.min_y.min(y0);
        self.max_y = self.max_y.max(y1);
        Ok(())
    }
}

pub(crate) fn run<E>(
    graph: &Graph,
    mut callback: impl FnMut(usize) -> std::result::Result<(), E>,
) -> Result<Layout, E> {
    let mut work = Work::new(&mut callback);
    work.charge(1)?;
    validate(graph, &mut work)?;
    if graph.nodes.is_empty() {
        return Ok(Layout::default());
    }
    let layers = Layers::build(graph, &mut work)?;
    work.charge(
        graph
            .nodes
            .len()
            .checked_add(graph.edges.len())
            .ok_or(LayoutError::Overflow)?,
    )?;
    let mut drawing = Drawing {
        graph,
        space: filled(layers.rows.len(), RowSpace::default())?,
        layers,
        centers: filled(graph.nodes.len(), Point::default())?,
        tracks: filled(graph.edges.len(), Track::default())?,
    };
    let mut edges = filled(graph.edges.len(), Route::default())?;
    let mut bounds = Bounds::default();
    let mut start = 0.0;
    for i in 0..drawing.layers.components.len() {
        let rows = drawing.layers.components[i].clone();
        let mut extent = drawing.place_across(rows.clone(), &mut work)?;
        drawing.plan_tracks(rows.clone(), &mut extent, &mut work)?;
        drawing.place_depth(rows.clone(), &mut work)?;
        drawing.assign_ports(rows.clone(), &mut work)?;
        let frame = Frame {
            left: extent.min,
            start,
            direction: graph.direction,
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
                        *p = frame.map(*p)?;
                        bounds.include(*p, Size::default())?;
                    }
                    if let (Some(p), Some(size)) = (&mut route.label_center, graph.edges[id].label)
                    {
                        *p = frame.map(*p)?;
                        bounds.include(*p, size)?;
                    }
                    edges[id] = route;
                }
                let center = frame.map(drawing.centers[node])?;
                drawing.centers[node] = center;
                bounds.include(center, graph.nodes[node])?;
            }
        }
        if i + 1 < drawing.layers.components.len() {
            let end = match graph.direction {
                Direction::Down | Direction::Up => bounds.max_x,
                Direction::Right | Direction::Left => bounds.max_y,
            };
            start = advance(end, graph.node_spacing)?;
        }
    }
    work.charge(0)?;
    Ok(Layout {
        centers: drawing.centers,
        edges,
        bounds,
    })
}
