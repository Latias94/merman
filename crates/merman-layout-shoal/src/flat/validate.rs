use super::*;
use crate::Endpoint;
use crate::validate;
use crate::work::{push, reserved};

#[cfg(test)]
pub(super) fn components(
    graph: &Graph,
    options: &Options,
    work: &mut Work<'_>,
) -> LayoutResult<Vec<Component>> {
    collect(graph, options, false, work)
}

pub(super) fn collect(
    graph: &Graph,
    options: &Options,
    all_nodes: bool,
    work: &mut Work<'_>,
) -> LayoutResult<Vec<Component>> {
    partition(graph, options, all_nodes, work).map(|(components, _)| components)
}

pub(super) fn partition(
    graph: &Graph,
    options: &Options,
    all_nodes: bool,
    work: &mut Work<'_>,
) -> LayoutResult<(Vec<Component>, bool)> {
    work.check()?;
    let n = graph.nodes.len();
    let m = graph.edges.len();
    if n > u32::MAX as usize || m > u32::MAX as usize {
        return Err(WorkError::ArithmeticOverflow.into());
    }
    if !graph.groups.is_empty() {
        return Err(LayoutError::Unsupported("subgraphs"));
    }
    validate::options(options)?;
    let s = options.spacing;
    let mut boundaries = false;
    let mut pinned = false;
    let mut has_editor_hints = false;
    work.charge(n)?;
    for node in &graph.nodes {
        if node.parent.is_some() {
            return Err(LayoutError::InvalidInput(
                "invalid node geometry, parent or order hint",
            ));
        }
        validate::node(node)?;
        boundaries |= node.boundary.is_some();
        pinned |= node.pin != 0;
        has_editor_hints =
            has_editor_hints || node.order_hint.is_some() || node.position_hint.is_some();
    }
    let endpoint = |ep: Endpoint, port: PortSideHint| -> LayoutResult<u32> {
        let Endpoint::Node(id) = ep else {
            return Err(LayoutError::InvalidInput("missing group endpoint"));
        };
        let node = graph
            .nodes
            .get(id.0 as usize)
            .ok_or(LayoutError::InvalidInput(
                "edge endpoint is outside its input array",
            ))?;
        validate::port(node.size, port)?;
        Ok(id.0)
    };
    work.charge(m)?;
    let mut endpoints = reserved(m)?;
    for edge in &graph.edges {
        let u = endpoint(edge.source, edge.source_port)?;
        let v = endpoint(edge.target, edge.target_port)?;
        validate::edge(edge)?;
        endpoints.push((u, v));
    }
    let whole = all_nodes || boundaries || pinned;
    let adjacency = if whole {
        work.charge(n)?;
        None
    } else {
        Some((
            Csr::new(n, endpoints.iter().map(|&(u, _)| u), work)?,
            Csr::new(n, endpoints.iter().map(|&(_, v)| v), work)?,
        ))
    };
    let mut owner = filled(n, if whole { 0 } else { u32::MAX })?;
    let mut local = filled(n, 0u32)?;
    let mut stack = reserved(if whole { 0 } else { n })?;
    let mut components = Vec::new();
    for root in 0..n {
        if !whole && owner[root] != u32::MAX {
            continue;
        }
        let mut component = Component {
            boundaries,
            pinned,
            ..Component::default()
        };
        if let Some((succ, pred)) = &adjacency {
            let index = components.len() as u32;
            stack.push(root as u32);
            owner[root] = index;
            while let Some(v) = stack.pop() {
                work.visit(1)?;
                push(&mut component.ids, v)?;
                for &e in succ.at(v).iter().chain(pred.at(v)) {
                    work.visit(1)?;
                    let (a, b) = endpoints[e as usize];
                    let next = if a == v { b } else { a };
                    if owner[next as usize] == u32::MAX {
                        owner[next as usize] = index;
                        stack.push(next);
                    }
                }
            }
            work.charge_sort(component.ids.len())?;
            component
                .ids
                .sort_unstable_by_key(|&v| (graph.nodes[v as usize].seq, v));
        } else {
            component.ids = reserved(n)?;
            component.ids.extend(0..n as u32);
        }
        for (i, &v) in component.ids.iter().enumerate() {
            local[v as usize] = i as u32;
            let mut node = graph.nodes[v as usize];
            node.order_hint = node.order_hint_in(graph.direction);
            component.group_proxies |= node.group_proxy;
            push(&mut component.nodes, node)?;
        }
        component.margin_left = filled(component.ids.len(), 0.0)?;
        component.margin_right = filled(component.ids.len(), 0.0)?;
        push(&mut components, component)?;
        if whole {
            break;
        }
    }
    let mut loop_count = filled(n, 0usize)?;
    for (e, &(a, b)) in endpoints.iter().enumerate() {
        let c = &mut components[owner[a as usize] as usize];
        let spec = &graph.edges[e];
        let from = local[a as usize];
        let to = local[b as usize];
        if a != b {
            push(
                &mut c.links,
                Link {
                    from,
                    to,
                    edge: e as u32,
                    span: spec.min_rank_span.max(1),
                    weight: spec.weight,
                },
            )?;
            continue;
        }
        let right = loop_count[a as usize].is_multiple_of(2);
        loop_count[a as usize] += 1;
        let margin = if right {
            &mut c.margin_right
        } else {
            &mut c.margin_left
        };
        let label = canonical_size(graph.direction, spec.label.unwrap_or_default());
        let runway = spec.start_marker.length.max(spec.end_marker.length);
        let turn_room = s.port_pitch.max(8.0);
        let depth = s.port_pitch.max(s.edge).max(runway + turn_room);
        margin[from as usize] += depth + label.width + s.loop_gap;
        let reach = margin[from as usize];
        if !reach.is_finite() {
            return Err(LayoutError::InvalidInput(
                "self-loop clearance exceeds finite coordinates",
            ));
        }
        push(
            &mut c.loops,
            Loop {
                edge: e as u32,
                node: from,
                right,
                reach,
            },
        )?;
    }
    work.charge_sort(components.len())?;
    components.sort_unstable_by_key(|c| (c.nodes[0].seq, c.ids[0]));
    Ok((components, has_editor_hints))
}
