use std::collections::{HashMap, HashSet};
use std::hash::Hash;

use super::model::AsciiGraph;
use crate::error::{AsciiError, Result};
use crate::resource::{AsciiResourceLimitId, AsciiResourceLimitPhase, ResourceContext};

#[derive(Debug)]
pub(super) struct GraphGroupTopology<'a> {
    graph: &'a AsciiGraph,
    group_index_by_id: HashMap<&'a str, usize>,
    node_index_by_id: HashMap<&'a str, usize>,
    direct_group_index_by_node: HashMap<&'a str, usize>,
    parent_index_by_group: HashMap<usize, usize>,
    container_group_indices_by_member: HashMap<&'a str, Vec<usize>>,
}

impl<'a> GraphGroupTopology<'a> {
    pub(super) fn try_new(graph: &'a AsciiGraph, resources: &mut ResourceContext) -> Result<Self> {
        let member_count = graph.groups.iter().try_fold(0usize, |total, group| {
            checked_layout_work_add(resources, total, group.nodes.len())
        })?;
        let construction_work = checked_layout_work_add(
            resources,
            checked_layout_work_add(resources, graph.groups.len(), graph.nodes.len())?,
            member_count,
        )?;
        resources.charge_layout_work(construction_work)?;

        let mut group_index_by_id = HashMap::new();
        try_reserve_hash_map(&mut group_index_by_id, graph.groups.len())?;
        for (index, group) in graph.groups.iter().enumerate() {
            group_index_by_id.insert(group.id.as_str(), index);
        }

        let mut node_index_by_id = HashMap::new();
        try_reserve_hash_map(&mut node_index_by_id, graph.nodes.len())?;
        for (index, node) in graph.nodes.iter().enumerate() {
            node_index_by_id.insert(node.id.as_str(), index);
        }

        let mut parent_index_by_group = HashMap::new();
        try_reserve_hash_map(&mut parent_index_by_group, graph.groups.len())?;
        let mut direct_group_index_by_node = HashMap::new();
        try_reserve_hash_map(&mut direct_group_index_by_node, graph.nodes.len())?;
        let mut container_group_indices_by_member = HashMap::<&str, Vec<usize>>::new();
        try_reserve_hash_map(&mut container_group_indices_by_member, member_count)?;

        for (group_index, group) in graph.groups.iter().enumerate() {
            for member in &group.nodes {
                if let Some(child_index) = group_index_by_id.get(member.as_str()).copied() {
                    parent_index_by_group
                        .entry(child_index)
                        .or_insert(group_index);
                }
                if node_index_by_id.contains_key(member.as_str()) {
                    direct_group_index_by_node
                        .entry(member.as_str())
                        .or_insert(group_index);
                }
                let containers = container_group_indices_by_member
                    .entry(member.as_str())
                    .or_default();
                try_reserve_vec(containers, 1)?;
                containers.push(group_index);
            }
        }

        Ok(Self {
            graph,
            group_index_by_id,
            node_index_by_id,
            direct_group_index_by_node,
            parent_index_by_group,
            container_group_indices_by_member,
        })
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

    pub(super) fn parent_group_index(&self, group_index: usize) -> Option<usize> {
        self.parent_index_by_group.get(&group_index).copied()
    }

    pub(super) fn group_member_node_indices(
        &self,
        group_index: usize,
        resources: &mut ResourceContext,
    ) -> Result<Vec<usize>> {
        let mut indices = HashSet::new();
        let mut visited_groups = HashSet::new();
        let mut stack = Vec::new();
        push_group_frame(&mut stack, group_index, 1, resources)?;

        while let Some((index, depth)) = stack.pop() {
            try_reserve_hash_set(&mut visited_groups, 1)?;
            if !visited_groups.insert(index) {
                continue;
            }
            let Some(group) = self.graph.groups.get(index) else {
                continue;
            };

            for member in &group.nodes {
                resources.charge_layout_work(1)?;
                if let Some(node_index) = self.node_index(member) {
                    try_reserve_hash_set(&mut indices, 1)?;
                    indices.insert(node_index);
                } else if let Some(child_group_index) = self.group_index(member) {
                    push_group_frame(
                        &mut stack,
                        child_group_index,
                        next_nesting_depth(depth, resources)?,
                        resources,
                    )?;
                }
            }
        }

        let mut indices_vec = Vec::new();
        try_reserve_vec(&mut indices_vec, indices.len())?;
        for index in indices {
            indices_vec.push(index);
        }
        indices_vec.sort_unstable();
        Ok(indices_vec)
    }

    pub(super) fn groups_containing_endpoint(
        &self,
        endpoint: &str,
        resources: &mut ResourceContext,
    ) -> Result<HashSet<usize>> {
        let mut groups = HashSet::new();
        let mut stack = Vec::new();
        if let Some(group_index) = self.group_index(endpoint) {
            push_group_frame(&mut stack, group_index, 1, resources)?;
        }
        if let Some(container_indices) = self.container_group_indices_by_member.get(endpoint) {
            for group_index in container_indices.iter().copied() {
                push_group_frame(&mut stack, group_index, 1, resources)?;
            }
        }

        while let Some((group_index, depth)) = stack.pop() {
            try_reserve_hash_set(&mut groups, 1)?;
            if !groups.insert(group_index) {
                continue;
            }
            let Some(group) = self.graph.groups.get(group_index) else {
                continue;
            };
            if let Some(parent_indices) = self
                .container_group_indices_by_member
                .get(group.id.as_str())
            {
                for parent_index in parent_indices.iter().copied() {
                    push_group_frame(
                        &mut stack,
                        parent_index,
                        next_nesting_depth(depth, resources)?,
                        resources,
                    )?;
                }
            }
        }

        Ok(groups)
    }

    pub(super) fn group_depth(
        &self,
        group_index: usize,
        resources: &mut ResourceContext,
    ) -> Result<usize> {
        let mut depth = 0usize;
        let mut current = group_index;

        for _ in 0..self.graph.groups.len() {
            resources.charge_layout_work(1)?;
            resources.check_nesting_depth(next_nesting_depth(depth, resources)?)?;
            let Some(parent_index) = self.parent_index_by_group.get(&current).copied() else {
                return Ok(depth);
            };
            depth = next_nesting_depth(depth, resources)?;
            current = parent_index;
        }

        Ok(depth)
    }
}

fn push_group_frame(
    stack: &mut Vec<(usize, usize)>,
    group_index: usize,
    depth: usize,
    resources: &mut ResourceContext,
) -> Result<()> {
    resources.check_nesting_depth(depth)?;
    resources.charge_layout_work(1)?;
    try_reserve_vec(stack, 1)?;
    stack.push((group_index, depth));
    Ok(())
}

fn next_nesting_depth(depth: usize, resources: &ResourceContext) -> Result<usize> {
    depth.checked_add(1).ok_or_else(|| {
        resources
            .policy()
            .overflow(AsciiResourceLimitId::MaxNestingDepth)
    })
}

fn checked_layout_work_add(
    resources: &ResourceContext,
    left: usize,
    right: usize,
) -> Result<usize> {
    resources.checked_work_add(left, right)
}

fn try_reserve_vec<T>(values: &mut Vec<T>, additional: usize) -> Result<()> {
    values
        .try_reserve(additional)
        .map_err(|_| allocation_failed())
}

fn try_reserve_hash_map<K, V>(map: &mut HashMap<K, V>, additional: usize) -> Result<()>
where
    K: Eq + Hash,
{
    map.try_reserve(additional).map_err(|_| allocation_failed())
}

fn try_reserve_hash_set<T>(set: &mut HashSet<T>, additional: usize) -> Result<()>
where
    T: Eq + Hash,
{
    set.try_reserve(additional).map_err(|_| allocation_failed())
}

fn allocation_failed() -> AsciiError {
    AsciiError::allocation_failed(AsciiResourceLimitPhase::LayoutWork.as_str())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::graph::model::{GraphDirection, GraphGroupStyle};
    use crate::resource::AsciiResourcePolicy;
    use merman_core::resources::ResourceProfile;

    #[test]
    fn nested_topology_construction_accepts_exact_work_and_rejects_max_minus_one() {
        const GROUP_COUNT: usize = 64;

        let mut graph = AsciiGraph::new(GraphDirection::TopDown);
        graph.add_node("node", "Node");
        let mut member = "node".to_string();
        for index in 0..GROUP_COUNT {
            let group_id = format!("group-{index}");
            graph.add_group_with_style(
                group_id.clone(),
                format!("Group {index}"),
                None,
                vec![member],
                GraphGroupStyle::default(),
            );
            member = group_id;
        }

        let exact_work = graph.nodes.len() + graph.groups.len() + GROUP_COUNT;
        let unbounded = AsciiResourcePolicy::for_profile(ResourceProfile::UnboundedForTrustedInput);
        let exact_policy = unbounded
            .with_limit(AsciiResourceLimitId::MaxLayoutWorkUnits, exact_work)
            .expect("exact layout-work limit should be valid");
        let mut exact_resources = ResourceContext::new(exact_policy);
        GraphGroupTopology::try_new(&graph, &mut exact_resources)
            .expect("the exact topology construction work should pass");
        assert_eq!(exact_resources.layout_work_used(), exact_work);

        let below_policy = unbounded
            .with_limit(AsciiResourceLimitId::MaxLayoutWorkUnits, exact_work - 1)
            .expect("max-minus-one layout-work limit should be valid");
        let mut below_resources = ResourceContext::new(below_policy);
        let error = GraphGroupTopology::try_new(&graph, &mut below_resources)
            .expect_err("max-minus-one work should fail before topology allocation");
        let AsciiError::ResourceLimitExceeded(details) = error else {
            panic!("expected a layout-work resource error, got {error:?}");
        };
        assert_eq!(details.limit, AsciiResourceLimitId::MaxLayoutWorkUnits);
        assert_eq!(details.actual, exact_work);
        assert_eq!(details.max, exact_work - 1);
        assert_eq!(below_resources.layout_work_used(), 0);
    }
}
