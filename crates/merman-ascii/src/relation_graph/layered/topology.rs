use super::super::RelationGraphBox;
use super::boxes::{LayeredRelationEdge, LayeredRelationError, LayeredRelationPlanningError};
use crate::resource::ResourceContext;
use std::collections::{BTreeSet, HashMap};

/// Ranks a directed component without changing any semantic edge endpoints.
///
/// Iterative Kosaraju traversal identifies strongly connected components. The
/// condensation graph determines component order; a stable directed DFS unfolds
/// each component into ranks. Edges going back through those ranks remain real
/// edges and are routed through private exterior channels.
pub(super) fn directed_relation_levels(
    boxes: &[&RelationGraphBox],
    edges: &[LayeredRelationEdge],
    resources: &ResourceContext,
) -> Result<HashMap<String, usize>, LayeredRelationPlanningError> {
    resources.checkpoint()?;
    let node_work = resources.checked_work_mul(boxes.len(), 12)?;
    let edge_work = resources.checked_work_mul(edges.len(), 8)?;
    resources.charge_layout_work(resources.checked_work_add(node_work, edge_work)?)?;
    resources.charge_layout_work(super::comparison_sort_work(boxes.len(), resources)?)?;

    let mut ordered = Vec::new();
    ordered
        .try_reserve_exact(boxes.len())
        .map_err(|_| super::super::layout_allocation_failed())?;
    ordered.extend(boxes.iter().copied());
    ordered.sort_unstable_by_key(|node| node.id());
    let mut index = HashMap::new();
    index
        .try_reserve(ordered.len())
        .map_err(|_| super::super::layout_allocation_failed())?;
    for (ordinal, node) in ordered.iter().enumerate() {
        resources.checkpoint()?;
        if index.insert(node.id(), ordinal).is_some() {
            return Err(LayeredRelationError::MissingEndpoint.into());
        }
    }

    let mut outgoing = adjacency(ordered.len())?;
    let mut incoming = adjacency(ordered.len())?;
    for edge in edges {
        resources.checkpoint()?;
        let source = *index
            .get(edge.route_source_id())
            .ok_or(LayeredRelationError::MissingEndpoint)?;
        let target = *index
            .get(edge.route_target_id())
            .ok_or(LayeredRelationError::MissingEndpoint)?;
        outgoing[source]
            .try_reserve(1)
            .map_err(|_| super::super::layout_allocation_failed())?;
        incoming[target]
            .try_reserve(1)
            .map_err(|_| super::super::layout_allocation_failed())?;
        outgoing[source].push(target);
        incoming[target].push(source);
    }
    for neighbors in outgoing.iter_mut().chain(incoming.iter_mut()) {
        resources.checkpoint()?;
        resources.charge_layout_work(super::comparison_sort_work(neighbors.len(), resources)?)?;
        neighbors.sort_unstable();
    }

    let mut seen = filled(ordered.len(), false)?;
    let mut finished = Vec::new();
    finished
        .try_reserve_exact(ordered.len())
        .map_err(|_| super::super::layout_allocation_failed())?;
    let mut stack = Vec::new();
    stack
        .try_reserve_exact(ordered.len())
        .map_err(|_| super::super::layout_allocation_failed())?;
    for root in 0..ordered.len() {
        resources.checkpoint()?;
        if seen[root] {
            continue;
        }
        seen[root] = true;
        stack.push((root, 0));
        while let Some((node, next)) = stack.last_mut() {
            resources.checkpoint()?;
            resources.charge_layout_work(1)?;
            if let Some(&child) = outgoing[*node].get(*next) {
                *next += 1;
                if !seen[child] {
                    seen[child] = true;
                    stack.push((child, 0));
                }
            } else {
                finished.push(*node);
                stack.pop();
            }
        }
    }

    let mut component = filled(ordered.len(), usize::MAX)?;
    let mut members: Vec<Vec<usize>> = Vec::new();
    members
        .try_reserve_exact(ordered.len())
        .map_err(|_| super::super::layout_allocation_failed())?;
    let mut pending = Vec::new();
    pending
        .try_reserve_exact(ordered.len())
        .map_err(|_| super::super::layout_allocation_failed())?;
    for root in finished.into_iter().rev() {
        resources.checkpoint()?;
        if component[root] != usize::MAX {
            continue;
        }
        let id = members.len();
        let mut group = Vec::new();
        component[root] = id;
        pending.push(root);
        while let Some(node) = pending.pop() {
            resources.checkpoint()?;
            resources.charge_layout_work(1)?;
            group
                .try_reserve(1)
                .map_err(|_| super::super::layout_allocation_failed())?;
            group.push(node);
            for &parent in incoming[node].iter().rev() {
                resources.checkpoint()?;
                resources.charge_layout_work(1)?;
                if component[parent] == usize::MAX {
                    component[parent] = id;
                    pending.push(parent);
                }
            }
        }
        // IDs choose a deterministic DFS root; the directed edges determine
        // the remaining order, so renaming a ring does not create extra returns.
        resources.charge_layout_work(super::comparison_sort_work(group.len(), resources)?)?;
        group.sort_unstable();
        members.push(group);
    }

    resources.charge_layout_work(seen.len())?;
    seen.fill(false);
    for (id, group) in members.iter_mut().enumerate() {
        resources.checkpoint()?;
        if group.len() <= 1 {
            continue;
        }
        resources.charge_layout_work(group.len())?;
        let mut preorder = Vec::new();
        preorder
            .try_reserve_exact(group.len())
            .map_err(|_| super::super::layout_allocation_failed())?;
        let root = group[0];
        seen[root] = true;
        preorder.push(root);
        stack.push((root, 0));
        while let Some((node, next)) = stack.last_mut() {
            resources.checkpoint()?;
            resources.charge_layout_work(1)?;
            if let Some(&child) = outgoing[*node].get(*next) {
                *next += 1;
                if component[child] == id && !seen[child] {
                    seen[child] = true;
                    preorder.push(child);
                    stack.push((child, 0));
                }
            } else {
                stack.pop();
            }
        }
        *group = preorder;
    }

    let mut degrees = filled(members.len(), 0usize)?;
    let mut children = adjacency(members.len())?;
    for (source, neighbors) in outgoing.iter().enumerate() {
        for &target in neighbors {
            resources.checkpoint()?;
            if component[source] != component[target] {
                let parent = component[source];
                let child = component[target];
                children[parent]
                    .try_reserve(1)
                    .map_err(|_| super::super::layout_allocation_failed())?;
                children[parent].push(child);
                degrees[child] = resources.checked_work_add(degrees[child], 1)?;
            }
        }
    }
    let mut ready = BTreeSet::new();
    for (id, degree) in degrees.iter().enumerate() {
        if *degree == 0 {
            ready.insert((members[id][0], id));
        }
    }
    let mut starts = filled(members.len(), 0usize)?;
    let mut ranks = filled(ordered.len(), 0usize)?;
    while let Some((_, id)) = ready.pop_first() {
        resources.checkpoint()?;
        resources.charge_layout_work(1)?;
        for (offset, &node) in members[id].iter().enumerate() {
            let rank = resources.checked_grid_add(starts[id], offset)?;
            resources.check_nesting_depth(rank)?;
            ranks[node] = rank;
        }
        let next = resources.checked_grid_add(starts[id], members[id].len())?;
        for &child in &children[id] {
            resources.checkpoint()?;
            resources.charge_layout_work(1)?;
            starts[child] = starts[child].max(next);
            degrees[child] -= 1;
            if degrees[child] == 0 {
                ready.insert((members[child][0], child));
            }
        }
    }
    let mut levels = HashMap::new();
    levels
        .try_reserve(ordered.len())
        .map_err(|_| super::super::layout_allocation_failed())?;
    for (node, rank) in ordered.into_iter().zip(ranks) {
        resources.checkpoint()?;
        levels.insert(node.id().to_string(), rank);
    }
    Ok(levels)
}

fn adjacency(len: usize) -> crate::Result<Vec<Vec<usize>>> {
    filled(len, Vec::new())
}

fn filled<T: Clone>(len: usize, value: T) -> crate::Result<Vec<T>> {
    let mut values = Vec::new();
    values
        .try_reserve_exact(len)
        .map_err(|_| super::super::layout_allocation_failed())?;
    values.resize(len, value);
    Ok(values)
}
