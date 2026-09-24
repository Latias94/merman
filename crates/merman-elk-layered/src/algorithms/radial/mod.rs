//! ELK Radial's default Eades placement, radius-extension overlap removal, and routing.
//!
//! Ported from Eclipse ELK 0.9.1 (EPL-2.0), commit
//! `62d5909f96fad541bc101ad52dabaece6b7eab7e`, `org.eclipse.elk.alg.radial`:
//! `RadialUtil`, `EadesRadial`, `AnnulusWedgeByNodeSpace`,
//! `AbstractRadiusExtensionCompaction`, `RadiusExtensionOverlapRemoval`,
//! `CalculateGraphSize`, and `StraightLineEdgeRouter`.
//! This is the configuration reachable through Mermaid 12 container algorithms. Measurement,
//! hierarchy traversal, and node micro layout belong to the caller. Nondefault radial sorters,
//! compactors, rotation, and optimization criteria are not exposed by that adapter.

use super::box_layout::Padding;
use crate::work::{WorkControl, WorkError, checked_add, checked_mul};

#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct Point {
    pub x: f64,
    pub y: f64,
}

#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct Margins {
    pub top: f64,
    pub right: f64,
    pub bottom: f64,
    pub left: f64,
}

#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct Node {
    pub width: f64,
    pub height: f64,
    /// Initial positions remain significant for nodes outside the first root's reachable graph.
    pub position: Point,
    pub margins: Margins,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Edge {
    pub source: usize,
    pub target: usize,
    /// Radial excludes port-sourced edges from successor discovery and routing, but they still
    /// count as incoming edges when choosing the first root.
    pub source_is_port: bool,
}

