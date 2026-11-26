//! Mermaid compound graph assembly, anchor selection, and extraction for the Dagre adapter.

use dugong::graphlib::{Graph, GraphOptions, is_javascript_array_index};
use dugong::{EdgeLabel, GraphLabel, NodeLabel, RankDir};
use merman_render_core::{Error, Result, layout_work::OperationLayoutWorkControl};
use std::collections::{HashMap, HashSet};
use std::num::NonZeroUsize;

pub fn rank_dir_from_flow(direction: &str) -> RankDir {
    let direction = direction.trim();
    if direction.eq_ignore_ascii_case("BT") {
        RankDir::BT
    } else if direction.eq_ignore_ascii_case("LR") {
        RankDir::LR
    } else if direction.eq_ignore_ascii_case("RL") {
        RankDir::RL
    } else {
        RankDir::TB
    }
}

pub fn normalize_dir(s: &str) -> String {
    s.trim().to_uppercase()
}

pub fn toggled_dir(parent: &str) -> String {
    let parent = normalize_dir(parent);
    if parent == "TB" || parent == "TD" {
        "LR".to_string()
    } else {
        "TB".to_string()
    }
}

fn flow_dir_from_rankdir(rankdir: RankDir) -> &'static str {
    match rankdir {
        RankDir::TB => "TB",
        RankDir::BT => "BT",
        RankDir::LR => "LR",
        RankDir::RL => "RL",
    }
}

pub fn lowest_common_parent(
    graph: &Graph<NodeLabel, EdgeLabel, GraphLabel>,
    a: &str,
    b: &str,
    work_control: &mut OperationLayoutWorkControl,
) -> Result<Option<String>> {
    if !graph.options().compound {
        return Ok(None);
    }

    let mut ancestors = HashSet::new();
    let mut current = graph.parent(a);
    while let Some(parent) = current {
        work_control.charge_adapter(1)?;
        ancestors.insert(parent.to_string());
        current = graph.parent(parent);
    }

    let mut current = graph.parent(b);
    while let Some(parent) = current {
        work_control.charge_adapter(1)?;
        if ancestors.contains(parent) {
            return Ok(Some(parent.to_string()));
        }
        current = graph.parent(parent);
    }

    Ok(None)
}

pub fn extract_descendants(
    id: &str,
    graph: &Graph<NodeLabel, EdgeLabel, GraphLabel>,
    work_control: &mut OperationLayoutWorkControl,
) -> Result<Vec<String>> {
    let initial_children = child_id_snapshot_work_upper_bound(graph, id, work_control)?;
    work_control.charge_adapter(initial_children)?;
    let mut out = Vec::new();
    let mut visited = HashSet::new();
    let mut stack = graph
        .children_iter(id)
        .map(str::to_string)
        .collect::<Vec<_>>();
    while let Some(node) = stack.pop() {
        work_control.charge_adapter(1)?;
        if !visited.insert(node.clone()) {
            continue;
        }
        let child_count = child_id_snapshot_work_upper_bound(graph, &node, work_control)?;
        work_control.charge_adapter(child_count)?;
        stack.extend(graph.children_iter(&node).map(str::to_string));
        out.push(node);
    }

    Ok(out)
}

#[derive(Debug, Clone)]
struct FlowchartClusterDbEntry {
    anchor_id: String,
    external_connections: bool,
}

fn flowchart_has_common_edge(
    graph: &Graph<NodeLabel, EdgeLabel, GraphLabel>,
    cluster_id: &str,
    leaf_id: &str,
    work_control: &mut OperationLayoutWorkControl,
) -> Result<bool> {
    debug_assert_ne!(cluster_id, leaf_id);
    let has_edges = graph.edge_count() != 0;
    work_control.charge_adapter(usize::from(has_edges))?;
    if !has_edges {
        return Ok(false);
    }
    let (Some(cluster_ix), Some(leaf_ix)) = (graph.node_ix(cluster_id), graph.node_ix(leaf_id))
    else {
        return Ok(false);
    };
    // Mermaid rewrites the source of cluster edges to the leaf but keeps the target.
    // An unchanged incoming edge can only match if it starts at the leaf.
    if graph.has_pinned_successor_ix(leaf_ix, cluster_ix) {
        return Ok(true);
    }
    graph.try_has_common_pinned_successor_ix(cluster_ix, leaf_ix, || work_control.charge_adapter(1))
}

