//! Connected-component layout and SimpleRow packing from Eclipse ELK (EPL-2.0).
//!
//! Source references (62d5909f96fad541bc101ad52dabaece6b7eab7e):
//! - https://github.com/eclipse-elk/elk/blob/62d5909f96fad541bc101ad52dabaece6b7eab7e/plugins/org.eclipse.elk.alg.layered/src/org/eclipse/elk/alg/layered/components/ComponentsProcessor.java
//! - https://github.com/eclipse-elk/elk/blob/62d5909f96fad541bc101ad52dabaece6b7eab7e/plugins/org.eclipse.elk.alg.layered/src/org/eclipse/elk/alg/layered/components/SimpleRowGraphPlacer.java
//! - https://github.com/eclipse-elk/elk/blob/62d5909f96fad541bc101ad52dabaece6b7eab7e/plugins/org.eclipse.elk.alg.layered/src/org/eclipse/elk/alg/layered/components/AbstractGraphPlacer.java

use crate::graph::{LGraph, LLabel, LPoint, LSize, PortRef};
use crate::options::{HierarchyHandling, PortConstraints};
use crate::pipeline::PipelineResult;
use crate::work::{WorkControl, checked_add, checked_mul, checked_n_log_n, checked_sum};

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum ComponentError {
    #[error(
        "flat ELK components with free or side-fixed external ports require the unported component-group placer"
    )]
    ExternalPortGroups,
    #[error("flat ELK component processing cannot own cross-hierarchy edges")]
    HierarchyEdges,
    #[error("ELK component {kind} reference {index} is outside its component")]
    InvalidReference { kind: &'static str, index: usize },
    #[error("ELK component packing requires finite nonnegative geometry and spacing")]
    InvalidGeometry,
}

pub(crate) struct Component {
    pub(crate) graph: LGraph,
    original_nodes: Vec<usize>,
    original_edges: Vec<usize>,
    priority: i32,
}