impl Edge {
    pub fn new(source: usize, target: usize) -> Self {
        Self {
            source,
            target,
            source_is_port: false,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Options {
    pub spacing: f64,
    pub padding: Padding,
}

impl Default for Options {
    fn default() -> Self {
        Self {
            spacing: 20.0,
            padding: Padding {
                top: 12.0,
                right: 12.0,
                bottom: 12.0,
                left: 12.0,
            },
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct EdgeLayout {
    pub start: Point,
    pub end: Point,
}

#[derive(Debug, Clone, Default, PartialEq)]
pub struct Layout {
    pub nodes: Vec<Point>,
    /// Edges outside the first root's reachable graph, and port-sourced edges, have no route.
    pub edges: Vec<Option<EdgeLayout>>,
    pub width: f64,
    pub height: f64,
}

#[derive(Debug, thiserror::Error, PartialEq)]
pub enum Error {
    #[error(transparent)]
    Work(#[from] WorkError),
    #[error("ELK Radial received invalid geometry on node {0}")]
    InvalidNode(usize),
    #[error("ELK Radial received invalid endpoint on edge {0}")]
    InvalidEdge(usize),
    #[error("ELK Radial received invalid option {0}")]
    InvalidOption(&'static str),
    #[error("ELK Radial requires a root without incoming edges")]
    MissingRoot,
    #[error("ELK Radial encountered a cycle reachable from the root")]
    ReachableCycle,
    #[error("ELK Radial geometry exceeded the finite numeric range")]
    NonFiniteGeometry,
    #[error("ELK Radial cannot extend an overlapping radius at its numeric precision")]
    NumericStagnation,
}

fn charge(work: &mut dyn WorkControl, amount: usize) -> Result<(), Error> {
    work.check(amount)?;
    work.charge(amount)?;
    Ok(())
}

/// Layout measured nodes without mutating caller inputs. As upstream, the first node without
/// incoming edges is the root; disconnected nodes retain their input positions before the final
/// translation. Acyclic shared successors preserve source traversal order. Reachable cycles fail
/// explicitly instead of overflowing the source's recursive traversal stack.
pub fn layout(
    nodes: &[Node],
    edges: &[Edge],
    options: &Options,
    work: &mut dyn WorkControl,
) -> Result<Layout, Error> {
    charge(
        work,
        checked_add(checked_mul(nodes.len(), 8)?, checked_mul(edges.len(), 2)?)?,
    )?;
    if !options.spacing.is_finite() || options.spacing < 0.0 {
        return Err(Error::InvalidOption("spacing"));
    }
    let padding = options.padding;
    if [padding.top, padding.right, padding.bottom, padding.left]
        .iter()
        .any(|v| !v.is_finite() || *v < 0.0)
    {
        return Err(Error::InvalidOption("padding"));
    }
    let mut radius: f64 = 0.0;
    for (i, node) in nodes.iter().enumerate() {
        let margins = node.margins;
        if [node.width, node.height]
            .iter()
            .any(|v| !v.is_finite() || *v < 0.0)
            || [
                node.position.x,
                node.position.y,
                margins.top,
                margins.right,
                margins.bottom,
                margins.left,
            ]
            .iter()
            .any(|v| !v.is_finite())
        {
            return Err(Error::InvalidNode(i));
        }
        radius = radius.max(diagonal(node));
    }
    if !radius.is_finite() {
        return Err(Error::NonFiniteGeometry);
    }
    let mut incoming = vec![false; nodes.len()];
    let mut successors = vec![Vec::new(); nodes.len()];
    for (i, edge) in edges.iter().enumerate() {
        if edge.source >= nodes.len() || edge.target >= nodes.len() {
            return Err(Error::InvalidEdge(i));
        }
        incoming[edge.target] = true;
        if !edge.source_is_port {
            successors[edge.source].push(edge.target);
        }
    }
    if nodes.is_empty() {
        return Ok(Layout::default());
    }
    let root = incoming
        .iter()
        .position(|&value| !value)
        .ok_or(Error::MissingRoot)?;
    let (weights, reachable) = wedge_weights(nodes, &successors, root, work)?;
    let mut positions: Vec<_> = nodes.iter().map(|node| node.position).collect();
    position_nodes(
        nodes,
        &successors,
        &weights,
        root,
        radius,
        &mut positions,
        work,
    )?;
    extend_radii(
        nodes,
        &successors,
        root,
        options.spacing,
        &mut positions,
        work,
    )?;
    let (width, height) = translate(nodes, &mut positions, padding);
    if !width.is_finite()
        || !height.is_finite()
        || positions
            .iter()
            .any(|p| !p.x.is_finite() || !p.y.is_finite())
    {
        return Err(Error::NonFiniteGeometry);
    }
    let mut routes = Vec::with_capacity(edges.len());
    for edge in edges {
        charge(work, 1)?;
        routes.push(if !edge.source_is_port && reachable[edge.source] {
            Some(route(
                nodes[edge.source],
                nodes[edge.target],
                positions[edge.source],
                positions[edge.target],
            ))
        } else {
            None
        });
    }
    if routes.iter().flatten().any(|route| {
        [route.start.x, route.start.y, route.end.x, route.end.y]
            .iter()
            .any(|v| !v.is_finite())
    }) {
        return Err(Error::NonFiniteGeometry);
    }
    Ok(Layout {
        nodes: positions,
        edges: routes,
        width,
        height,
    })
}

fn diagonal(node: &Node) -> f64 {
    (node.width * node.width + node.height * node.height).sqrt()
}

fn wedge_weights(
    nodes: &[Node],
    successors: &[Vec<usize>],
    root: usize,
    work: &mut dyn WorkControl,
) -> Result<(Vec<f64>, Vec<bool>), Error> {
    let mut weights = vec![0.0; nodes.len()];
    let mut state = vec![0_u8; nodes.len()];
    let mut stack = vec![(root, 0)];
    state[root] = 1;
    while let Some((node, next)) = stack.last_mut() {
        charge(work, 1)?;
        if let Some(&child) = successors[*node].get(*next) {
            *next += 1;
            match state[child] {
                1 => return Err(Error::ReachableCycle),
                0 => {
                    state[child] = 1;
                    stack.push((child, 0));
                }
                _ => {}
            }
        } else {
            let node = *node;
            charge(work, successors[node].len())?;
            let total: f64 = successors[node].iter().map(|&child| weights[child]).sum();
            weights[node] = total.max(diagonal(&nodes[node]));
            if !weights[node].is_finite() {
                return Err(Error::NonFiniteGeometry);
            }
            state[node] = 2;
            stack.pop();
        }
    }
    Ok((weights, state.into_iter().map(|value| value == 2).collect()))
}

#[allow(clippy::too_many_arguments)]
fn position_nodes(
    nodes: &[Node],
    successors: &[Vec<usize>],
    weights: &[f64],
    root: usize,
    radius: f64,
    positions: &mut [Point],
    work: &mut dyn WorkControl,
) -> Result<(), Error> {
    let mut stack = vec![(root, 0.0, 0.0, std::f64::consts::TAU)];
    while let Some((node, current_radius, minimum, maximum)) = stack.pop() {
        charge(work, checked_add(successors[node].len(), 1)?)?;
        let angle = (minimum + maximum) / 2.0;
        positions[node] = Point {
            x: current_radius * angle.cos() - nodes[node].width / 2.0,
            y: current_radius * angle.sin() - nodes[node].height / 2.0,
        };
        // Preserve the pinned source's parentheses: this is not acos(r / (r + radius)).
        // With the source default radius, tau is NaN and the complete wedge is used.
        let radius_ratio = if current_radius == 0.0 || !current_radius.is_finite() {
            f64::NAN
        } else {
            1.0
        };
        let tau = 2.0 * (radius_ratio + radius).acos();
        let (scale, mut alpha) = if tau < maximum - minimum {
            (tau / weights[node], (minimum + maximum - tau) / 2.0)
        } else {
            ((maximum - minimum) / weights[node], minimum)
        };
        let begin = stack.len();
        for &child in &successors[node] {
            let end = alpha + scale * weights[child];
            stack.push((child, current_radius + radius, alpha, end));
            alpha += scale * weights[child];
        }
        stack[begin..].reverse();
    }
    Ok(())
}

fn overlap(a: Node, b: Node, ap: Point, bp: Point, spacing: f64) -> bool {
    let (x1, x2) = (ap.x - spacing / 2.0, bp.x - spacing / 2.0);
    let (y1, y2) = (ap.y - spacing / 2.0, bp.y - spacing / 2.0);
    let (w1, w2) = (a.width + spacing, b.width + spacing);
    let (h1, h2) = (a.height + spacing, b.height + spacing);
    // The source uses strict corner tests, including its equal-coordinate behavior.
    ((x1 < x2 + w2 && x2 < x1) && (y1 < y2 + h2 && y2 < y1))
        || ((x2 < x1 + w1 && x1 < x2) && (y2 < y1 + h1 && y1 < y2))
        || ((x1 < x2 + w2 && x2 < x1) && (y1 < y2 && y2 < y1 + h1))
        || ((x2 < x1 + w1 && x1 < x2) && (y1 < y2 + h2 && y2 < y1))
}

fn extend_radii(
    nodes: &[Node],
    successors: &[Vec<usize>],
    root: usize,
    spacing: f64,
    positions: &mut [Point],
    work: &mut dyn WorkControl,
) -> Result<(), Error> {
    let center = Point {
        x: positions[root].x + nodes[root].width / 2.0,
        y: positions[root].y + nodes[root].height / 2.0,
    };
    let mut layer = successors[root].clone();
    let mut seen = vec![0_usize; nodes.len()];
    let mut generation = 0_usize;
    while !layer.is_empty() {
        charge(work, layer.len())?;
        let original = positions[layer[0]];
        loop {
            charge(work, layer.len())?;
            let overlapping = layer.len() > 1
                && layer.iter().enumerate().any(|(i, &a)| {
                    let b = layer[(i + 1) % layer.len()];
                    overlap(nodes[a], nodes[b], positions[a], positions[b], spacing)
                });
            if !overlapping {
                break;
            }
            charge(work, layer.len())?;
            for &node in &layer {
                let old = positions[node];
                let x = old.x + nodes[node].width / 2.0 - center.x;
                let y = old.y + nodes[node].height / 2.0 - center.y;
                let length = (x * x + y * y).sqrt();
                // contractLayer computes a new center and then subtracts the half-size.
                positions[node] = Point {
                    x: old.x + nodes[node].width / 2.0 + x * (1.0 / length)
                        - nodes[node].width / 2.0,
                    y: old.y + nodes[node].height / 2.0 + y * (1.0 / length)
                        - nodes[node].height / 2.0,
                };
                if !positions[node].x.is_finite() || !positions[node].y.is_finite() {
                    return Err(Error::NonFiniteGeometry);
                }
                if positions[node] == old {
                    return Err(Error::NumericStagnation);
                }
            }
        }
        let dx = positions[layer[0]].x - original.x;
        let dy = positions[layer[0]].y - original.y;
        let distance = (dx * dx + dy * dy).sqrt();
        generation = checked_add(generation, 1)?;
        let mut next = Vec::new();
        for node in layer {
            charge(work, successors[node].len())?;
            for &child in &successors[node] {
                if seen[child] != generation {
                    seen[child] = generation;
                    next.push(child);
                    let p = &mut positions[child];
                    let x = p.x + nodes[child].width / 2.0 - center.x;
                    let y = p.y + nodes[child].height / 2.0 - center.y;
                    let length = (x * x + y * y).sqrt();
                    // Upstream moves the immediate next level only, even if distance is zero.
                    p.x += x / length * distance;
                    p.y += y / length * distance;
                }
            }
        }
        layer = next;
    }
    Ok(())
}

fn translate(nodes: &[Node], positions: &mut [Point], padding: Padding) -> (f64, f64) {
    let (mut min_x, mut min_y) = (f64::MAX, f64::MAX);
    // Java Double.MIN_VALUE is the smallest positive subnormal, not the most negative value.
    let (mut max_x, mut max_y) = (f64::from_bits(1), f64::from_bits(1));
    for (node, p) in nodes.iter().zip(positions.iter()) {
        min_x = min_x.min(p.x - node.margins.left);
        min_y = min_y.min(p.y - node.margins.top);
        max_x = max_x.max(p.x + node.width + node.margins.right);
        max_y = max_y.max(p.y + node.height + node.margins.bottom);
    }
    let offset_x = min_x - padding.left;
    let offset_y = min_y - padding.top;
    for p in positions {
        p.x -= offset_x;
        p.y -= offset_y;
    }
    (
        max_x - min_x + padding.left + padding.right,
        max_y - min_y + padding.top + padding.bottom,
    )
}

fn route(source: Node, target: Node, sp: Point, tp: Point) -> EdgeLayout {
    let mut source_center = Point {
        x: sp.x + source.width / 2.0,
        y: sp.y + source.height / 2.0,
    };
    let mut target_center = Point {
        x: tp.x + target.width / 2.0,
        y: tp.y + target.height / 2.0,
    };
    let mut vector = Point {
        x: target_center.x - source_center.x,
        y: target_center.y - source_center.y,
    };
    let source_clip = clip(vector, source.width, source.height);
    vector.x -= source_clip.x;
    vector.y -= source_clip.y;
    source_center.x = target_center.x - vector.x;
    source_center.y = target_center.y - vector.y;
    let target_clip = clip(vector, target.width, target.height);
    vector.x -= target_clip.x;
    vector.y -= target_clip.y;
    target_center.x = source_center.x + vector.x;
    target_center.y = source_center.y + vector.y;
    EdgeLayout {
        start: source_center,
        end: target_center,
    }
}

fn clip(vector: Point, width: f64, height: f64) -> Point {
    let x_scale = if vector.x.abs() > width / 2.0 {
        width / 2.0 / vector.x.abs()
    } else {
        1.0
    };
    let y_scale = if vector.y.abs() > height / 2.0 {
        height / 2.0 / vector.y.abs()
    } else {
        1.0
    };
    let scale = x_scale.min(y_scale);
    Point {
        x: vector.x * scale,
        y: vector.y * scale,
    }
}

#[cfg(test)]
mod tests;