pub fn flowchart_find_non_cluster_child(
    id: &str,
    graph: &Graph<NodeLabel, EdgeLabel, GraphLabel>,
    cluster_id: &str,
    work_control: &mut OperationLayoutWorkControl,
) -> Result<Option<String>> {
    let child_count = graph.child_count(id);
    if child_count == 0 {
        return Ok(Some(id.to_string()));
    }
    work_control.charge_adapter(child_id_snapshot_work_upper_bound(graph, id, work_control)?)?;
    let mut reserve: Option<String> = None;
    let mut visited: HashSet<String> = HashSet::new();
    let mut stack = graph
        .children(id)
        .into_iter()
        .rev()
        .map(str::to_string)
        .collect::<Vec<_>>();

    while let Some(node) = stack.pop() {
        work_control.charge_adapter(1)?;
        if !visited.insert(node.clone()) {
            continue;
        }
        let child_count = graph.child_count(&node);
        if child_count != 0 {
            work_control.charge_adapter(child_id_snapshot_work_upper_bound(
                graph,
                &node,
                work_control,
            )?)?;
            stack.extend(graph.children(&node).into_iter().rev().map(str::to_string));
            continue;
        }
        if flowchart_has_common_edge(graph, cluster_id, &node, work_control)? {
            reserve = Some(node);
        } else {
            return Ok(Some(node));
        }
    }

    Ok(reserve)
}

fn flowchart_is_node_in_extractable_cluster(
    graph: &Graph<NodeLabel, EdgeLabel, GraphLabel>,
    node_id: &str,
    root_id: &str,
    cluster_db: &HashMap<String, FlowchartClusterDbEntry>,
    work_control: &mut OperationLayoutWorkControl,
) -> Result<bool> {
    let mut parent = graph.parent(node_id);
    while let Some(parent_id) = parent {
        work_control.charge_adapter(1)?;
        if parent_id == root_id {
            break;
        }
        if cluster_db
            .get(parent_id)
            .is_some_and(|entry| !entry.external_connections)
        {
            return Ok(true);
        }
        parent = graph.parent(parent_id);
    }
    Ok(false)
}

fn flowchart_find_safe_anchor_node(
    graph: &Graph<NodeLabel, EdgeLabel, GraphLabel>,
    cluster_id: &str,
    excluded_cluster: &str,
    descendants: &HashMap<String, Vec<String>>,
    cluster_db: &HashMap<String, FlowchartClusterDbEntry>,
    work_control: &mut OperationLayoutWorkControl,
) -> Result<Option<String>> {
    work_control.charge_adapter(graph.child_order_slot_count(cluster_id))?;
    for child in graph.children_iter(cluster_id) {
        if child == excluded_cluster {
            continue;
        }
        if let Some(excluded_descendants) = descendants.get(excluded_cluster) {
            let mut excluded = false;
            for descendant in excluded_descendants {
                work_control.charge_adapter(1)?;
                if descendant == child {
                    excluded = true;
                    break;
                }
            }
            if excluded {
                continue;
            }
        }

        let Some(candidate) =
            flowchart_find_non_cluster_child(child, graph, cluster_id, work_control)?
        else {
            continue;
        };
        if !flowchart_is_node_in_extractable_cluster(
            graph,
            &candidate,
            cluster_id,
            cluster_db,
            work_control,
        )? {
            return Ok(Some(candidate));
        }
    }
    Ok(None)
}