/// The supplied graph has already passed GraphConfigurator. Component construction copies
/// properties, not a new configuration or random seed. The caller charges the arena copy.
pub(crate) fn split(
    graph: &LGraph,
    work: &mut dyn WorkControl,
) -> PipelineResult<Option<Vec<Component>>> {
    if !graph.options.separate_connected_components
        || graph.options.hierarchy_handling == HierarchyHandling::IncludeChildren
        || (graph.graph_properties.external_ports
            && matches!(
                graph.options.port_constraints,
                PortConstraints::FixedOrder
                    | PortConstraints::FixedRatio
                    | PortConstraints::FixedPos
            ))
    {
        return Ok(None);
    }
    if !graph.hierarchy_edges.is_empty() || !graph.cross_hierarchy_edges.is_empty() {
        return Err(ComponentError::HierarchyEdges.into());
    }
    let node_count = graph.layerless_nodes.len();
    work.charge(checked_sum([
        checked_mul(node_count, 8)?,
        checked_mul(graph.edges.len(), 6)?,
    ])?)?;
    let mut owners = vec![usize::MAX; node_count];
    let mut groups = Vec::<Vec<usize>>::new();
    let mut stack = Vec::new();
    for start in 0..node_count {
        if owners[start] != usize::MAX {
            continue;
        }
        let owner = groups.len();
        let mut nodes = Vec::new();
        stack.push(start);
        while let Some(index) = stack.pop() {
            let node =
                graph
                    .layerless_nodes
                    .get(index)
                    .ok_or(ComponentError::InvalidReference {
                        kind: "node",
                        index,
                    })?;
            if owners[index] != usize::MAX {
                continue;
            }
            owners[index] = owner;
            nodes.push(index);
            // LPort.getConnectedPorts visits incoming then outgoing edges; reverse the
            // pushes so this iterative traversal retains the source recursive DFS order.
            for port in node.ports.iter().rev() {
                for &edge in port.outgoing_edges.iter().rev() {
                    let edge = graph
                        .edges
                        .get(edge)
                        .ok_or(ComponentError::InvalidReference {
                            kind: "edge",
                            index: edge,
                        })?;
                    stack.push(edge.target.node);
                }
                for &edge in port.incoming_edges.iter().rev() {
                    let edge = graph
                        .edges
                        .get(edge)
                        .ok_or(ComponentError::InvalidReference {
                            kind: "edge",
                            index: edge,
                        })?;
                    stack.push(edge.source.node);
                }
            }
        }
        groups.push(nodes);
    }
    if graph.graph_properties.external_ports && groups.len() > 1 {
        return Err(ComponentError::ExternalPortGroups.into());
    }
    crate::pipeline::charge_hierarchy_work(graph, work)?;
    work.charge(checked_mul(groups.len(), graph.id.len())?)?;
    let mut components = groups
        .into_iter()
        .map(|original_nodes| {
            let mut component = graph.component_shell();
            component.layerless_nodes = original_nodes
                .iter()
                .map(|&node| graph.layerless_nodes[node].clone())
                .collect();
            let priority = component
                .layerless_nodes
                .iter()
                .fold(0i32, |sum, node| sum.wrapping_add(node.component_priority));
            Component {
                graph: component,
                original_nodes,
                original_edges: Vec::new(),
                priority,
            }
        })
        .collect::<Vec<_>>();
    for (index, edge) in graph.edges.iter().enumerate() {
        let owner = *owners
            .get(edge.source.node)
            .ok_or(ComponentError::InvalidReference {
                kind: "source node",
                index: edge.source.node,
            })?;
        if owners.get(edge.target.node) != Some(&owner) {
            return Err(ComponentError::InvalidReference {
                kind: "target node",
                index: edge.target.node,
            }
            .into());
        }
        components[owner].original_edges.push(index);
        components[owner].graph.edges.push(edge.clone());
    }
    for &node in &graph.hidden_nodes {
        let owner = component_owner(&owners, node)?;
        components[owner].graph.hidden_nodes.push(node);
    }
    for &node in &graph.replaced_external_port_dummies {
        let owner = component_owner(&owners, node)?;
        components[owner]
            .graph
            .replaced_external_port_dummies
            .push(node);
    }
    for holder in &graph.self_loop_holders {
        let owner = component_owner(&owners, holder.node)?;
        components[owner]
            .graph
            .self_loop_holders
            .push(holder.clone());
    }
    // Reuse the global maps across components: clearing only each component's entries
    // keeps this O(V + E), including a graph made entirely of isolated nodes.
    let mut node_map = vec![usize::MAX; node_count];
    let mut edge_map = vec![usize::MAX; graph.edges.len()];
    for component in &mut components {
        for (local, &original) in component.original_nodes.iter().enumerate() {
            node_map[original] = local;
        }
        for (local, &original) in component.original_edges.iter().enumerate() {
            edge_map[original] = local;
        }
        remap_references(&mut component.graph, &node_map, &edge_map)?;
        component.graph.clear_layers();
        for &original in &component.original_nodes {
            node_map[original] = usize::MAX;
        }
        for &original in &component.original_edges {
            edge_map[original] = usize::MAX;
        }
    }
    Ok(Some(components))
}

fn component_owner(owners: &[usize], node: usize) -> Result<usize, ComponentError> {
    map_index(owners, node, "node")
}

fn map_index(map: &[usize], index: usize, kind: &'static str) -> Result<usize, ComponentError> {
    map.get(index)
        .copied()
        .filter(|&mapped| mapped != usize::MAX)
        .ok_or(ComponentError::InvalidReference { kind, index })
}

fn map_optional(
    value: &mut Option<usize>,
    map: &[usize],
    kind: &'static str,
) -> Result<(), ComponentError> {
    if let Some(index) = value {
        *index = map_index(map, *index, kind)?;
    }
    Ok(())
}

fn map_list(values: &mut [usize], map: &[usize], kind: &'static str) -> Result<(), ComponentError> {
    for index in values {
        *index = map_index(map, *index, kind)?;
    }
    Ok(())
}

fn map_port(port: &mut PortRef, nodes: &[usize]) -> Result<(), ComponentError> {
    port.node = map_index(nodes, port.node, "port node")?;
    Ok(())
}

fn map_labels(labels: &mut [LLabel], edges: &[usize]) -> Result<(), ComponentError> {
    for label in labels {
        map_optional(&mut label.end_label_edge, edges, "label edge")?;
    }
    Ok(())
}

