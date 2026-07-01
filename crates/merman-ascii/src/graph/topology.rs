use std::collections::{HashMap, HashSet};

use super::model::AsciiGraph;

pub(super) struct GraphGroupTopology<'a> {
    graph: &'a AsciiGraph,
    group_index_by_id: HashMap<&'a str, usize>,
    node_index_by_id: HashMap<&'a str, usize>,
    direct_group_index_by_node: HashMap<&'a str, usize>,
    parent_index_by_group: HashMap<usize, usize>,
}

impl<'a> GraphGroupTopology<'a> {
    pub(super) fn new(graph: &'a AsciiGraph) -> Self {
        let group_index_by_id = graph
            .groups
            .iter()
            .enumerate()
            .map(|(index, group)| (group.id.as_str(), index))
            .collect::<HashMap<_, _>>();
        let node_index_by_id = graph
            .nodes
            .iter()
            .enumerate()
            .map(|(index, node)| (node.id.as_str(), index))
            .collect::<HashMap<_, _>>();
        let mut parent_index_by_group = HashMap::new();
        for (parent_index, group) in graph.groups.iter().enumerate() {
            for member in &group.nodes {
                if let Some(child_index) = group_index_by_id.get(member.as_str()).copied() {
                    parent_index_by_group.insert(child_index, parent_index);
                }
            }
        }
        let mut direct_group_index_by_node = HashMap::new();
        for (group_index, group) in graph.groups.iter().enumerate() {
            for member in &group.nodes {
                if node_index_by_id.contains_key(member.as_str()) {
                    direct_group_index_by_node
                        .entry(member.as_str())
                        .or_insert(group_index);
                }
            }
        }

        Self {
            graph,
            group_index_by_id,
            node_index_by_id,
            direct_group_index_by_node,
            parent_index_by_group,
        }
    }

    pub(super) fn group_index(&self, group_id: &str) -> Option<usize> {
        self.group_index_by_id.get(group_id).copied()
    }

    pub(super) fn node_index(&self, node_id: &str) -> Option<usize> {
        self.node_index_by_id.get(node_id).copied()
    }

    pub(super) fn direct_node_group_index(&self, node_id: &str) -> Option<usize> {
        self.direct_group_index_by_node.get(node_id).copied()
    }

    pub(super) fn group_member_node_indices(&self, group_index: usize) -> Vec<usize> {
        let mut indices = HashSet::new();
        let mut visited_groups = HashSet::new();
        let mut stack = vec![group_index];

        while let Some(index) = stack.pop() {
            if !visited_groups.insert(index) {
                continue;
            }
            let Some(group) = self.graph.groups.get(index) else {
                continue;
            };

            for member in &group.nodes {
                if let Some(node_index) = self.node_index(member) {
                    indices.insert(node_index);
                } else if let Some(child_group_index) = self.group_index(member) {
                    stack.push(child_group_index);
                }
            }
        }

        let mut indices = indices.into_iter().collect::<Vec<_>>();
        indices.sort_unstable();
        indices
    }

    pub(super) fn group_contains_endpoint(&self, group_index: usize, endpoint: &str) -> bool {
        let mut visited_groups = HashSet::new();
        let mut stack = vec![group_index];

        while let Some(index) = stack.pop() {
            if !visited_groups.insert(index) {
                continue;
            }
            let Some(group) = self.graph.groups.get(index) else {
                continue;
            };
            if group.id == endpoint {
                return true;
            }

            for member in &group.nodes {
                if member == endpoint {
                    return true;
                }
                if let Some(child_group_index) = self.group_index(member) {
                    stack.push(child_group_index);
                }
            }
        }

        false
    }

    pub(super) fn group_depth(&self, group_index: usize) -> usize {
        let mut depth = 0;
        let mut current = group_index;
        let mut visited = HashSet::new();

        while visited.insert(current) {
            let Some(parent_index) = self.parent_index_by_group.get(&current).copied() else {
                break;
            };
            depth += 1;
            current = parent_index;
        }

        depth
    }
}