pub fn adjust_flowchart_clusters_and_edges(
    graph: &mut Graph<NodeLabel, EdgeLabel, GraphLabel>,
    work_control: &mut OperationLayoutWorkControl,
) -> Result<std::collections::HashMap<String, bool>> {
    use serde_json::Value;

    fn is_descendant(
        node_id: &str,
        cluster_id: &str,
        descendants: &HashMap<String, Vec<String>>,
        work_control: &mut OperationLayoutWorkControl,
    ) -> Result<bool> {
        let Some(cluster_descendants) = descendants.get(cluster_id) else {
            return Ok(false);
        };
        for descendant in cluster_descendants {
            work_control.charge_adapter(1)?;
            if descendant == node_id {
                return Ok(true);
            }
        }
        Ok(false)
    }

    let node_snapshot_work = node_id_snapshot_work_upper_bound(graph, work_control)?;
    work_control.charge_adapter(node_snapshot_work)?;
    let node_ids = graph.node_ids();
    let mut descendants: HashMap<String, Vec<String>> = HashMap::new();
    let mut cluster_db: std::collections::HashMap<String, FlowchartClusterDbEntry> =
        std::collections::HashMap::new();
    let mut cluster_ids = Vec::new();
    work_control.charge_adapter(node_ids.len())?;
    for id in node_ids {
        if graph.child_count(&id) == 0 {
            continue;
        }
        descendants.insert(id.clone(), extract_descendants(&id, graph, work_control)?);
        let anchor_id = flowchart_find_non_cluster_child(&id, graph, &id, work_control)?
            .unwrap_or_else(|| id.clone());
        cluster_ids.push(id.clone());
        cluster_db.insert(
            id,
            FlowchartClusterDbEntry {
                anchor_id,
                external_connections: false,
            },
        );
    }

    let tombstones = graph.edge_slot_count() - graph.edge_count();
    for id in &cluster_ids {
        if tombstones != 0 {
            // The iterator skips removed slots internally, before the charge for each live edge.
            work_control.charge_adapter(tombstones)?;
        }
        let mut has_external = false;
        for edge in graph.edges() {
            work_control.charge_adapter(1)?;
            let v_is_descendant = is_descendant(&edge.v, id, &descendants, work_control)?;
            let w_is_descendant = is_descendant(&edge.w, id, &descendants, work_control)?;
            if v_is_descendant ^ w_is_descendant {
                has_external = true;
                break;
            }
        }
        if let Some(entry) = cluster_db.get_mut(id) {
            entry.external_connections = has_external;
        }
    }

    for id in &cluster_ids {
        let Some(non_cluster_child) = cluster_db.get(id).map(|e| e.anchor_id.clone()) else {
            continue;
        };
        let parent = graph.parent(&non_cluster_child);
        if parent.is_some_and(|p| p != id.as_str())
            && parent.is_some_and(|p| cluster_db.contains_key(p))
            && parent.is_some_and(|p| !cluster_db.get(p).is_some_and(|e| e.external_connections))
            && let Some(p) = parent
            && let Some(entry) = cluster_db.get_mut(id)
        {
            entry.anchor_id = p.to_string();
        }

        work_control.charge_adapter(graph.edge_slot_count())?;
        let mut has_direct_outgoing_edge = false;
        for edge in graph.edges() {
            has_direct_outgoing_edge |= edge.v == *id;
        }
        let needs_safe_anchor = cluster_db
            .get(id)
            .is_some_and(|entry| entry.external_connections)
            && has_direct_outgoing_edge
            && flowchart_is_node_in_extractable_cluster(
                graph,
                &non_cluster_child,
                id,
                &cluster_db,
                work_control,
            )?;
        if needs_safe_anchor
            && let Some(excluded_cluster) = graph.parent(&non_cluster_child)
            && let Some(safe_anchor) = flowchart_find_safe_anchor_node(
                graph,
                id,
                excluded_cluster,
                &descendants,
                &cluster_db,
                work_control,
            )?
            && let Some(entry) = cluster_db.get_mut(id)
        {
            entry.anchor_id = safe_anchor;
        }
    }

    fn get_anchor_id(
        id: &str,
        cluster_db: &std::collections::HashMap<String, FlowchartClusterDbEntry>,
    ) -> String {
        let Some(entry) = cluster_db.get(id) else {
            return id.to_string();
        };
        if !entry.external_connections {
            return id.to_string();
        }
        entry.anchor_id.clone()
    }

    let edge_snapshot_work =
        work_control.checked_add(graph.edge_slot_count(), graph.edge_count())?;
    work_control.charge_adapter(edge_snapshot_work)?;
    let edge_keys = graph.edge_keys();
    for ek in edge_keys {
        if !cluster_db.contains_key(&ek.v) && !cluster_db.contains_key(&ek.w) {
            continue;
        }

        let Some(mut edge_label) = graph.edge_by_key(&ek).cloned() else {
            continue;
        };

        let v = get_anchor_id(&ek.v, &cluster_db);
        let w = get_anchor_id(&ek.w, &cluster_db);

        // Match Mermaid `adjustClustersAndEdges`: edges that touch cluster nodes are removed and
        // re-inserted even when their endpoints do not change. This affects edge iteration order
        // and therefore cycle-breaking determinism in Dagre's acyclic pass.
        let _ = graph.remove_edge_key(&ek);

        if v != ek.v {
            if let Some(parent) = graph.parent(&v)
                && let Some(entry) = cluster_db.get_mut(parent)
            {
                entry.external_connections = true;
            }
            edge_label
                .extras
                .insert("fromCluster".to_string(), Value::String(ek.v.clone()));
        }

        if w != ek.w {
            if let Some(parent) = graph.parent(&w)
                && let Some(entry) = cluster_db.get_mut(parent)
            {
                entry.external_connections = true;
            }
            edge_label
                .extras
                .insert("toCluster".to_string(), Value::String(ek.w.clone()));
        }

        graph.set_edge_named(v, w, ek.name, Some(edge_label));
    }

    Ok(cluster_db
        .into_iter()
        .map(|(id, entry)| (id, entry.external_connections))
        .collect())
}

