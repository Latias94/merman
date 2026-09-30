//! ELK Mr. Tree on measured flat graphs.
//!
//! Modified Rust translation of Eclipse ELK 0.9.1 (EPL-2.0), commit
//! `62d5909f96fad541bc101ad52dabaece6b7eab7e`, `org.eclipse.elk.alg.mrtree`:
//! importer, component processing, DFS treeification, Walker placement and overlap-avoiding
//! edge routing. The API admits the options emitted by Mermaid 12; alternative tree weighting,
//! BFS treeification and optional compaction are not selected by that adapter.
//! Measurement, ports, compound traversal and edge labels remain caller-owned. The source
//! importer ignores self loops and does not lay out edge labels.

use crate::work::{WorkControl, WorkError, checked_add, checked_mul, checked_n_log_n};
use std::collections::BTreeSet;

pub use super::box_layout::Padding;

mod placement;
mod routing;

#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct Point {
    pub x: f64,
    pub y: f64,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Node {
    pub width: f64,
    pub height: f64,
    pub priority: i32,
    /// First ELK node label, used by the source's edge identity comparison.
    pub label: String,
    /// Mr. Tree normalization reads the first child's padding, independently of graph padding.
    pub padding: Padding,
}

impl Default for Node {
    fn default() -> Self {
        Self {
            width: 0.0,
            height: 0.0,
            priority: 1,
            label: String::new(),
            padding: default_padding(),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Edge {
    pub source: usize,
    pub target: usize,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum Direction {
    #[default]
    Down,
    Up,
    Right,
    Left,
}

impl Direction {
    fn horizontal(self) -> bool {
        matches!(self, Self::Right | Self::Left)
    }

    fn reverse(self) -> bool {
        matches!(self, Self::Up | Self::Left)
    }

    fn cross(self, p: Point) -> f64 {
        if self.horizontal() { p.y } else { p.x }
    }

    fn along(self, p: Point) -> f64 {
        if self.horizontal() { p.x } else { p.y }
    }

    fn point(self, cross: f64, along: f64) -> Point {
        if self.horizontal() {
            Point { x: along, y: cross }
        } else {
            Point { x: cross, y: along }
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Options {
    pub direction: Direction,
    pub spacing: f64,
    pub edge_node_spacing: f64,
    pub aspect_ratio: f64,
    pub padding: Padding,
    pub separate_components: bool,
}

impl Default for Options {
    fn default() -> Self {
        Self {
            direction: Direction::Down,
            spacing: 20.0,
            edge_node_spacing: 3.0,
            aspect_ratio: 1.6_f32 as f64,
            padding: default_padding(),
            separate_components: true,
        }
    }
}

const fn default_padding() -> Padding {
    Padding {
        top: 20.0,
        right: 20.0,
        bottom: 20.0,
        left: 20.0,
    }
}

#[derive(Debug, Clone, Default, PartialEq)]
pub struct Layout {
    pub nodes: Vec<Point>,
    /// Full source-to-target point chains. Self loops are absent.
    pub edges: Vec<Option<Vec<Point>>>,
    pub width: f64,
    pub height: f64,
}

#[derive(Debug, thiserror::Error, PartialEq)]
pub enum Error {
    #[error(transparent)]
    Work(#[from] WorkError),
    #[error("ELK Mr. Tree received invalid {field} on node {index}")]
    InvalidNode { index: usize, field: &'static str },
    #[error("ELK Mr. Tree received invalid endpoint on edge {0}")]
    InvalidEdge(usize),
    #[error("ELK Mr. Tree received invalid option {0}")]
    InvalidOption(&'static str),
    #[error("ELK Mr. Tree geometry exceeded the supported numeric range")]
    NumericRange,
}

struct TreeNode {
    input: Option<usize>,
    id: usize,
    identity: String,
    super_root: bool,
    width: f64,
    height: f64,
    position: Point,
    parent: Option<usize>,
    children: Vec<usize>,
    left_sibling: Option<usize>,
    left_neighbor: Option<usize>,
    level: usize,
    level_height: f64,
    level_min: f64,
    level_max: f64,
    prelim: f64,
    modifier: f64,
}

impl TreeNode {
    fn size(&self) -> Point {
        Point {
            x: self.width,
            y: self.height,
        }
    }
    fn center(&self) -> Point {
        Point {
            x: self.position.x + self.width / 2.0,
            y: self.position.y + self.height / 2.0,
        }
    }
}

struct TreeEdge {
    input: Option<usize>,
    source: usize,
    target: usize,
    points: Vec<Point>,
}

struct Component {
    nodes: Vec<TreeNode>,
    edges: Vec<TreeEdge>,
    /// Component DFS appends incident edges on return, twice per original edge.
    edge_order: Vec<usize>,
    min: Point,
    max: Point,
    priority: i32,
}

/// Layout does not mutate input. Work interruption therefore never exposes a partial result.
pub fn layout(
    nodes: &[Node],
    edges: &[Edge],
    options: &Options,
    work: &mut dyn WorkControl,
) -> Result<Layout, Error> {
    validate(nodes, edges, options, work)?;
    if nodes.is_empty() {
        return Ok(Layout::default());
    }
    let mut components = components(nodes, edges, options.separate_components, work)?;
    for component in &mut components {
        placement::place(component, options, work)?;
        routing::route(component, options, work)?;
    }
    pack(&mut components, nodes, options, work)?;
    let mut result = Layout {
        nodes: vec![Point::default(); nodes.len()],
        edges: vec![None; edges.len()],
        ..Layout::default()
    };
    // The source importer computes its final extent around the already top-left positions.
    let mut min = Point {
        x: i32::MAX as f64,
        y: i32::MAX as f64,
    };
    let mut max = Point {
        x: i32::MIN as f64,
        y: i32::MIN as f64,
    };
    for component in components {
        for node in component.nodes {
            work.charge(1)?;
            min.x = min.x.min(node.position.x - node.width / 2.0);
            min.y = min.y.min(node.position.y - node.height / 2.0);
            max.x = max.x.max(node.position.x + node.width / 2.0);
            max.y = max.y.max(node.position.y + node.height / 2.0);
            if let Some(input) = node.input {
                result.nodes[input] = node.position;
            }
        }
        for edge in component.edges {
            if let Some(input) = edge.input {
                result.edges[input] = Some(edge.points);
            }
        }
    }
    result.width = max.x - min.x + options.padding.left + options.padding.right;
    result.height = max.y - min.y + options.padding.top + options.padding.bottom;
    if !result.width.is_finite()
        || !result.height.is_finite()
        || result
            .nodes
            .iter()
            .chain(result.edges.iter().flatten().flatten())
            .any(|p| !p.x.is_finite() || !p.y.is_finite())
    {
        return Err(Error::NumericRange);
    }
    Ok(result)
}

fn validate(
    nodes: &[Node],
    edges: &[Edge],
    options: &Options,
    work: &mut dyn WorkControl,
) -> Result<(), Error> {
    work.charge(checked_add(nodes.len(), edges.len())?)?;
    for (index, node) in nodes.iter().enumerate() {
        for (field, value) in [
            ("width", node.width),
            ("height", node.height),
            ("padding.top", node.padding.top),
            ("padding.right", node.padding.right),
            ("padding.bottom", node.padding.bottom),
            ("padding.left", node.padding.left),
        ] {
            if !value.is_finite() || value < 0.0 {
                return Err(Error::InvalidNode { index, field });
            }
        }
    }
    for (i, edge) in edges.iter().enumerate() {
        if edge.source >= nodes.len() || edge.target >= nodes.len() {
            return Err(Error::InvalidEdge(i));
        }
    }
    for (field, value) in [
        ("spacing", options.spacing),
        ("edge_node_spacing", options.edge_node_spacing),
        ("padding.top", options.padding.top),
        ("padding.right", options.padding.right),
        ("padding.bottom", options.padding.bottom),
        ("padding.left", options.padding.left),
    ] {
        if !value.is_finite() || value < 0.0 {
            return Err(Error::InvalidOption(field));
        }
    }
    if !options.aspect_ratio.is_finite() || options.aspect_ratio <= 0.0 {
        return Err(Error::InvalidOption("aspect_ratio"));
    }
    Ok(())
}

fn components(
    nodes: &[Node],
    edges: &[Edge],
    separate: bool,
    work: &mut dyn WorkControl,
) -> Result<Vec<Component>, Error> {
    work.charge(checked_add(
        checked_mul(nodes.len(), 3)?,
        checked_mul(edges.len(), 4)?,
    )?)?;
    let mut outgoing = vec![Vec::new(); nodes.len()];
    for (i, edge) in edges.iter().enumerate() {
        if edge.source != edge.target {
            outgoing[edge.source].push(i);
        }
    }
    let import_order: Vec<_> = outgoing.into_iter().flatten().collect();
    let mut incidence = vec![Vec::new(); nodes.len()];
    for &i in &import_order {
        incidence[edges[i].source].push(i);
        incidence[edges[i].target].push(i);
    }
    let mut groups = Vec::new();
    if separate {
        let mut seen = vec![false; nodes.len()];
        for root in 0..nodes.len() {
            if seen[root] {
                continue;
            }
            seen[root] = true;
            let mut members = vec![root];
            let mut order = Vec::new();
            // An explicit return event preserves the recursive source's edge append order.
            let mut stack = vec![(root, 0, false)];
            while let Some((node, cursor, returning)) = stack.pop() {
                work.charge(1)?;
                if cursor == incidence[node].len() {
                    continue;
                }
                let edge = incidence[node][cursor];
                if returning {
                    order.push(edge);
                    stack.push((node, cursor + 1, false));
                } else {
                    stack.push((node, cursor, true));
                    let other = if edges[edge].source == node {
                        edges[edge].target
                    } else {
                        edges[edge].source
                    };
                    if !seen[other] {
                        seen[other] = true;
                        members.push(other);
                        stack.push((other, 0, false));
                    }
                }
            }
            groups.push((members, order));
        }
    } else {
        groups.push(((0..nodes.len()).collect(), import_order.clone()));
    }
    let mut result = Vec::with_capacity(groups.len());
    // Reuse maps across components; do not allocate N entries for each isolated node.
    let mut local_node = vec![usize::MAX; nodes.len()];
    let mut local_edge = vec![usize::MAX; edges.len()];
    for (members, order) in groups {
        let mut local_nodes = Vec::with_capacity(members.len());
        let mut priority: i32 = 0;
        for (id, &input) in members.iter().enumerate() {
            let n = &nodes[input];
            local_node[input] = id;
            priority = priority.wrapping_add(n.priority);
            local_nodes.push(TreeNode {
                input: Some(input),
                id,
                identity: if n.label.is_empty() {
                    format!("n_{id}")
                } else {
                    format!("n_{}", n.label)
                },
                super_root: n.label == "SUPER_ROOT",
                width: n.width.max(1.0),
                height: n.height.max(1.0),
                position: Point::default(),
                parent: None,
                children: Vec::new(),
                left_sibling: None,
                left_neighbor: None,
                level: 0,
                level_height: 0.0,
                level_min: 0.0,
                level_max: 0.0,
                prelim: 0.0,
                modifier: 0.0,
            });
        }
        let mut local_edges = Vec::new();
        let mut edge_order = Vec::with_capacity(order.len());
        for &input in &order {
            if local_edge[input] == usize::MAX {
                local_edge[input] = local_edges.len();
                local_edges.push(TreeEdge {
                    input: Some(input),
                    source: local_node[edges[input].source],
                    target: local_node[edges[input].target],
                    points: Vec::new(),
                });
            }
            edge_order.push(local_edge[input]);
        }
        // Treeification sees imported outgoing order, not the component edge-list DFS order.
        for &input in &order {
            local_edge[input] = usize::MAX;
        }
        result.push(Component {
            nodes: local_nodes,
            edges: local_edges,
            edge_order,
            min: Point::default(),
            max: Point::default(),
            priority,
        });
    }
    Ok(result)
}

fn move_component(
    component: &mut Component,
    offset: Point,
    work: &mut dyn WorkControl,
) -> Result<(), Error> {
    work.charge(component.nodes.len())?;
    for node in &mut component.nodes {
        node.position.x += offset.x;
        node.position.y += offset.y;
    }
    for edge in &mut component.edges {
        work.charge(edge.points.len())?;
        for p in &mut edge.points {
            p.x += offset.x;
            p.y += offset.y;
        }
    }
    Ok(())
}

fn pack(
    components: &mut [Component],
    inputs: &[Node],
    options: &Options,
    work: &mut dyn WorkControl,
) -> Result<(), Error> {
    if components.len() > 1 {
        work.charge(checked_n_log_n(components.len())?)?;
        components.sort_by(|a, b| {
            b.priority.cmp(&a.priority).then_with(|| {
                ((a.max.x - a.min.x) * (a.max.y - a.min.y))
                    .total_cmp(&((b.max.x - b.min.x) * (b.max.y - b.min.y)))
            })
        });
        let mut total_area = 0.0;
        let mut max_width: f64 = 0.0;
        for c in components.iter() {
            max_width = max_width.max(c.max.x - c.min.x);
            total_area += (c.max.x - c.min.x) * (c.max.y - c.min.y);
        }
        max_width = max_width.max((total_area.sqrt() as f32) as f64 * options.aspect_ratio);
        let mut x = 0.0;
        let mut y = 0.0;
        let mut row_height: f64 = 0.0;
        for c in components.iter_mut() {
            let width = c.max.x - c.min.x;
            let height = c.max.y - c.min.y;
            if x + width > max_width {
                x = 0.0;
                y += row_height + options.spacing;
                row_height = 0.0;
            }
            move_component(
                c,
                Point {
                    x: x - c.min.x,
                    y: y - c.min.y,
                },
                work,
            )?;
            row_height = row_height.max(height);
            x += width + options.spacing;
        }
    }
    // Source property merging keeps the last component's pre-pack GRAPH_X/YMIN. It then
    // normalizes all components using the first child's padding, without recalculating bounds.
    let last = &components[components.len() - 1];
    let first_input = components[0].nodes[0].input.ok_or(Error::NumericRange)?;
    let padding = inputs[first_input].padding;
    let offset = Point {
        x: padding.left + padding.right - last.min.x,
        y: padding.top + padding.bottom - last.min.y,
    };
    for c in components {
        move_component(c, offset, work)?;
    }
    Ok(())
}

#[cfg(test)]
mod tests;
