use super::working::{ProducerLaneRole, WorkingNode};
use indexmap::IndexMap;
use std::collections::{HashMap, HashSet};

pub(super) struct LaneModel {
    lanes: HashSet<String>,
    pools: HashSet<String>,
    lane_by_id: HashMap<String, Option<String>>,
    pub lanes_by_pool: HashMap<String, Vec<String>>,
}

impl LaneModel {
    pub fn new(nodes: &IndexMap<String, WorkingNode>) -> Self {
        let group_parents: HashSet<&str> = nodes
            .values()
            .filter(|node| node.is_group())
            .filter_map(|node| node.parent_id.as_deref())
            .collect();
        let pools: HashSet<String> = nodes
            .values()
            .filter(|node| {
                node.is_group()
                    && matches!(node.producer_lane_role, Some(ProducerLaneRole::Pool))
                    && group_parents.contains(node.id.as_str())
            })
            .map(|node| node.id.clone())
            .collect();
        let mut lanes = HashSet::new();
        let mut lanes_by_pool: HashMap<String, Vec<String>> = HashMap::new();
        for node in nodes.values().filter(|node| node.is_group()) {
            if pools.contains(&node.id) {
                continue;
            }
            if let Some(parent) = node
                .parent_id
                .as_ref()
                .filter(|parent| pools.contains(*parent))
            {
                lanes.insert(node.id.clone());
                lanes_by_pool
                    .entry(parent.clone())
                    .or_default()
                    .push(node.id.clone());
            } else if node.parent_id.is_none() {
                lanes.insert(node.id.clone());
            }
        }
        let mut lane_by_id: HashMap<String, Option<String>> = HashMap::new();
        for node in nodes.values() {
            if lane_by_id.contains_key(&node.id) {
                continue;
            }
            let mut current = node.parent_id.as_deref();
            let mut seen = HashSet::from([node.id.as_str()]);
            let mut path = vec![node.id.as_str()];
            let mut resolved = None;
            while let Some(parent) = current {
                if !seen.insert(parent) {
                    break;
                }
                if lanes.contains(parent) {
                    resolved = Some(parent.to_string());
                    break;
                }
                if let Some(cached) = lane_by_id.get(parent) {
                    resolved = cached.clone();
                    break;
                }
                path.push(parent);
                current = nodes.get(parent).and_then(|node| node.parent_id.as_deref());
            }
            for id in path {
                lane_by_id.insert(id.to_string(), resolved.clone());
            }
        }
        Self {
            lanes,
            pools,
            lane_by_id,
            lanes_by_pool,
        }
    }

    pub fn is_lane(&self, id: &str) -> bool {
        self.lanes.contains(id)
    }

    pub fn is_pool(&self, id: &str) -> bool {
        self.pools.contains(id)
    }

    pub fn lane_of(&self, id: &str) -> Option<&str> {
        self.lane_by_id.get(id).and_then(Option::as_deref)
    }

    #[cfg(any(feature = "diagram-flowchart", feature = "diagram-swimlane"))]
    pub fn order(
        &self,
        nodes: &IndexMap<String, WorkingNode>,
        indices: &HashMap<&str, f64>,
    ) -> Vec<String> {
        let lane_ids: Vec<String> = nodes
            .values()
            .filter(|node| self.is_lane(&node.id))
            .map(|node| node.id.clone())
            .collect();
        if !lane_ids.is_empty() && lane_ids.iter().all(|id| indices.contains_key(id.as_str())) {
            let mut indexed = lane_ids;
            indexed.sort_by(|left, right| {
                indices[left.as_str()]
                    .partial_cmp(&indices[right.as_str()])
                    .unwrap_or(std::cmp::Ordering::Equal)
            });
            return indexed;
        }
        if self.pools.is_empty() {
            return lane_ids.into_iter().rev().collect();
        }
        let mut order = Vec::new();
        for node in nodes.values().filter(|node| node.parent_id.is_none()) {
            if self.is_pool(&node.id) {
                order.extend(
                    self.lanes_by_pool
                        .get(&node.id)
                        .into_iter()
                        .flatten()
                        .cloned(),
                );
            } else if self.is_lane(&node.id) {
                order.push(node.id.clone());
            }
        }
        order.reverse();
        order
    }
}