pub fn checked_n_log_n(value: usize, work_control: &OperationLayoutWorkControl) -> Result<usize> {
    if value <= 1 {
        return Ok(0);
    }
    let passes = usize::BITS as usize - (value - 1).leading_zeros() as usize;
    work_control.checked_mul(value, passes)
}

pub fn ordered_key_update_work_upper_bound(
    entry_bound: usize,
    update_count: usize,
    work_control: &OperationLayoutWorkControl,
) -> Result<usize> {
    let height = if entry_bound <= 1 {
        1
    } else {
        let log = usize::BITS as usize - (entry_bound - 1).leading_zeros() as usize;
        work_control.checked_add(log, 1)?
    };
    work_control.checked_mul(update_count, height)
}

pub fn node_id_snapshot_work_upper_bound(
    graph: &Graph<NodeLabel, EdgeLabel, GraphLabel>,
    work_control: &OperationLayoutWorkControl,
) -> Result<usize> {
    work_control.checked_add(graph.node_order_slot_count(), graph.node_count())
}

pub fn child_id_snapshot_work_upper_bound(
    graph: &Graph<NodeLabel, EdgeLabel, GraphLabel>,
    parent: &str,
    work_control: &OperationLayoutWorkControl,
) -> Result<usize> {
    work_control.checked_add(
        graph.child_order_slot_count(parent),
        graph.child_count(parent),
    )
}

pub fn charge_node_insertion_ordering(
    graph: &Graph<NodeLabel, EdgeLabel, GraphLabel>,
    node_id: &str,
    work_control: &mut OperationLayoutWorkControl,
) -> Result<()> {
    if graph.has_node(node_id) || !is_javascript_array_index(node_id) {
        return Ok(());
    }
    let final_node_count = work_control.checked_add(graph.node_count(), 1)?;
    let ordered_maps = 1 + usize::from(graph.options().compound);
    let work = ordered_key_update_work_upper_bound(final_node_count, ordered_maps, work_control)?;
    work_control.charge_adapter(work)
}

pub fn charge_edge_endpoint_insertions(
    graph: &Graph<NodeLabel, EdgeLabel, GraphLabel>,
    v: &str,
    w: &str,
    work_control: &mut OperationLayoutWorkControl,
) -> Result<()> {
    let v_missing = !graph.has_node(v);
    let w_missing = w != v && !graph.has_node(w);
    let missing_count = usize::from(v_missing) + usize::from(w_missing);
    if missing_count == 0 {
        return Ok(());
    }

    let numeric_count = usize::from(v_missing && is_javascript_array_index(v))
        + usize::from(w_missing && is_javascript_array_index(w));
    let final_node_count = work_control.checked_add(graph.node_count(), missing_count)?;
    let ordered_updates =
        work_control.checked_mul(numeric_count, 1 + usize::from(graph.options().compound))?;
    let ordered_work =
        ordered_key_update_work_upper_bound(final_node_count, ordered_updates, work_control)?;
    work_control.charge_adapter(work_control.checked_add(missing_count, ordered_work)?)
}

