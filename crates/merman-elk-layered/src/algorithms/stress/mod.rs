//! ELK Stress majorization on measured, flat graphs.
//!
//! Ported from Eclipse ELK 0.9.1 (EPL-2.0), commit
//! `62d5909f96fad541bc101ad52dabaece6b7eab7e`: `StressLayoutProvider` and
//! `StressMajorization` in `org.eclipse.elk.alg.force.stress`.
//! The shared Force importer supplies component order, label placement, rectangle clipping,
//! and packing. Non-interactive initialization runs the actual Force kernel first, as upstream
//! does. Measurement, hierarchy traversal, and option resolution belong to the caller.

use super::force;
use crate::work::{WorkControl, WorkError, checked_add, checked_mul};
use std::cmp::Ordering;
use std::collections::BinaryHeap;

pub use force::{Layout, Point};

#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct Node {
    pub geometry: force::Node,
    pub fixed: bool,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Edge {
    pub geometry: force::Edge,
    pub desired_length: Option<f64>,
}

impl Edge {
    pub fn new(source: usize, target: usize) -> Self {
        Self {
            geometry: force::Edge::new(source, target),
            desired_length: None,
        }
    }
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum Dimension {
    #[default]
    Xy,
    X,
    Y,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Options {
    /// Shared import/export options and the non-interactive Force initialization parameters.
    /// `interactive` skips Force initialization; `resolved_seed` follows Force's seed contract.
    pub force: force::Options,
    pub dimension: Dimension,
    pub desired_edge_length: f64,
    pub epsilon: f64,
    /// The source uses a do/while loop: zero still performs one update pass.
    pub iteration_limit: usize,
}

impl Default for Options {
    fn default() -> Self {
        Self {
            force: force::Options::default(),
            dimension: Dimension::Xy,
            desired_edge_length: 100.0,
            epsilon: 0.001,
            iteration_limit: i32::MAX as usize,
        }
    }
}

#[derive(Debug, thiserror::Error, PartialEq)]
pub enum Error {
    #[error(transparent)]
    Work(#[from] WorkError),
    #[error(transparent)]
    Force(#[from] force::Error),
    #[error("ELK Stress received invalid option {0}")]
    InvalidOption(&'static str),
    #[error("ELK Stress received invalid desired length on edge {0}")]
    InvalidEdgeLength(usize),
    #[error("ELK Stress geometry exceeded the finite numeric range")]
    NonFiniteGeometry,
}

/// Runs localized stress minimization in source component/node order. Fixed nodes keep their
/// internal coordinates during minimization; source padding and component packing still translate
/// their final output. Failure leaves all caller-owned inputs unchanged.
pub fn layout(
    nodes: &[Node],
    edges: &[Edge],
    options: &Options,
    work: &mut dyn WorkControl,
) -> Result<Layout, Error> {
    charge(work, checked_add(nodes.len(), edges.len())?)?;
    if !options.desired_edge_length.is_finite() || options.desired_edge_length <= 0.0 {
        return Err(Error::InvalidOption("desired_edge_length"));
    }
    if !options.epsilon.is_finite() || options.epsilon < 0.0 {
        return Err(Error::InvalidOption("epsilon"));
    }
    if options.iteration_limit > i32::MAX as usize {
        return Err(Error::InvalidOption("iteration_limit"));
    }
    let mut force_nodes: Vec<_> = nodes.iter().map(|n| n.geometry).collect();
    let mut force_edges = Vec::with_capacity(edges.len());
    for (i, edge) in edges.iter().enumerate() {
        if let Some(length) = edge.desired_length
            && (!length.is_finite() || length <= 0.0)
        {
            return Err(Error::InvalidEdgeLength(i));
        }
        charge(work, edge.geometry.labels.len())?;
        force_edges.push(edge.geometry.clone());
    }
    if !options.force.interactive {
        let initial = force::layout(&force_nodes, &force_edges, &options.force, work)?;
        for (node, position) in force_nodes.iter_mut().zip(initial.nodes) {
            node.x = position.x;
            node.y = position.y;
        }
    }
    let mut graph = force::import(&force_nodes, &force_edges, &options.force, work)?;
    for component in &graph.components {
        if component.nodes.len() > 1 {
            minimize(
                &mut graph.particles,
                &component.nodes,
                nodes,
                edges,
                options,
                work,
            )?;
        }
    }
    graph.refresh_labels(&force_edges, work)?;
    Ok(force::finish(
        graph,
        &force_nodes,
        &force_edges,
        &options.force,
        work,
    )?)
}

fn charge(work: &mut dyn WorkControl, units: usize) -> Result<(), Error> {
    work.check(units)?;
    work.charge(units)?;
    Ok(())
}

// Queue duplicates are harmless: settled/stale entries are discarded. Distances, including the
// source's Integer.MAX_VALUE unreachable sentinel, retain the source arithmetic and edge weights.
#[derive(Clone, Copy, PartialEq)]
struct Distance {
    value: f64,
    node: usize,
}
impl Eq for Distance {}
impl Ord for Distance {
    fn cmp(&self, other: &Self) -> Ordering {
        other
            .value
            .total_cmp(&self.value)
            .then_with(|| other.node.cmp(&self.node))
    }
}
impl PartialOrd for Distance {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

fn minimize(
    particles: &mut [force::Particle],
    order: &[usize],
    nodes: &[Node],
    edges: &[Edge],
    options: &Options,
    work: &mut dyn WorkControl,
) -> Result<(), Error> {
    let n = order.len();
    let square = checked_mul(n, n)?;
    charge(work, checked_add(checked_mul(square, 2)?, nodes.len())?)?;
    let mut local = vec![usize::MAX; nodes.len()];
    for (i, &node) in order.iter().enumerate() {
        local[node] = i;
    }
    let mut adjacency = vec![Vec::new(); n];
    for edge in edges {
        charge(work, 1)?;
        let a = local[edge.geometry.source];
        let b = local[edge.geometry.target];
        if a == usize::MAX || b == usize::MAX || a == b {
            continue;
        }
        charge(work, 2)?;
        let length = edge.desired_length.unwrap_or(options.desired_edge_length);
        adjacency[a].push((b, length));
        adjacency[b].push((a, length));
    }
    let mut distances = vec![i32::MAX as f64; square];
    let mut weights = vec![0.0; square];
    for source in 0..n {
        let row = &mut distances[source * n..(source + 1) * n];
        row[source] = 0.0;
        let mut queue = BinaryHeap::new();
        queue.push(Distance {
            value: 0.0,
            node: source,
        });
        while let Some(Distance { value, node }) = queue.pop() {
            charge(work, 1)?;
            if value != row[node] {
                continue;
            }
            for &(other, length) in &adjacency[node] {
                charge(work, 1)?;
                let distance = value + length;
                if distance < row[other] {
                    row[other] = distance;
                    queue.push(Distance {
                        value: distance,
                        node: other,
                    });
                }
            }
        }
    }
    for (weight, distance) in weights.iter_mut().zip(&distances) {
        *weight = 1.0 / (distance * distance);
    }
    charge(work, square)?;
    let mut previous = stress(particles, order, &distances, &weights);
    for count in 0..=options.iteration_limit {
        charge(work, checked_mul(square, 2)?)?;
        for (i, &u) in order.iter().enumerate() {
            if nodes[u].fixed {
                continue;
            }
            let position = particles[u].position;
            let mut sum = 0.0;
            let mut x = 0.0;
            let mut y = 0.0;
            for (j, &v) in order.iter().enumerate() {
                if i == j {
                    continue;
                }
                let weight = weights[i * n + j];
                sum += weight;
                let other = particles[v].position;
                let dx = position.x - other.x;
                let dy = position.y - other.y;
                let distance = (dx * dx + dy * dy).sqrt();
                if distance > 0.0 && options.dimension != Dimension::Y {
                    x += weight * (other.x + distances[i * n + j] * dx / distance);
                }
                if distance > 0.0 && options.dimension != Dimension::X {
                    y += weight * (other.y + distances[i * n + j] * dy / distance);
                }
            }
            let next = Point {
                x: if options.dimension == Dimension::Y {
                    position.x
                } else {
                    x / sum
                },
                y: if options.dimension == Dimension::X {
                    position.y
                } else {
                    y / sum
                },
            };
            if !next.x.is_finite() || !next.y.is_finite() {
                return Err(Error::NonFiniteGeometry);
            }
            particles[u].position = next;
        }
        let current = stress(particles, order, &distances, &weights);
        if !current.is_finite() {
            return Err(Error::NonFiniteGeometry);
        }
        if previous == 0.0
            || (previous - current) / previous < options.epsilon
            || count >= options.iteration_limit
        {
            break;
        }
        previous = current;
    }
    Ok(())
}

fn stress(
    particles: &[force::Particle],
    order: &[usize],
    distances: &[f64],
    weights: &[f64],
) -> f64 {
    let mut result = 0.0;
    let n = order.len();
    for (i, &u) in order.iter().enumerate() {
        for (j, &v) in order.iter().enumerate().skip(i + 1) {
            let dx = particles[u].position.x - particles[v].position.x;
            let dy = particles[u].position.y - particles[v].position.y;
            let displacement = (dx * dx + dy * dy).sqrt() - distances[i * n + j];
            result += weights[i * n + j] * displacement * displacement;
        }
    }
    result
}

#[cfg(test)]
mod tests;