/// Maps every arena reference, including processor-created dummies and detached edges.
/// Port ordinals are node-local and are deliberately unchanged.
fn remap_references(
    graph: &mut LGraph,
    nodes: &[usize],
    edges: &[usize],
) -> Result<(), ComponentError> {
    for node in &mut graph.layerless_nodes {
        map_optional(
            &mut node.replaced_external_port_dummy,
            nodes,
            "replaced dummy",
        )?;
        map_list(&mut node.in_layer_successor_constraints, nodes, "successor")?;
        map_optional(&mut node.in_layer_layout_unit, nodes, "layout unit")?;
        map_list(
            &mut node.barycenter_associates,
            nodes,
            "barycenter associate",
        )?;
        map_optional(&mut node.origin_edge, edges, "origin edge")?;
        for port in [&mut node.long_edge_source, &mut node.long_edge_target]
            .into_iter()
            .flatten()
        {
            map_port(port, nodes)?;
        }
        map_labels(&mut node.labels, edges)?;
        map_labels(&mut node.represented_labels, edges)?;
        for port in &mut node.ports {
            port.node = map_index(nodes, port.node, "port owner")?;
            map_optional(&mut port.long_edge_target_node, nodes, "long edge target")?;
            map_list(&mut port.incoming_edges, edges, "incoming edge")?;
            map_list(&mut port.outgoing_edges, edges, "outgoing edge")?;
            map_labels(&mut port.labels, edges)?;
        }
    }
    for edge in &mut graph.edges {
        map_port(&mut edge.source, nodes)?;
        map_port(&mut edge.target, nodes)?;
        if let Some(port) = &mut edge.original_opposite_port {
            map_port(port, nodes)?;
        }
        map_labels(&mut edge.labels, edges)?;
    }
    for layer in &mut graph.layers {
        map_list(&mut layer.nodes, nodes, "layer node")?;
    }
    map_list(&mut graph.hidden_nodes, nodes, "hidden node")?;
    map_list(
        &mut graph.replaced_external_port_dummies,
        nodes,
        "external dummy",
    )?;
    for holder in &mut graph.self_loop_holders {
        holder.node = map_index(nodes, holder.node, "self loop owner")?;
        for hyper_loop in &mut holder.hyper_loops {
            for edge in &mut hyper_loop.edges {
                edge.edge = map_index(edges, edge.edge, "self loop edge")?;
            }
            if let Some(labels) = &mut hyper_loop.labels {
                for label in &mut labels.label_refs {
                    label.edge = map_index(edges, label.edge, "self loop label")?;
                }
            }
        }
    }
    let graph_id = graph.id.clone();
    graph.try_for_each_graph_mut(|current| {
        for node in &mut current.layerless_nodes {
            if let Some(origin) = &mut node.origin_port
                && origin.graph_id == graph_id
            {
                map_port(&mut origin.port, nodes)?;
            }
            for port in &mut node.ports {
                if let Some(dummy) = &mut port.port_dummy
                    && dummy.graph_id == graph_id
                {
                    dummy.node = map_index(nodes, dummy.node, "graph port dummy")?;
                }
            }
        }
        for edge in &mut current.cross_hierarchy_edges {
            if edge.graph_id == graph_id {
                edge.edge = map_index(edges, edge.edge, "hierarchy edge")?;
            }
        }
        Ok(())
    })
}

fn offset_graph(graph: &mut LGraph, delta: LPoint) {
    for node in &mut graph.layerless_nodes {
        node.position.x += delta.x;
        node.position.y += delta.y;
        for port in &node.ports {
            for &edge in &port.outgoing_edges {
                let edge = &mut graph.edges[edge];
                for point in &mut edge.bend_points {
                    point.x += delta.x;
                    point.y += delta.y;
                }
                for label in &mut edge.labels {
                    label.position.x += delta.x;
                    label.position.y += delta.y;
                }
            }
        }
    }
}