pub fn remove_node_work_upper_bound(
    graph: &Graph<NodeLabel, EdgeLabel, GraphLabel>,
    node_id: &str,
    work_control: &OperationLayoutWorkControl,
) -> Result<usize> {
    let node_slots = graph.node_slot_count();
    let edge_slots = graph.edge_slot_count();
    let incident_slots = if graph.options().directed {
        work_control.checked_mul(edge_slots, 2)?
    } else {
        edge_slots
    };
    let cache_and_tombstone_work = work_control.checked_add(
        work_control.checked_mul(node_slots, 7)?,
        work_control.checked_mul(edge_slots, 6)?,
    )?;
    let incident_work = work_control.checked_add(
        work_control.checked_mul(incident_slots, 2)?,
        checked_n_log_n(incident_slots, work_control)?,
    )?;
    let compound = graph.options().compound;
    let previous_order_slots = if compound {
        graph.parent(node_id).map_or_else(
            || graph.root_child_order_slot_count(),
            |parent| graph.child_order_slot_count(parent),
        )
    } else {
        0
    };
    let promoted_order_slots = if compound {
        graph.child_order_slot_count(node_id)
    } else {
        0
    };
    let object_order_scan_work = work_control.checked_add(
        graph.node_order_slot_count(),
        work_control.checked_add(previous_order_slots, promoted_order_slots)?,
    )?;
    let numeric_node = usize::from(is_javascript_array_index(node_id));
    let promoted_numeric = if compound {
        graph.array_index_child_count(node_id)
    } else {
        0
    };
    let numeric_updates = work_control.checked_add(
        work_control.checked_mul(numeric_node, 1 + usize::from(compound))?,
        promoted_numeric,
    )?;
    let ordered_map_work = ordered_key_update_work_upper_bound(
        graph.array_index_node_count(),
        numeric_updates,
        work_control,
    )?;
    work_control.checked_add(
        work_control.checked_add(
            work_control.checked_add(cache_and_tombstone_work, incident_work)?,
            object_order_scan_work,
        )?,
        ordered_map_work,
    )
}

fn set_parent_controlled(
    graph: &mut Graph<NodeLabel, EdgeLabel, GraphLabel>,
    child: &str,
    parent: &str,
    work_control: &mut OperationLayoutWorkControl,
) -> Result<()> {
    let child_exists = graph.has_node(child);
    let parent_exists = graph.has_node(parent);
    let inserted_nodes = if child == parent {
        usize::from(!child_exists)
    } else {
        usize::from(!child_exists) + usize::from(!parent_exists)
    };
    let final_node_slots = work_control.checked_add(graph.node_slot_count(), inserted_nodes)?;
    let child_is_numeric = is_javascript_array_index(child);
    let relation_changes =
        !child_exists || graph.parent(child) != Some(parent) || !child_is_numeric;
    let previous_order_slots = if relation_changes && !child_is_numeric {
        if child_exists {
            graph.parent(child).map_or_else(
                || graph.root_child_order_slot_count(),
                |previous_parent| graph.child_order_slot_count(previous_parent),
            )
        } else {
            work_control.checked_add(graph.root_child_order_slot_count(), 1)?
        }
    } else {
        0
    };
    let inserted_numeric_nodes = if child == parent {
        usize::from(!child_exists && child_is_numeric)
    } else {
        usize::from(!child_exists && child_is_numeric)
            + usize::from(!parent_exists && is_javascript_array_index(parent))
    };
    let final_numeric_nodes =
        work_control.checked_add(graph.array_index_node_count(), inserted_numeric_nodes)?;
    let insertion_updates = work_control.checked_mul(
        inserted_numeric_nodes,
        1 + usize::from(graph.options().compound),
    )?;
    let relation_updates = if relation_changes && child_is_numeric {
        2
    } else {
        0
    };
    let ordered_map_work = ordered_key_update_work_upper_bound(
        final_numeric_nodes,
        work_control.checked_add(insertion_updates, relation_updates)?,
        work_control,
    )?;
    let base_work = work_control.checked_add(
        final_node_slots,
        work_control.checked_add(inserted_nodes, 1)?,
    )?;
    let work = work_control.checked_add(
        work_control.checked_add(base_work, previous_order_slots)?,
        ordered_map_work,
    )?;
    work_control.charge_adapter(work)?;
    graph.set_parent_ref(child, parent);
    Ok(())
}

struct ClusterCopyDestination {
    from_inside: bool,
    last_source: Option<usize>,
}

#[derive(Default)]
struct ClusterCopyEdges {
    // Internal edges have no destination index. A one-based index keeps each slot two words.
    slots: Vec<(usize, Option<NonZeroUsize>)>,
    destinations: Vec<ClusterCopyDestination>,
}

