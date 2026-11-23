use super::*;
use crate::work::{push, reserved};
use crate::{EdgeSpec, Endpoint, PackingPolicy, PortSideHint, Side, WidthPolicy};

pub(crate) fn length(x: f64) -> bool {
    x.is_finite() && x >= 0.0
}
pub(crate) fn measured(s: Size) -> bool {
    [s.width, s.height]
        .into_iter()
        .all(|extent| length(extent) && (extent == 0.0 || extent * 0.5 > 0.0))
}

pub(crate) fn validate_options(options: &Options) -> LayoutResult<()> {
    let s = options.spacing;
    if ![
        s.node,
        s.rank,
        s.component,
        s.edge,
        s.loop_gap,
        s.corner_clearance,
        s.port_pitch,
        s.min_port_pitch,
        s.end_label_across,
        s.end_label_along,
        s.port_sep,
    ]
    .into_iter()
    .all(length)
    {
        return Err(LayoutError::InvalidInput(
            "spacing must be finite and nonnegative",
        ));
    }
    let positive = |v: f64| v.is_finite() && v > 0.0;
    match options.packing {
        PackingPolicy::AspectTargeted { target_ratio } if !positive(target_ratio) => {
            return Err(LayoutError::InvalidInput("invalid aspect target"));
        }
        PackingPolicy::FitWidth { max_width } if !positive(max_width) => {
            return Err(LayoutError::InvalidInput("invalid packing width"));
        }
        _ => {}
    }
    if let WidthPolicy::FitWidth {
        max_width,
        turn_breakpoint,
        ..
    } = options.width_policy
        && (!positive(max_width) || !length(turn_breakpoint))
    {
        return Err(LayoutError::InvalidInput("invalid width policy"));
    }
    if let Some(q) = options.grid_quantum {
        if !positive(q) {
            return Err(LayoutError::InvalidInput("invalid grid quantum"));
        }
        return Err(LayoutError::Unsupported("grid layout"));
    }
    Ok(())
}

pub(crate) fn validate_node(node: &NodeSpec) -> LayoutResult<()> {
    if !measured(node.size)
        || node.port_span.is_some_and(|v| !length(v))
        || node.perpendicular_port_span.is_some_and(|v| !length(v))
        || node
            .anchor_y
            .is_some_and(|v| !length(v) || v > node.size.height)
        || (node.flow_relative_axis.is_some() && node.anchor_y.is_some())
        || node.order_hint.is_some_and(|h| !h.cross_key.is_finite())
        || !(-1..=1).contains(&node.pin)
    {
        return Err(LayoutError::InvalidInput(
            "invalid node geometry, parent or order hint",
        ));
    }
    Ok(())
}

pub(crate) fn validate_port(size: Size, port: PortSideHint) -> LayoutResult<()> {
    if let PortSideHint::FixedOffset { side, offset } = port {
        let face = match side {
            Side::Top | Side::Bottom => size.width,
            Side::Left | Side::Right => size.height,
        };
        if !offset.is_finite() || offset.abs() > face * 0.5 {
            return Err(LayoutError::InvalidInput("port offset outside face"));
        }
    }
    Ok(())
}

pub(crate) fn validate_edge(edge: &EdgeSpec) -> LayoutResult<()> {
    if !edge.weight.is_finite()
        || edge.weight <= 0.0
        || ![edge.label, edge.start_label, edge.end_label]
            .into_iter()
            .flatten()
            .all(measured)
        || ![edge.start_marker, edge.end_marker]
            .into_iter()
            .all(|m| length(m.length) && length(m.width))
    {
        return Err(LayoutError::InvalidInput(
            "invalid edge label, marker or weight",
        ));
    }
    Ok(())
}

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
    work.check()?;
    let n = graph.nodes.len();
    let m = graph.edges.len();
    if n > u32::MAX as usize || m > u32::MAX as usize {
        return Err(WorkError::ArithmeticOverflow.into());
    }
    if !graph.groups.is_empty() {
        return Err(LayoutError::Unsupported("subgraphs"));
    }
    validate_options(options)?;
    let s = options.spacing;
    let mut boundaries = false;
    let mut pinned = false;
    work.charge(n)?;
    for node in &graph.nodes {
        if node.parent.is_some() {
            return Err(LayoutError::InvalidInput(
                "invalid node geometry, parent or order hint",
            ));
        }
        validate_node(node)?;
        boundaries |= node.boundary.is_some();
        pinned |= node.pin != 0;
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
        validate_port(node.size, port)?;
        Ok(id.0)
    };
    work.charge(m)?;
    let mut endpoints = reserved(m)?;
    for edge in &graph.edges {
        let u = endpoint(edge.source, edge.source_port)?;
        let v = endpoint(edge.target, edge.target_port)?;
        validate_edge(edge)?;
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
            charge_sort(component.ids.len(), work)?;
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
        let depth = s
            .port_pitch
            .max(s.edge)
            .max(spec.start_marker.length)
            .max(spec.end_marker.length);
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
    charge_sort(components.len(), work)?;
    components.sort_unstable_by_key(|c| (c.nodes[0].seq, c.ids[0]));
    Ok(components)
}

pub(super) fn charge_sort(len: usize, work: &mut Work<'_>) -> LayoutResult<()> {
    if len > 1 {
        work.charge(
            len.checked_mul(len.ilog2() as usize + 1)
                .ok_or(WorkError::ArithmeticOverflow)?,
        )?;
    }
    Ok(())
}
