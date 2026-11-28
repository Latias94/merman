//! Forster in-layer successor and layout-unit constraints.
//!
//! Source reference:
//! - https://github.com/eclipse-elk/elk/blob/62d5909f96fad541bc101ad52dabaece6b7eab7e/plugins/org.eclipse.elk.alg.layered/src/org/eclipse/elk/alg/layered/p3order/ForsterConstraintResolver.java

use std::collections::{HashMap, HashSet, VecDeque};

use crate::graph::{LGraph, LNodeKind};

use super::BarycenterState;

#[derive(Debug, Clone, PartialEq)]
pub(super) struct ConstraintGroup {
    nodes: Vec<usize>,
    outgoing: Vec<usize>,
    incoming: Vec<usize>,
    incoming_count: usize,
    summed_weight: f64,
    degree: usize,
}

impl ConstraintGroup {
    pub(super) fn new(node: usize) -> Self {
        Self {
            nodes: vec![node],
            outgoing: Vec::new(),
            incoming: Vec::new(),
            incoming_count: 0,
            summed_weight: 0.0,
            degree: 0,
        }
    }

    pub(super) fn barycenter(&self, states: &HashMap<usize, BarycenterState>) -> Option<f64> {
        self.nodes
            .first()
            .and_then(|node| states.get(node))
            .and_then(|state| state.barycenter)
    }

    fn set_barycenter(&self, states: &mut HashMap<usize, BarycenterState>, value: f64) {
        for node in &self.nodes {
            states.entry(*node).or_default().barycenter = Some(value);
        }
    }

    fn reset_edges(&mut self) {
        self.outgoing.clear();
        self.incoming.clear();
        self.incoming_count = 0;
    }

    fn has_outgoing(&self) -> bool {
        !self.outgoing.is_empty()
    }

    fn has_incoming(&self) -> bool {
        !self.incoming.is_empty()
    }
}

pub(super) fn resolve_forster_constraints(
    graph: &LGraph,
    layer: &mut [usize],
    states: &mut HashMap<usize, BarycenterState>,
) {
    if layer.len() < 2 {
        return;
    }

    let mut groups = layer
        .iter()
        .copied()
        .map(ConstraintGroup::new)
        .collect::<Vec<_>>();
    if graph.in_layer_successor_constraints_between_non_dummies {
        process_forster_constraints(graph, states, &mut groups, true);
        apply_forster_group_order(layer, states, &groups);
        groups = layer
            .iter()
            .copied()
            .map(ConstraintGroup::new)
            .collect::<Vec<_>>();
    }
    process_forster_constraints(graph, states, &mut groups, false);

    apply_forster_group_order(layer, states, &groups);
}

fn apply_forster_group_order(
    layer: &mut [usize],
    states: &mut HashMap<usize, BarycenterState>,
    groups: &[ConstraintGroup],
) {
    let mut index = 0usize;
    for group in groups {
        for node in &group.nodes {
            if index < layer.len() {
                layer[index] = *node;
                index += 1;
            }
            if let Some(barycenter) = group.barycenter(states) {
                states.entry(*node).or_default().barycenter = Some(barycenter);
            }
        }
    }
}

fn process_forster_constraints(
    graph: &LGraph,
    states: &mut HashMap<usize, BarycenterState>,
    groups: &mut Vec<ConstraintGroup>,
    only_between_normal_nodes: bool,
) {
    build_forster_constraints_graph(graph, groups, only_between_normal_nodes);
    while let Some((predecessor, successor)) = find_violated_forster_constraint(states, groups) {
        handle_violated_forster_constraint(states, groups, predecessor, successor);
    }
}

fn build_forster_constraints_graph(
    graph: &LGraph,
    groups: &mut [ConstraintGroup],
    only_between_normal_nodes: bool,
) {
    for group in groups.iter_mut() {
        group.reset_edges();
    }

    let index_by_node = groups
        .iter()
        .enumerate()
        .flat_map(|(index, group)| group.nodes.iter().map(move |node| (*node, index)))
        .collect::<HashMap<_, _>>();

    let mut last_non_dummy_node: Option<usize> = None;
    let mut edges = Vec::<(usize, usize)>::new();
    for (group_index, group) in groups.iter().enumerate() {
        let Some(node) = group.nodes.first().copied() else {
            continue;
        };

        if only_between_normal_nodes && graph.layerless_nodes[node].kind != LNodeKind::Normal {
            continue;
        }

        for successor in &graph.layerless_nodes[node].in_layer_successor_constraints {
            if only_between_normal_nodes
                && graph.layerless_nodes[*successor].kind != LNodeKind::Normal
            {
                continue;
            }
            if let Some(successor_group) = index_by_node.get(successor).copied() {
                edges.push((group_index, successor_group));
            }
        }

        if !only_between_normal_nodes && graph.layerless_nodes[node].kind == LNodeKind::Normal {
            if let Some(previous) = last_non_dummy_node {
                let previous_unit_nodes = layout_unit_nodes(graph, previous, groups);
                let current_unit_nodes = layout_unit_nodes(graph, node, groups);
                for previous_unit in &previous_unit_nodes {
                    for current_unit in &current_unit_nodes {
                        if let (Some(left), Some(right)) = (
                            index_by_node.get(previous_unit).copied(),
                            index_by_node.get(current_unit).copied(),
                        ) {
                            edges.push((left, right));
                        }
                    }
                }
            }
            last_non_dummy_node = Some(node);
        }
    }

    for (from, to) in edges {
        add_forster_constraint(groups, from, to);
    }
}