fn cluster_copy_edge_snapshot(
    graph: &Graph<NodeLabel, EdgeLabel, GraphLabel>,
    root_id: &str,
    descendants: &[String],
    work_control: &mut OperationLayoutWorkControl,
) -> Result<ClusterCopyEdges> {
    let mut edges = ClusterCopyEdges::default();
    if descendants.is_empty() || graph.edge_count() == 0 {
        return Ok(edges);
    }

    let mut members = HashSet::new();
    for descendant in descendants {
        work_control.charge_adapter(1)?;
        members.insert(descendant.as_str());
    }

    let mut destinations = None;
    let slot_count = graph.edge_slot_count();
    work_control.charge_adapter(slot_count)?;
    for edge_ix in 0..slot_count {
        if edge_ix % 256 == 0 {
            work_control.charge_adapter(0)?;
        }
        let Some(key) = graph.edge_key_by_ix(edge_ix) else {
            continue;
        };
        if key.v == root_id || key.w == root_id {
            continue;
        }
        work_control.charge_adapter(2)?;
        let v_in = members.contains(key.v.as_str());
        let w_in = members.contains(key.w.as_str());
        if !v_in && !w_in {
            continue;
        }
        work_control.charge_adapter(1)?;
        let destination = if v_in && w_in {
            None
        } else {
            // With a fixed root in a directed graph, direction, outside endpoint and name
            // identify the complete rewritten key. Keep input order independent of the map.
            let outside = if v_in { key.w.as_str() } else { key.v.as_str() };
            *destinations
                .get_or_insert_with(HashMap::new)
                .entry((v_in, outside, key.name.as_deref()))
                .or_insert_with(|| {
                    edges.destinations.push(ClusterCopyDestination {
                        from_inside: v_in,
                        last_source: None,
                    });
                    NonZeroUsize::new(edges.destinations.len())
                })
        };
        edges.slots.push((edge_ix, destination));
    }
    Ok(edges)
}