/// Places completed components, then restores the caller's original dense indices. Java
/// retains object identity while moving nodes; stable original indices are its Rust equivalent.
pub(crate) fn combine(
    target: &mut LGraph,
    mut components: Vec<Component>,
    work: &mut dyn WorkControl,
) -> PipelineResult<()> {
    let original_nodes = target.layerless_nodes.len();
    let original_edges = target.edges.len();
    let node_count = checked_sum(components.iter().map(|c| c.graph.layerless_nodes.len()))?;
    let edge_count = checked_sum(components.iter().map(|c| c.graph.edges.len()))?;
    work.charge(checked_sum([
        checked_mul(node_count, 5)?,
        checked_mul(edge_count, 5)?,
        checked_n_log_n(components.len())?,
    ])?)?;
    let size = match components.len() {
        0 => LSize::default(),
        1 => {
            let component = &mut components[0].graph;
            let offset = component.offset;
            offset_graph(component, offset);
            component.offset = LPoint::default();
            component.size
        }
        _ => {
            components.sort_by(|left, right| {
                right.priority.cmp(&left.priority).then_with(|| {
                    let area = |g: &LGraph| g.size.width * g.size.height;
                    area(&left.graph).total_cmp(&area(&right.graph))
                })
            });
            let spacing = target.options.spacing.component_component;
            let aspect_ratio = target.options.aspect_ratio;
            if !spacing.is_finite()
                || spacing < 0.0
                || !aspect_ratio.is_finite()
                || aspect_ratio < 0.0
            {
                return Err(ComponentError::InvalidGeometry.into());
            }
            let mut area = 0.0;
            let mut row_width = 0.0f64;
            for component in &components {
                let size = component.graph.size;
                if !size.width.is_finite()
                    || !size.height.is_finite()
                    || size.width < 0.0
                    || size.height < 0.0
                {
                    return Err(ComponentError::InvalidGeometry.into());
                }
                area += size.width * size.height;
                row_width = row_width.max(size.width);
            }
            row_width = row_width.max(f64::from(area.sqrt() as f32) * aspect_ratio);
            if !area.is_finite() || !row_width.is_finite() {
                return Err(ComponentError::InvalidGeometry.into());
            }
            let (mut x, mut y, mut highest, mut broadest): (f64, f64, f64, f64) =
                (0.0, 0.0, 0.0, spacing);
            for component in &mut components {
                let graph = &mut component.graph;
                if x + graph.size.width > row_width {
                    x = 0.0;
                    y += highest + spacing;
                    highest = 0.0;
                }
                offset_graph(
                    graph,
                    LPoint {
                        x: x + graph.offset.x,
                        y: y + graph.offset.y,
                    },
                );
                graph.offset = LPoint::default();
                broadest = broadest.max(x + graph.size.width);
                highest = highest.max(graph.size.height);
                x += graph.size.width + spacing;
            }
            LSize {
                width: broadest,
                height: y + highest,
            }
        }
    };
    let mut nodes = std::iter::repeat_with(|| None)
        .take(node_count)
        .collect::<Vec<_>>();
    let mut edges = std::iter::repeat_with(|| None)
        .take(edge_count)
        .collect::<Vec<_>>();
    let mut next_node = original_nodes;
    let mut next_edge = original_edges;
    let mut hidden = Vec::new();
    let mut replaced = Vec::new();
    let mut self_loops = Vec::new();
    let mut cyclic = false;
    for mut component in components {
        let mut node_map = component.original_nodes;
        let added_nodes = component.graph.layerless_nodes.len() - node_map.len();
        node_map.extend(next_node..checked_add(next_node, added_nodes)?);
        next_node += added_nodes;
        let mut edge_map = component.original_edges;
        let added_edges = component.graph.edges.len() - edge_map.len();
        edge_map.extend(next_edge..checked_add(next_edge, added_edges)?);
        next_edge += added_edges;
        component.graph.clear_layers();
        remap_references(&mut component.graph, &node_map, &edge_map)?;
        for (index, node) in node_map.into_iter().zip(component.graph.layerless_nodes) {
            nodes[index] = Some(node);
        }
        for (index, edge) in edge_map.into_iter().zip(component.graph.edges) {
            edges[index] = Some(edge);
        }
        hidden.extend(component.graph.hidden_nodes);
        replaced.extend(component.graph.replaced_external_port_dummies);
        self_loops.extend(component.graph.self_loop_holders);
        cyclic |= component.graph.cyclic;
    }
    target.layerless_nodes = nodes
        .into_iter()
        .enumerate()
        .map(|(index, node)| {
            node.ok_or(ComponentError::InvalidReference {
                kind: "combined node",
                index,
            })
        })
        .collect::<Result<_, _>>()?;
    target.edges = edges
        .into_iter()
        .enumerate()
        .map(|(index, edge)| {
            edge.ok_or(ComponentError::InvalidReference {
                kind: "combined edge",
                index,
            })
        })
        .collect::<Result<_, _>>()?;
    target.layers.clear();
    target.hidden_nodes = hidden;
    target.replaced_external_port_dummies = replaced;
    target.self_loop_holders = self_loops;
    target.cyclic = cyclic;
    target.size = size;
    // Keep the target's minimum constraint. Source copyProperties merges its property map;
    // the missing minimum on a component does not remove the target's original property.
    Ok(())
}