fn layout_unit_nodes(graph: &LGraph, unit: usize, groups: &[ConstraintGroup]) -> Vec<usize> {
    groups
        .iter()
        .flat_map(|group| group.nodes.iter().copied())
        .filter(|node| graph.layerless_nodes[*node].in_layer_layout_unit == Some(unit))
        .collect::<Vec<_>>()
}

fn add_forster_constraint(groups: &mut [ConstraintGroup], from: usize, to: usize) {
    if from == to {
        return;
    }
    if !groups[from].outgoing.contains(&to) {
        groups[from].outgoing.push(to);
        groups[to].incoming_count += 1;
    }
}

fn find_violated_forster_constraint(
    states: &HashMap<usize, BarycenterState>,
    groups: &mut [ConstraintGroup],
) -> Option<(usize, usize)> {
    let mut active_groups = VecDeque::new();
    for (index, group) in groups.iter_mut().enumerate() {
        group.incoming.clear();
        if group.has_outgoing() && group.incoming_count == 0 {
            active_groups.push_back(index);
        }
    }

    while let Some(group_index) = active_groups.pop_front() {
        if groups[group_index].has_incoming() {
            for predecessor in groups[group_index].incoming.clone() {
                let predecessor_barycenter = groups[predecessor].barycenter(states).unwrap_or(0.0);
                let group_barycenter = groups[group_index].barycenter(states).unwrap_or(0.0);
                if predecessor_barycenter as f32 == group_barycenter as f32 {
                    if predecessor > group_index {
                        return Some((predecessor, group_index));
                    }
                } else if predecessor_barycenter > group_barycenter {
                    return Some((predecessor, group_index));
                }
            }
        }

        let outgoing = groups[group_index].outgoing.clone();
        for successor in outgoing {
            groups[successor].incoming.insert(0, group_index);
            if groups[successor].incoming_count == groups[successor].incoming.len() {
                active_groups.push_back(successor);
            }
        }
    }

    None
}

fn handle_violated_forster_constraint(
    states: &mut HashMap<usize, BarycenterState>,
    groups: &mut Vec<ConstraintGroup>,
    first_group: usize,
    second_group: usize,
) {
    let merged = merge_constraint_groups(states, &groups[first_group], &groups[second_group]);
    let merged_barycenter = merged.barycenter(states);
    let mut next = Vec::<ConstraintGroup>::with_capacity(groups.len().saturating_sub(1));
    let mut old_to_new = vec![usize::MAX; groups.len()];
    let mut merged_index = None;
    let mut inserted = false;

    for (index, group) in groups.iter().enumerate() {
        if index == first_group || index == second_group {
            continue;
        }
        if !inserted
            && let (Some(group_barycenter), Some(merged_barycenter)) =
                (group.barycenter(states), merged_barycenter)
            && group_barycenter > merged_barycenter
        {
            merged_index = Some(next.len());
            next.push(merged.clone());
            inserted = true;
        }
        old_to_new[index] = next.len();
        next.push(group.clone());
    }

    if !inserted {
        merged_index = Some(next.len());
        next.push(merged);
    }
    let merged_index = merged_index.expect("merged group should be inserted");
    old_to_new[first_group] = merged_index;
    old_to_new[second_group] = merged_index;
    remap_forster_constraints(&mut next, &old_to_new);

    *groups = next;
}

pub(super) fn merge_constraint_groups(
    states: &mut HashMap<usize, BarycenterState>,
    first: &ConstraintGroup,
    second: &ConstraintGroup,
) -> ConstraintGroup {
    let mut nodes = first.nodes.clone();
    nodes.extend(second.nodes.iter().copied());

    let merged = ConstraintGroup {
        nodes,
        outgoing: merged_outgoing_constraints(first, second),
        incoming: Vec::new(),
        incoming_count: 0,
        summed_weight: first.summed_weight + second.summed_weight,
        degree: first.degree + second.degree,
    };

    let barycenter = if merged.degree > 0 {
        Some(merged.summed_weight / merged.degree as f64)
    } else {
        match (first.barycenter(states), second.barycenter(states)) {
            (Some(first), Some(second)) => Some((first + second) / 2.0),
            (Some(first), None) => Some(first),
            (None, Some(second)) => Some(second),
            (None, None) => None,
        }
    };
    if let Some(barycenter) = barycenter {
        merged.set_barycenter(states, barycenter);
    }

    merged
}

fn merged_outgoing_constraints(first: &ConstraintGroup, second: &ConstraintGroup) -> Vec<usize> {
    first
        .outgoing
        .iter()
        .chain(second.outgoing.iter())
        .copied()
        .collect::<Vec<_>>()
}

fn remap_forster_constraints(groups: &mut [ConstraintGroup], old_to_new: &[usize]) {
    for group in groups.iter_mut() {
        group.outgoing = group
            .outgoing
            .iter()
            .filter_map(|old_index| old_to_new.get(*old_index).copied())
            .filter(|new_index| *new_index != usize::MAX)
            .collect::<Vec<_>>();
    }

    for (group_index, group) in groups.iter_mut().enumerate() {
        group.outgoing.retain(|successor| *successor != group_index);
        let mut seen = HashSet::new();
        group.outgoing.retain(|successor| seen.insert(*successor));
        group.incoming.clear();
        group.incoming_count = 0;
    }

    let edges = groups
        .iter()
        .enumerate()
        .flat_map(|(from, group)| group.outgoing.iter().copied().map(move |to| (from, to)))
        .collect::<Vec<_>>();
    for (_, to) in edges {
        groups[to].incoming_count += 1;
    }
}