pub fn copy_cluster(
    cluster_id: &str,
    graph: &mut Graph<NodeLabel, EdgeLabel, GraphLabel>,
    new_graph: &mut Graph<NodeLabel, EdgeLabel, GraphLabel>,
    root_id: &str,
    descendants: &HashMap<String, Vec<String>>,
    work_control: &mut OperationLayoutWorkControl,
) -> Result<()> {
    #[derive(Debug)]
    struct CopyFrame {
        node: String,
        owner_cluster: String,
        expanded: bool,
    }

    let mut nodes: Vec<(String, String)> = Vec::new();
    let mut visited: HashSet<String> = HashSet::new();
    let mut stack: Vec<CopyFrame> = Vec::new();
    if cluster_id != root_id {
        stack.push(CopyFrame {
            node: cluster_id.to_string(),
            owner_cluster: cluster_id.to_string(),
            expanded: true,
        });
    }
    let child_count = graph.child_count(cluster_id);
    let child_snapshot = child_id_snapshot_work_upper_bound(graph, cluster_id, work_control)?;
    work_control.charge_adapter(work_control.checked_add(child_snapshot, child_count)?)?;
    stack.extend(
        graph
            .children(cluster_id)
            .into_iter()
            .rev()
            .map(|child| CopyFrame {
                node: child.to_string(),
                owner_cluster: cluster_id.to_string(),
                expanded: false,
            }),
    );

    while let Some(frame) = stack.pop() {
        work_control.charge_adapter(1)?;
        if frame.expanded {
            nodes.push((frame.node, frame.owner_cluster));
            continue;
        }
        if !visited.insert(frame.node.clone()) {
            continue;
        }

        let child_count = graph.child_count(&frame.node);
        if child_count == 0 {
            nodes.push((frame.node, frame.owner_cluster));
            continue;
        }

        let child_snapshot = child_id_snapshot_work_upper_bound(graph, &frame.node, work_control)?;
        let child_clone_work = work_control.checked_add(child_snapshot, child_count)?;
        work_control.charge_adapter(work_control.checked_add(child_clone_work, 1)?)?;
        stack.push(CopyFrame {
            node: frame.node.clone(),
            owner_cluster: frame.node.clone(),
            expanded: true,
        });
        stack.extend(
            graph
                .children(&frame.node)
                .into_iter()
                .rev()
                .map(|child| CopyFrame {
                    node: child.to_string(),
                    owner_cluster: frame.node.clone(),
                    expanded: false,
                }),
        );
    }

    // The production caller copies the root's descendants, so no removal can delete the root.
    let remember_boundary_sources = cluster_id == root_id && graph.options().directed;
    let mut edge_snapshot = None;
    for (node, owner_cluster) in nodes {
        work_control.charge_adapter(1)?;
        if !graph.has_node(&node) {
            continue;
        }

        let data = graph.node(&node).cloned().unwrap_or_default();
        charge_node_insertion_ordering(new_graph, &node, work_control)?;
        new_graph.set_node(node.clone(), data);

        if let Some(parent) = graph.parent(&node)
            && parent != root_id
        {
            set_parent_controlled(new_graph, &node, parent, work_control)?;
        }
        if owner_cluster != root_id && node != owner_cluster {
            set_parent_controlled(new_graph, &node, &owner_cluster, work_control)?;
        }

        let edges = match &mut edge_snapshot {
            Some(edges) => edges,
            pending @ None => pending.insert(cluster_copy_edge_snapshot(
                graph,
                root_id,
                descendants.get(root_id).map_or(&[], Vec::as_slice),
                work_control,
            )?),
        };
        // The first pass must see every eligible edge in Graphlib order. Inserting an internal
        // edge can create endpoints before their nodes are visited by this traversal.
        for &(edge_ix, destination) in &edges.slots {
            work_control.charge_adapter(2)?; // Replay this entry and retain it after removal.
            let destination = destination.map(|index| &mut edges.destinations[index.get() - 1]);
            // A live source still owns its outside endpoint. The root and rewritten key
            // therefore remain live, and writing the same source label is a no-op.
            if destination
                .as_ref()
                .is_some_and(|entry| entry.last_source == Some(edge_ix))
            {
                continue;
            }
            let Some(edge_key) = graph.edge_key_by_ix(edge_ix) else {
                continue;
            };
            match destination {
                None => {
                    if !new_graph.has_edge(&edge_key.v, &edge_key.w, edge_key.name.as_deref())
                        && let Some(label) = graph.edge_by_key(edge_key).cloned()
                    {
                        charge_edge_endpoint_insertions(
                            new_graph,
                            &edge_key.v,
                            &edge_key.w,
                            work_control,
                        )?;
                        new_graph.set_edge_key(edge_key.clone(), label);
                    }
                }
                Some(destination) => {
                    let Some(label) = graph.edge_by_key(edge_key).cloned() else {
                        continue;
                    };
                    let (v, w) = if destination.from_inside {
                        (root_id, edge_key.w.as_str())
                    } else {
                        (edge_key.v.as_str(), root_id)
                    };
                    charge_edge_endpoint_insertions(graph, v, w, work_control)?;
                    let rewritten = dugong::graphlib::EdgeKey::new(v, w, edge_key.name.as_deref());
                    graph.set_edge_key(rewritten, label);
                    if remember_boundary_sources {
                        destination.last_source = Some(edge_ix);
                    }
                }
            }
        }

        let removal_work = remove_node_work_upper_bound(graph, &node, work_control)?;
        work_control.charge_adapter(removal_work)?;
        let _ = graph.remove_node(&node);
        // Internal edges were installed on the first pass. Rewritten edges touch the root and
        // cannot enter the snapshot. Retire removed source slots before an append can reuse them.
        edges.slots.retain(|&(edge_ix, destination)| {
            destination.is_some() && graph.edge_key_by_ix(edge_ix).is_some()
        });
    }

    Ok(())
}

