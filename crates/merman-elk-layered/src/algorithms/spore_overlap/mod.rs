//! SPOrE overlap removal from Eclipse ELK 0.9.1, EPL-2.0.
//!
//! Source commit 62d5909f96fad541bc101ad52dabaece6b7eab7e: OverlapRemovalLayoutProvider,
//! ElkGraphImporter, GrowTreePhase, NaiveMinST, BowyerWatsonTriangulation and ScanlineOverlapCheck.
//! Measured rectangles include caller-computed margins. Underlying layout, compound scheduling,
//! node resizing constraints and label translation belong to the adapter. The source uses global
//! Math.random only to separate duplicate centers; callers supply that operation-owned stream.

use super::box_layout::Padding;
pub use super::force::Point;
use crate::work::{WorkControl, WorkError, checked_add, checked_mul};
mod triangulation;

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Node {
    pub x: f64,
    pub y: f64,
    pub width: f64,
    pub height: f64,
    pub margin: Padding,
}
impl Default for Node {
    fn default() -> Self {
        Self {
            x: 0.,
            y: 0.,
            width: 0.,
            height: 0.,
            margin: Padding {
                top: 0.,
                right: 0.,
                bottom: 0.,
                left: 0.,
            },
        }
    }
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Edge {
    pub source: usize,
    pub target: usize,
}
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Options {
    pub spacing: f64,
    pub padding: Padding,
    /// Center of the parent's original rectangle, before overlap removal.
    pub parent_center: Point,
    pub max_iterations: usize,
    pub scanline: bool,
}
impl Default for Options {
    fn default() -> Self {
        Self {
            spacing: 8.,
            padding: Padding {
                top: 8.,
                right: 8.,
                bottom: 8.,
                left: 8.,
            },
            parent_center: Point::default(),
            max_iterations: 64,
            scanline: true,
        }
    }
}
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Layout {
    pub nodes: Vec<Point>,
    pub edges: Vec<[Point; 2]>,
    pub width: f64,
    pub height: f64,
    /// Source's final ElkUtil.translate offset, also applied to existing contained labels.
    pub translation: Point,
}
#[derive(Debug, thiserror::Error, PartialEq)]
pub enum Error {
    #[error(transparent)]
    Work(#[from] WorkError),
    #[error("SPOrE received invalid rectangle {0}")]
    InvalidNode(usize),
    #[error("SPOrE received invalid edge {0}")]
    InvalidEdge(usize),
    #[error("SPOrE received invalid layout options")]
    InvalidOptions,
    #[error("SPOrE received randomness outside [0, 1)")]
    InvalidRandomness,
    #[error("SPOrE geometry exceeded its finite numeric range")]
    NonFiniteGeometry,
    #[error("SPOrE jitter cannot separate coincident coordinates at their numeric precision")]
    NumericStagnation,
}
#[derive(Clone, Copy)]
struct Rectangle {
    x: f64,
    y: f64,
    width: f64,
    height: f64,
}
impl Rectangle {
    fn center(self) -> Point {
        Point {
            x: self.x + self.width / 2.,
            y: self.y + self.height / 2.,
        }
    }
}
#[derive(Clone, Copy)]
struct Vertex {
    original: Point,
    position: Point,
    rect: Rectangle,
}
impl Vertex {
    fn translate(&mut self, delta: Point) {
        self.position.x += delta.x;
        self.position.y += delta.y;
        self.rect.x += delta.x;
        self.rect.y += delta.y;
    }
}
fn finite(p: Point) -> bool {
    p.x.is_finite() && p.y.is_finite()
}
fn distance(a: Point, b: Point) -> f64 {
    ((a.x - b.x).powi(2) + (a.y - b.y).powi(2)).sqrt()
}
fn charge(work: &mut dyn WorkControl, units: usize) -> Result<(), Error> {
    work.check(units)?;
    work.charge(units)?;
    Ok(())
}
fn fuzzy(a: f64, b: f64, epsilon: f64) -> std::cmp::Ordering {
    if a.is_nan() {
        return if b.is_nan() {
            std::cmp::Ordering::Equal
        } else {
            std::cmp::Ordering::Greater
        };
    }
    if b.is_nan() {
        return std::cmp::Ordering::Less;
    }
    if (a - b).abs() <= epsilon {
        std::cmp::Ordering::Equal
    } else {
        a.total_cmp(&b)
    }
}

pub fn layout(
    nodes: &[Node],
    edges: &[Edge],
    options: &Options,
    random: &mut dyn FnMut() -> f64,
    work: &mut dyn WorkControl,
) -> Result<Layout, Error> {
    charge(work, checked_add(nodes.len(), edges.len())?)?;
    if !options.spacing.is_finite()
        || options.spacing < 0.
        || !finite(options.parent_center)
        || options.max_iterations > i32::MAX as usize
        || [
            options.padding.top,
            options.padding.right,
            options.padding.bottom,
            options.padding.left,
        ]
        .iter()
        .any(|v| !v.is_finite() || *v < 0.)
    {
        return Err(Error::InvalidOptions);
    }
    for (i, n) in nodes.iter().enumerate() {
        if !finite(Point { x: n.x, y: n.y })
            || [
                n.width,
                n.height,
                n.margin.top,
                n.margin.right,
                n.margin.bottom,
                n.margin.left,
            ]
            .iter()
            .any(|v| !v.is_finite() || *v < 0.)
        {
            return Err(Error::InvalidNode(i));
        }
    }
    for (i, e) in edges.iter().enumerate() {
        if e.source >= nodes.len() || e.target >= nodes.len() {
            return Err(Error::InvalidEdge(i));
        }
    }
    if nodes.is_empty() {
        return Ok(Layout::default());
    }
    let mut vertices: Vec<Vertex> = Vec::with_capacity(nodes.len());
    for node in nodes {
        let mut position = Point {
            x: node.x + node.width / 2.,
            y: node.y + node.height / 2.,
        };
        if !finite(position) {
            return Err(Error::NonFiniteGeometry);
        }
        loop {
            charge(work, checked_add(vertices.len(), 1)?)?;
            if !vertices.iter().any(|v| v.original == position) {
                break;
            }
            let old = position;
            let x = random();
            let y = random();
            if !(0.0..1.0).contains(&x) || !(0.0..1.0).contains(&y) {
                return Err(Error::InvalidRandomness);
            }
            position.x += (x - 0.5) * 0.001;
            position.y += (y - 0.5) * 0.001;
            if position == old {
                return Err(Error::NumericStagnation);
            }
        }
        let rect = Rectangle {
            x: position.x - node.width / 2. - options.spacing / 2. - node.margin.left,
            y: position.y - node.height / 2. - options.spacing / 2. - node.margin.top,
            width: node.width + options.spacing + node.margin.left + node.margin.right,
            height: node.height + options.spacing + node.margin.top + node.margin.bottom,
        };
        if !finite(rect.center()) || !rect.width.is_finite() || !rect.height.is_finite() {
            return Err(Error::NonFiniteGeometry);
        }
        vertices.push(Vertex {
            original: position,
            position,
            rect,
        });
    }
    let root = vertices
        .iter()
        .enumerate()
        .min_by(|(_, a), (_, b)| {
            distance(a.original, options.parent_center)
                .total_cmp(&distance(b.original, options.parent_center))
        })
        .map(|(i, _)| i)
        .unwrap_or(0);
    for _ in 0..options.max_iterations {
        let mut links = if options.scanline {
            overlaps(&vertices, work)?
        } else {
            Vec::new()
        };
        if options.scanline && links.is_empty() {
            break;
        }
        triangulation::triangulate(&vertices, &mut links, work)?;
        let parents = spanning_tree(&vertices, &links, root, work)?;
        let mut changed = false;
        // Prim's accepted-edge order always places a parent before its children. Every
        // operation here depends only on that parent, so it equals the source DFS traversal.
        for (parent, child) in parents {
            charge(work, 1)?;
            let p = vertices[parent];
            vertices[child].translate(Point {
                x: p.position.x - p.original.x,
                y: p.position.y - p.original.y,
            });
            let t = overlap(p.rect, vertices[child].rect);
            changed |= t > 1.;
            let c = vertices[child];
            let target = Point {
                x: p.position.x + (c.original.x - p.original.x) * t,
                y: p.position.y + (c.original.y - p.original.y) * t,
            };
            if !finite(target) {
                return Err(Error::NonFiniteGeometry);
            }
            vertices[child].translate(Point {
                x: target.x - c.position.x,
                y: target.y - c.position.y,
            });
        }
        for vertex in &mut vertices {
            vertex.original = vertex.rect.center();
        }
        if !changed {
            break;
        }
    }
    let min_x = vertices
        .iter()
        .map(|v| v.rect.x)
        .fold(f64::INFINITY, f64::min);
    let min_y = vertices
        .iter()
        .map(|v| v.rect.y)
        .fold(f64::INFINITY, f64::min);
    let max_x = vertices
        .iter()
        .zip(nodes)
        .map(|(v, n)| v.rect.x + n.width)
        .fold(f64::NEG_INFINITY, f64::max);
    let max_y = vertices
        .iter()
        .zip(nodes)
        .map(|(v, n)| v.rect.y + n.height)
        .fold(f64::NEG_INFINITY, f64::max);
    let translation = Point {
        x: options.padding.left - min_x,
        y: options.padding.top - min_y,
    };
    let mut output = Layout {
        nodes: vertices
            .iter()
            .map(|v| Point {
                x: v.rect.x + translation.x,
                y: v.rect.y + translation.y,
            })
            .collect(),
        edges: Vec::with_capacity(edges.len()),
        width: max_x - min_x + options.padding.left + options.padding.right,
        height: max_y - min_y + options.padding.top + options.padding.bottom,
        translation,
    };
    for edge in edges {
        charge(work, 1)?;
        let center = |index: usize| Point {
            x: output.nodes[index].x + nodes[index].width / 2.,
            y: output.nodes[index].y + nodes[index].height / 2.,
        };
        let a = center(edge.source);
        let b = center(edge.target);
        let start = clipped(a, b, nodes[edge.source]);
        // Source clips the target toward the already clipped start, not the source center.
        let end = clipped(b, start, nodes[edge.target]);
        output.edges.push([start, end]);
    }
    if !output.width.is_finite()
        || !output.height.is_finite()
        || !finite(translation)
        || output.nodes.iter().any(|&p| !finite(p))
        || output.edges.iter().flatten().any(|&p| !finite(p))
    {
        return Err(Error::NonFiniteGeometry);
    }
    Ok(output)
}
fn clipped(origin: Point, target: Point, node: Node) -> Point {
    let dx = target.x - origin.x;
    let dy = target.y - origin.y;
    let mut scale: f64 = 1.;
    if dx.abs() > node.width / 2. {
        scale = scale.min(node.width / 2. / dx.abs());
    }
    if dy.abs() > node.height / 2. {
        scale = scale.min(node.height / 2. / dy.abs());
    }
    Point {
        x: origin.x + dx * scale,
        y: origin.y + dy * scale,
    }
}
fn overlap(a: Rectangle, b: Rectangle) -> f64 {
    let dx = (a.center().x - b.center().x).abs();
    let dy = (a.center().y - b.center().y).abs();
    if dx > a.width / 2. + b.width / 2. || dy > a.height / 2. + b.height / 2. {
        return 1.;
    }
    let x = (a.x - (b.x + b.width))
        .abs()
        .min((a.x + a.width - b.x).abs());
    let y = (a.y - (b.y + b.height))
        .abs()
        .min((a.y + a.height - b.y).abs());
    if dx == 0. && dy == 0. {
        0.
    } else if dx == 0. {
        y / dy + 1.
    } else if dy == 0. {
        x / dx + 1.
    } else {
        (x / dx).min(y / dy) + 1.
    }
}
fn cost(a: Rectangle, b: Rectangle) -> f64 {
    use std::cmp::Ordering::{Equal, Greater};
    let x = (a.x - (b.x + b.width)).max(b.x - (a.x + a.width));
    let y = (a.y - (b.y + b.height)).max(b.y - (a.y + a.height));
    let fx = fuzzy(x, 0., 0.00001);
    let fy = fuzzy(y, 0., 0.00001);
    let separation = if matches!(fx, Equal | Greater) ^ matches!(fy, Equal | Greater) {
        x.max(y)
    } else if fx == Greater {
        (x * x + y * y).sqrt()
    } else {
        -(x * x + y * y).sqrt()
    };
    if separation >= 0. {
        separation
    } else {
        -(overlap(a, b) - 1.) * distance(a.center(), b.center())
    }
}
fn spanning_tree(
    vertices: &[Vertex],
    links: &[(i32, [usize; 2])],
    root: usize,
    work: &mut dyn WorkControl,
) -> Result<Vec<(usize, usize)>, Error> {
    charge(
        work,
        checked_add(
            vertices.len(),
            checked_mul(links.len(), (links.len().max(1).ilog2() + 2) as usize)?,
        )?,
    )?;
    let mut links: Vec<_> = links
        .iter()
        .map(|(_, pair)| (*pair, cost(vertices[pair[0]].rect, vertices[pair[1]].rect)))
        .collect();
    if links.iter().any(|(_, c)| !c.is_finite()) {
        return Err(Error::NonFiniteGeometry);
    }
    links.sort_by(|a, b| a.1.total_cmp(&b.1));
    let mut included = vec![false; vertices.len()];
    included[root] = true;
    let mut result = Vec::with_capacity(vertices.len().saturating_sub(1));
    loop {
        charge(work, links.len())?;
        let Some(index) = links
            .iter()
            .position(|(pair, _)| included[pair[0]] != included[pair[1]])
        else {
            break;
        };
        let ([a, b], _) = links.remove(index);
        let (parent, child) = if included[a] { (a, b) } else { (b, a) };
        included[child] = true;
        result.push((parent, child));
    }
    Ok(result)
}
fn overlaps(
    vertices: &[Vertex],
    work: &mut dyn WorkControl,
) -> Result<Vec<(i32, [usize; 2])>, Error> {
    let event_count = checked_mul(vertices.len(), 2)?;
    charge(
        work,
        checked_mul(event_count, (event_count.max(1).ilog2() + 2) as usize)?,
    )?;
    let mut events = Vec::with_capacity(event_count);
    for (i, v) in vertices.iter().enumerate() {
        events.push((v.rect.y, true, i));
        events.push((v.rect.y + v.rect.height, false, i));
    }
    events.sort_by(|a, b| a.0.total_cmp(&b.0).then_with(|| a.1.cmp(&b.1)));
    let mut active: Vec<usize> = Vec::new();
    let mut links = Vec::new();
    for (_, low, i) in events {
        charge(work, checked_add(active.len(), 1)?)?;
        if !low {
            active.retain(|&j| j != i);
            continue;
        }
        let cmp = |&j: &usize| {
            vertices[j]
                .rect
                .x
                .total_cmp(&vertices[i].rect.x)
                .then_with(|| vertices[j].original.x.total_cmp(&vertices[i].original.x))
                .then_with(|| vertices[j].original.y.total_cmp(&vertices[i].original.y))
        };
        let at = active.binary_search_by(cmp).unwrap_or_else(|p| p);
        active.insert(at, i);
        let mut found = false;
        for &j in &active {
            let a = vertices[i].rect;
            let b = vertices[j].rect;
            if i != j
                && fuzzy(a.x, b.x + b.width, 0.0001).is_lt()
                && fuzzy(b.x, a.x + a.width, 0.0001).is_lt()
            {
                triangulation::add_edge(&mut links, vertices, [i, j], work)?;
                found = true;
            } else if found {
                break;
            }
        }
    }
    Ok(links)
}
#[cfg(test)]
mod tests;
