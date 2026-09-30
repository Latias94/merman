//! ELK Force on measured, flat graphs.
//!
//! Ported from Eclipse ELK 0.9.1 (EPL-2.0), commit
//! `62d5909f96fad541bc101ad52dabaece6b7eab7e`: `org.eclipse.elk.alg.force`
//! (`ForceLayoutProvider`, `ComponentsProcessor`, `ElkGraphImporter`, `graph`, and `model`).
//! Measurement and compound traversal belong to the caller. Like the source importer, this
//! kernel ignores self loops and clips straight edges to node rectangles. Ports are not fixed
//! anchors in this algorithm. Repulsive bend particles are not exposed by Mermaid 12 and are
//! deliberately absent from this API; this is not a general ELK option interpreter.

use crate::random::JavaRandom;
use crate::work::{WorkControl, WorkError, checked_add, checked_mul};
use std::collections::BTreeMap;

#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct Point {
    pub x: f64,
    pub y: f64,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Node {
    pub width: f64,
    pub height: f64,
    pub x: f64,
    pub y: f64,
    pub priority: i32,
}

impl Default for Node {
    fn default() -> Self {
        Self {
            width: 0.0,
            height: 0.0,
            x: 0.0,
            y: 0.0,
            priority: 1,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Label {
    pub width: f64,
    pub height: f64,
    pub inline: bool,
    pub priority: i32,
}

impl Default for Label {
    fn default() -> Self {
        Self {
            width: 0.0,
            height: 0.0,
            inline: false,
            priority: 1,
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct Edge {
    pub source: usize,
    pub target: usize,
    pub priority: i32,
    /// Source reads this on the edge, not its containing graph.
    pub label_spacing: f64,
    pub labels: Vec<Label>,
}

impl Edge {
    pub fn new(source: usize, target: usize) -> Self {
        Self {
            source,
            target,
            priority: 1,
            label_spacing: 5.0,
            labels: Vec::new(),
        }
    }
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum Model {
    #[default]
    FruchtermanReingold,
    Eades,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Options {
    pub model: Model,
    pub iterations: usize,
    pub temperature: f64,
    pub repulsion: f64,
    pub spacing: f64,
    pub aspect_ratio: f64,
    pub padding: super::box_layout::Padding,
    pub interactive: bool,
    pub separate_components: bool,
    /// Resolved Java seed, not ELK's `randomSeed=0` unseeded sentinel. The caller owns
    /// resolving that sentinel once at the operation boundary.
    pub resolved_seed: i64,
}

impl Default for Options {
    fn default() -> Self {
        Self {
            model: Model::FruchtermanReingold,
            iterations: 300,
            temperature: 0.001,
            repulsion: 5.0,
            spacing: 80.0,
            aspect_ratio: 1.6_f32 as f64,
            padding: super::box_layout::Padding {
                top: 50.0,
                right: 50.0,
                bottom: 50.0,
                left: 50.0,
            },
            interactive: false,
            separate_components: true,
            resolved_seed: 1,
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct EdgeLayout {
    pub start: Point,
    pub end: Point,
    pub labels: Vec<Point>,
}

#[derive(Debug, Clone, Default, PartialEq)]
pub struct Layout {
    /// Node top-left positions in input order. Node dimensions remain caller-owned.
    pub nodes: Vec<Point>,
    /// Self loops have no route, matching the source importer's exclusion.
    pub edges: Vec<Option<EdgeLayout>>,
    pub width: f64,
    pub height: f64,
}

#[derive(Debug, thiserror::Error, PartialEq)]
pub enum Error {
    #[error(transparent)]
    Work(#[from] WorkError),
    #[error("ELK Force received invalid {field} on node {index}")]
    InvalidNode { index: usize, field: &'static str },
    #[error("ELK Force received invalid {field} on edge {index}")]
    InvalidEdge { index: usize, field: &'static str },
    #[error("ELK Force received invalid option {0}")]
    InvalidOption(&'static str),
    #[error("ELK Force geometry exceeded the finite numeric range")]
    NonFiniteGeometry,
    #[error("ELK Force cannot separate coincident coordinates at their numeric precision")]
    NumericStagnation,
}

#[derive(Clone, Copy)]
pub(super) struct Particle {
    pub(super) position: Point,
    displacement: Point,
    width: f64,
    height: f64,
    priority: i32,
}

#[derive(Default)]
pub(super) struct Component {
    pub(super) nodes: Vec<usize>,
    edges: Vec<usize>,
}

/// Runs the source force model. Failure leaves all caller-owned inputs unchanged.
pub fn layout(
    nodes: &[Node],
    edges: &[Edge],
    options: &Options,
    work: &mut dyn WorkControl,
) -> Result<Layout, Error> {
    let mut graph = import(nodes, edges, options, work)?;
    let mut random = JavaRandom::new(options.resolved_seed);
    for component in &graph.components {
        simulate(
            &mut graph.particles,
            component,
            edges,
            &graph.labels,
            options,
            &mut random,
            work,
        )?;
    }
    finish(graph, nodes, edges, options, work)
}

/// Shared source importer and component representation used by Force and Stress.
#[derive(Default)]
pub(super) struct ImportedGraph {
    pub(super) particles: Vec<Particle>,
    pub(super) components: Vec<Component>,
    labels: Vec<Vec<usize>>,
    edge_order: Vec<usize>,
}

impl ImportedGraph {
    pub(super) fn refresh_labels(
        &mut self,
        edges: &[Edge],
        work: &mut dyn WorkControl,
    ) -> Result<(), Error> {
        for &e in &self.edge_order {
            charge(work, checked_add(1, self.labels[e].len())?)?;
            refresh_labels(&mut self.particles, &edges[e], &self.labels[e]);
        }
        Ok(())
    }
}

pub(super) fn import(
    nodes: &[Node],
    edges: &[Edge],
    options: &Options,
    work: &mut dyn WorkControl,
) -> Result<ImportedGraph, Error> {
    charge(work, 1)?;
    validate(nodes, edges, options, work)?;
    if nodes.is_empty() {
        return Ok(ImportedGraph::default());
    }
    let mut count = nodes.len();
    for edge in edges {
        count = checked_add(count, edge.labels.len())?;
    }
    charge(work, count)?;
    let mut particles = Vec::with_capacity(count);
    for node in nodes {
        particles.push(Particle {
            position: Point {
                x: node.x + node.width / 2.0,
                y: node.y + node.height / 2.0,
            },
            displacement: Point::default(),
            width: node.width.max(1.0),
            height: node.height.max(1.0),
            priority: node.priority,
        });
    }
    let mut labels = vec![Vec::new(); edges.len()];
    // ElkGraphImporter visits outgoing edges in node order, regardless of the graph edge array.
    let mut edge_order: Vec<usize> = (0..edges.len())
        .filter(|&e| edges[e].source != edges[e].target)
        .collect();
    edge_order.sort_by_key(|&e| edges[e].source);
    for &e in &edge_order {
        for label in &edges[e].labels {
            labels[e].push(particles.len());
            particles.push(Particle {
                position: Point::default(),
                displacement: Point::default(),
                width: label.width.max(1.0),
                height: label.height.max(1.0),
                priority: label.priority,
            });
        }
    }
    let components = split(
        nodes.len(),
        edges,
        &edge_order,
        options.separate_components,
        work,
    )?;
    Ok(ImportedGraph {
        particles,
        components,
        labels,
        edge_order,
    })
}

pub(super) fn finish(
    graph: ImportedGraph,
    nodes: &[Node],
    edges: &[Edge],
    options: &Options,
    work: &mut dyn WorkControl,
) -> Result<Layout, Error> {
    if nodes.is_empty() {
        return Ok(Layout::default());
    }
    let ImportedGraph {
        mut particles,
        components,
        labels,
        edge_order,
    } = graph;
    pack(&mut particles, &components, &labels, options, work)?;
    let (min, max) = bounds(&particles, 0..nodes.len());
    let offset = Point {
        x: options.padding.left - min.x,
        y: options.padding.top - min.y,
    };
    let mut result = Layout {
        nodes: Vec::with_capacity(nodes.len()),
        edges: vec![None; edges.len()],
        width: max.x - min.x + options.padding.left + options.padding.right,
        height: max.y - min.y + options.padding.top + options.padding.bottom,
    };
    for (index, node) in nodes.iter().enumerate() {
        let p = particles[index].position;
        result.nodes.push(Point {
            x: p.x + offset.x - node.width / 2.0,
            y: p.y + offset.y - node.height / 2.0,
        });
    }
    for &e in &edge_order {
        charge(work, 1)?;
        let edge = &edges[e];
        let mut start = clip(&particles[edge.source], particles[edge.target].position);
        let mut end = clip(&particles[edge.target], particles[edge.source].position);
        translate(&mut start, offset);
        translate(&mut end, offset);
        let positions = labels[e]
            .iter()
            .map(|&l| {
                let mut p = particles[l].position;
                translate(&mut p, offset);
                p
            })
            .collect();
        result.edges[e] = Some(EdgeLayout {
            start,
            end,
            labels: positions,
        });
    }
    if !result.width.is_finite()
        || !result.height.is_finite()
        || result.nodes.iter().any(|p| !finite(*p))
        || result
            .edges
            .iter()
            .flatten()
            .any(|e| !finite(e.start) || !finite(e.end) || e.labels.iter().any(|p| !finite(*p)))
    {
        return Err(Error::NonFiniteGeometry);
    }
    Ok(result)
}

fn split(
    n: usize,
    edges: &[Edge],
    order: &[usize],
    separate: bool,
    work: &mut dyn WorkControl,
) -> Result<Vec<Component>, Error> {
    if !separate {
        return Ok(vec![Component {
            nodes: (0..n).collect(),
            edges: order.to_vec(),
        }]);
    }
    charge(work, checked_add(n, checked_mul(order.len(), 2)?)?)?;
    let mut incidence = vec![Vec::new(); n];
    for &e in order {
        incidence[edges[e].source].push(e);
        incidence[edges[e].target].push(e);
    }
    let mut seen = vec![false; n];
    let mut result = Vec::new();
    // Explicit enter/edge/return frames preserve the source DFS's node and postorder edge order.
    // In cycles the source can append an edge twice. Preserve this multiplicity: it influences
    // both adjacency weights and particle visitation for the edge's labels.
    enum Visit {
        Node(usize, Option<usize>),
        Edge(usize, usize, Option<usize>),
        Append(usize),
    }
    for start in 0..n {
        if seen[start] {
            continue;
        }
        let mut component = Component::default();
        let mut stack = vec![Visit::Node(start, None)];
        while let Some(visit) = stack.pop() {
            charge(work, 1)?;
            match visit {
                Visit::Node(node, last) => {
                    if seen[node] {
                        continue;
                    }
                    seen[node] = true;
                    component.nodes.push(node);
                    for &e in incidence[node].iter().rev() {
                        stack.push(Visit::Edge(e, node, last));
                    }
                }
                Visit::Edge(e, node, last) => {
                    let edge = &edges[e];
                    if Some(edge.source) == last || Some(edge.target) == last {
                        continue;
                    }
                    stack.push(Visit::Append(e));
                    if edge.target != node {
                        stack.push(Visit::Node(edge.target, Some(node)));
                    }
                    if edge.source != node {
                        stack.push(Visit::Node(edge.source, Some(node)));
                    }
                }
                Visit::Append(e) => component.edges.push(e),
            }
        }
        result.push(component);
    }
    Ok(result)
}

fn simulate(
    particles: &mut [Particle],
    component: &Component,
    edges: &[Edge],
    labels: &[Vec<usize>],
    options: &Options,
    random: &mut JavaRandom,
    work: &mut dyn WorkControl,
) -> Result<(), Error> {
    let mut order = component.nodes.clone();
    let mut adjacency = BTreeMap::<(usize, usize), i32>::new();
    for &e in &component.edges {
        charge(work, 1)?;
        order.extend_from_slice(&labels[e]);
        let edge = &edges[e];
        let weight = adjacency
            .entry((edge.source.min(edge.target), edge.source.max(edge.target)))
            .or_default();
        *weight = weight.wrapping_add(edge.priority);
    }
    let n = component.nodes.len() as f64;
    let bound = (n * 16.0 + component.edges.len() as f64).max(256.0);
    let mut width = 0.0;
    let mut height = 0.0;
    for &node in &component.nodes {
        if !options.interactive {
            particles[node].position = Point {
                x: random.next_double() * n,
                y: random.next_double() * n,
            };
        }
        width += particles[node].width;
        height += particles[node].height;
    }
    let k = (width * height / (2.0 * n)).sqrt() * (options.spacing * 0.01);
    if !k.is_finite() {
        return Err(Error::NonFiniteGeometry);
    }
    for &node in &component.nodes {
        if !finite(particles[node].position) {
            return Err(Error::NonFiniteGeometry);
        }
    }
    let mut temperature = options.temperature;
    let threshold = temperature / options.iterations as f64;
    let mut refresh_work = component.edges.len();
    for &edge in &component.edges {
        refresh_work = checked_add(refresh_work, labels[edge].len())?;
    }
    let iteration_work = checked_add(checked_mul(order.len(), order.len())?, refresh_work)?;
    let mut iteration = 0;
    while match options.model {
        Model::Eades => iteration < options.iterations,
        Model::FruchtermanReingold => temperature > 0.0,
    } {
        charge(work, iteration_work)?;
        for &e in &component.edges {
            refresh_labels(particles, &edges[e], &labels[e]);
        }
        if options.model == Model::FruchtermanReingold {
            temperature -= threshold;
        }
        for &v in &order {
            for &u in &order {
                if u == v {
                    continue;
                }
                while particles[u].position == particles[v].position {
                    charge(work, 1)?;
                    let before_u = particles[u].position;
                    let before_v = particles[v].position;
                    particles[u].position.x += random.next_double() - 0.5;
                    particles[u].position.y += random.next_double() - 0.5;
                    particles[v].position.x += random.next_double() - 0.5;
                    particles[v].position.y += random.next_double() - 0.5;
                    if particles[u].position == before_u && particles[v].position == before_v {
                        return Err(Error::NumericStagnation);
                    }
                }
                let a = particles[u];
                let b = particles[v];
                let dx = b.position.x - a.position.x;
                let dy = b.position.y - a.position.y;
                let length = (dx * dx + dy * dy).sqrt();
                let radius_a = (a.width * a.width + a.height * a.height).sqrt() / 2.0;
                let radius_b = (b.width * b.width + b.height * b.height).sqrt() / 2.0;
                let distance = (length - radius_a - radius_b).max(0.0);
                let connection = adjacency.get(&(u.min(v), u.max(v))).copied().unwrap_or(0);
                let force = match options.model {
                    Model::Eades => {
                        if connection > 0 {
                            -(if distance > 0.0 {
                                (distance / options.spacing).ln()
                            } else {
                                -100.0
                            }) * connection as f64
                        } else {
                            (if distance > 0.0 {
                                options.repulsion / (distance * distance)
                            } else {
                                options.repulsion * 100.0
                            }) * a.priority as f64
                        }
                    }
                    Model::FruchtermanReingold => {
                        let mut force = (if distance > 0.0 {
                            k * k / distance
                        } else {
                            k * k * 100.0
                        }) * a.priority as f64;
                        if connection > 0 {
                            force -= distance * distance / k * connection as f64;
                        }
                        force * temperature
                    }
                };
                let scale = force / length;
                particles[v].displacement.x += dx * scale;
                particles[v].displacement.y += dy * scale;
            }
        }
        for &v in &order {
            let p = &mut particles[v];
            if !finite(p.displacement) {
                return Err(Error::NonFiniteGeometry);
            }
            p.position.x += p.displacement.x.clamp(-bound, bound);
            p.position.y += p.displacement.y.clamp(-bound, bound);
            p.displacement = Point::default();
        }
        iteration += 1;
    }
    Ok(())
}

fn refresh_labels(particles: &mut [Particle], edge: &Edge, indices: &[usize]) {
    let a = particles[edge.source].position;
    let b = particles[edge.target].position;
    for (&index, label) in indices.iter().zip(&edge.labels) {
        let p = &mut particles[index];
        p.position = if label.inline {
            Point {
                x: a.x + (b.x - a.x) * 0.5 - p.width * 0.5,
                y: a.y + (b.y - a.y) * 0.5 - p.height * 0.5,
            }
        } else {
            let x = a.x.min(b.x) + (a.x - b.x).abs() / 2.0 + edge.label_spacing;
            let mid_y = a.y.min(b.y) + (a.y - b.y).abs() / 2.0;
            let y = if (a.x >= b.x) == (a.y >= b.y) {
                mid_y - edge.label_spacing - p.height
            } else {
                mid_y + edge.label_spacing
            };
            Point { x, y }
        };
    }
}

fn pack(
    particles: &mut [Particle],
    components: &[Component],
    labels: &[Vec<usize>],
    options: &Options,
    work: &mut dyn WorkControl,
) -> Result<(), Error> {
    if components.len() <= 1 {
        return Ok(());
    }
    let bounds: Vec<_> = components
        .iter()
        .map(|c| bounds(particles, c.nodes.iter().copied()))
        .collect();
    let priorities: Vec<i32> = components
        .iter()
        .map(|c| {
            c.nodes
                .iter()
                .fold(0_i32, |sum, &n| sum.wrapping_add(particles[n].priority))
        })
        .collect();
    let mut order: Vec<_> = (0..components.len()).collect();
    order.sort_by(|&a, &b| {
        priorities[b].cmp(&priorities[a]).then_with(|| {
            let (amin, amax) = bounds[a];
            let (bmin, bmax) = bounds[b];
            ((amax.x - amin.x) * (amax.y - amin.y))
                .total_cmp(&((bmax.x - bmin.x) * (bmax.y - bmin.y)))
        })
    });
    let mut widest: f64 = 0.0;
    let mut area = 0.0;
    for &(min, max) in &bounds {
        widest = widest.max(max.x - min.x);
        area += (max.x - min.x) * (max.y - min.y);
    }
    // Source explicitly rounds the square-root and aspect-ratio product to Java float.
    widest = widest.max(((area.sqrt() as f32) * (options.aspect_ratio as f32)) as f64);
    let mut x = 0.0;
    let mut y = 0.0;
    let mut tallest: f64 = 0.0;
    for c in order {
        charge(
            work,
            checked_add(components[c].nodes.len(), components[c].edges.len())?,
        )?;
        let (min, max) = bounds[c];
        let width = max.x - min.x;
        let height = max.y - min.y;
        if x + width > widest {
            x = 0.0;
            y += tallest + options.spacing;
            tallest = 0.0;
        }
        let shift = Point {
            x: x - min.x,
            y: y - min.y,
        };
        for &node in &components[c].nodes {
            translate(&mut particles[node].position, shift);
        }
        for &e in &components[c].edges {
            for &l in &labels[e] {
                translate(&mut particles[l].position, shift);
            }
        }
        tallest = tallest.max(height);
        x += width + options.spacing;
    }
    Ok(())
}

fn bounds(particles: &[Particle], indices: impl Iterator<Item = usize>) -> (Point, Point) {
    let mut min = Point {
        x: i32::MAX as f64,
        y: i32::MAX as f64,
    };
    let mut max = Point {
        x: i32::MIN as f64,
        y: i32::MIN as f64,
    };
    for i in indices {
        let p = particles[i];
        min.x = min.x.min(p.position.x - p.width / 2.0);
        min.y = min.y.min(p.position.y - p.height / 2.0);
        max.x = max.x.max(p.position.x + p.width / 2.0);
        max.y = max.y.max(p.position.y + p.height / 2.0);
    }
    (min, max)
}

fn clip(node: &Particle, target: Point) -> Point {
    let dx = target.x - node.position.x;
    let dy = target.y - node.position.y;
    let sx = if dx.abs() > node.width / 2.0 {
        node.width / 2.0 / dx.abs()
    } else {
        1.0
    };
    let sy = if dy.abs() > node.height / 2.0 {
        node.height / 2.0 / dy.abs()
    } else {
        1.0
    };
    let scale = sx.min(sy);
    Point {
        x: node.position.x + dx * scale,
        y: node.position.y + dy * scale,
    }
}

fn translate(point: &mut Point, offset: Point) {
    point.x += offset.x;
    point.y += offset.y;
}
fn finite(point: Point) -> bool {
    point.x.is_finite() && point.y.is_finite()
}
fn charge(work: &mut dyn WorkControl, units: usize) -> Result<(), Error> {
    work.check(units)?;
    work.charge(units)?;
    Ok(())
}

fn validate(
    nodes: &[Node],
    edges: &[Edge],
    options: &Options,
    work: &mut dyn WorkControl,
) -> Result<(), Error> {
    if options.iterations == 0 || options.iterations > i32::MAX as usize {
        return Err(Error::InvalidOption("iterations"));
    }
    for (name, value) in [
        ("temperature", options.temperature),
        ("repulsion", options.repulsion),
        ("spacing", options.spacing),
        ("aspect_ratio", options.aspect_ratio),
    ] {
        if !value.is_finite() || value <= 0.0 {
            return Err(Error::InvalidOption(name));
        }
    }
    if options.temperature / options.iterations as f64 == 0.0 {
        return Err(Error::InvalidOption("temperature"));
    }
    for p in [
        options.padding.top,
        options.padding.right,
        options.padding.bottom,
        options.padding.left,
    ] {
        if !p.is_finite() || p < 0.0 {
            return Err(Error::InvalidOption("padding"));
        }
    }
    for (index, node) in nodes.iter().enumerate() {
        charge(work, 1)?;
        for (field, value) in [("width", node.width), ("height", node.height)] {
            if !value.is_finite() || value < 0.0 {
                return Err(Error::InvalidNode { index, field });
            }
        }
        for (field, value) in [
            ("x", node.x),
            ("y", node.y),
            ("center x", node.x + node.width / 2.0),
            ("center y", node.y + node.height / 2.0),
        ] {
            if !value.is_finite() {
                return Err(Error::InvalidNode { index, field });
            }
        }
    }
    for (index, edge) in edges.iter().enumerate() {
        charge(work, checked_add(1, edge.labels.len())?)?;
        if edge.source >= nodes.len() || edge.target >= nodes.len() {
            return Err(Error::InvalidEdge {
                index,
                field: "endpoint",
            });
        }
        if !edge.label_spacing.is_finite() || edge.label_spacing < 0.0 {
            return Err(Error::InvalidEdge {
                index,
                field: "label_spacing",
            });
        }
        for label in &edge.labels {
            if !label.width.is_finite()
                || !label.height.is_finite()
                || label.width < 0.0
                || label.height < 0.0
            {
                return Err(Error::InvalidEdge {
                    index,
                    field: "label dimensions",
                });
            }
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests;