/// Extracts independently placed clusters and records them in nested render order.
pub fn extract_clusters_recursively<'a>(
    graph: &mut Graph<NodeLabel, EdgeLabel, GraphLabel>,
    authored_direction: impl Fn(&str) -> Option<&'a str>,
    external_connections_by_id: &HashMap<String, bool>,
    extracted: &mut HashMap<String, Graph<NodeLabel, EdgeLabel, GraphLabel>>,
    extracted_order: &mut Vec<String>,
    work_control: &mut OperationLayoutWorkControl,
) -> Result<()> {
    // Mermaid's recursive JavaScript extractor stops after depth 10 as a stack-safety bailout.
    // Merman intentionally replaces that fixed cutoff with an explicit stack plus the shared work
    // limit, preserving extraction semantics for deeper valid graphs without unbounded recursion.
    struct ExtractFrame {
        id: String,
        graph: Graph<NodeLabel, EdgeLabel, GraphLabel>,
        expanded: bool,
    }

    type ExtractedCluster = (String, Graph<NodeLabel, EdgeLabel, GraphLabel>);

    fn extract_one_level<'a>(
        graph: &mut Graph<NodeLabel, EdgeLabel, GraphLabel>,
        authored_direction: &impl Fn(&str) -> Option<&'a str>,
        external_connections_by_id: &HashMap<String, bool>,
        work_control: &mut OperationLayoutWorkControl,
    ) -> Result<Vec<ExtractedCluster>> {
        let snapshot_work = node_id_snapshot_work_upper_bound(graph, work_control)?;
        work_control.charge_adapter(snapshot_work)?;
        let node_ids = graph.node_ids();
        let mut descendants = HashMap::new();
        work_control.charge_adapter(node_ids.len())?;
        for id in &node_ids {
            if graph.child_count(id) == 0 {
                continue;
            }
            descendants.insert(id.clone(), extract_descendants(id, graph, work_control)?);
        }

        let mut extracted_here: Vec<ExtractedCluster> = Vec::new();

        work_control.charge_adapter(node_ids.len())?;
        let candidates: Vec<String> = node_ids
            .into_iter()
            .filter(|id| graph.has_node(id))
            .filter(|id| graph.child_count(id) != 0)
            // Mermaid's Dagre extractor only promotes clusters without external connections.
            // An explicit `direction` controls the nested graph's rankdir after extraction; it
            // does not change whether the cluster is extracted. Keeping this predicate tied to
            // the computed `externalConnections` flag also keeps cluster extraction, edge
            // ownership, and the downstream render plan on the same semantic boundary.
            //
            // Reference:
            // - `packages/mermaid/src/rendering-util/layout-algorithms/dagre/mermaid-graphlib.js`
            .filter(|id| {
                external_connections_by_id
                    .get(id.as_str())
                    .is_some_and(|external| !external)
            })
            .collect();

        for id in candidates {
            if !graph.has_node(&id) {
                continue;
            }
            if graph.child_count(&id) == 0 {
                continue;
            }

            let mut cluster_graph: Graph<NodeLabel, EdgeLabel, GraphLabel> =
                Graph::new(GraphOptions {
                    multigraph: true,
                    compound: true,
                    directed: true,
                });

            // Mermaid's `extractor(...)` uses:
            // - `clusterData.dir` when explicitly set for the subgraph
            // - otherwise: toggle relative to the current graph's rankdir (TB<->LR)
            let dir = authored_direction(id.as_str())
                .map(str::trim)
                .filter(|s| !s.is_empty())
                .map(normalize_dir)
                .unwrap_or_else(|| toggled_dir(flow_dir_from_rankdir(graph.graph().rankdir)));

            cluster_graph.set_graph(GraphLabel {
                rankdir: rank_dir_from_flow(&dir),
                // Mermaid's cluster extractor initializes subgraphs with a fixed dagre config
                // (nodesep/ranksep=50, marginx/marginy=8). Before each recursive render Mermaid then
                // overrides `nodesep` to the parent graph value and `ranksep` to `parent.ranksep + 25`.
                //
                // We model that in headless mode by keeping the extractor defaults here, then applying
                // the per-depth override inside `layout_graph_with_recursive_clusters(...)` right
                // before laying out each extracted graph.
                //
                // Reference:
                // - `packages/mermaid/src/rendering-util/layout-algorithms/dagre/mermaid-graphlib.js`
                // - `packages/mermaid/src/rendering-util/layout-algorithms/dagre/index.js`
                nodesep: 50.0,
                ranksep: 50.0,
                marginx: 8.0,
                marginy: 8.0,
                acyclicer: None,
                ..Default::default()
            });

            copy_cluster(
                &id,
                graph,
                &mut cluster_graph,
                &id,
                &descendants,
                work_control,
            )?;
            extracted_here.push((id, cluster_graph));
        }

        Ok(extracted_here)
    }

    let extracted_here = extract_one_level(
        graph,
        &authored_direction,
        external_connections_by_id,
        work_control,
    )?;
    let mut stack: Vec<ExtractFrame> = extracted_here
        .into_iter()
        .rev()
        .map(|(id, graph)| ExtractFrame {
            id,
            graph,
            expanded: false,
        })
        .collect();

    while let Some(mut frame) = stack.pop() {
        if frame.expanded {
            extracted_order.push(frame.id.clone());
            extracted.insert(frame.id, frame.graph);
            continue;
        }

        let children = extract_one_level(
            &mut frame.graph,
            &authored_direction,
            external_connections_by_id,
            work_control,
        )?;
        frame.expanded = true;
        stack.push(frame);
        stack.extend(children.into_iter().rev().map(|(id, graph)| ExtractFrame {
            id,
            graph,
            expanded: false,
        }));
    }
    Ok(())
}
