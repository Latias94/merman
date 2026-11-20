use super::{HashMap, RefCell, RouteOwner};
use crate::Result;
use crate::graph::layout::{GraphLayout, GroupLayout};
use crate::graph::routing::geometry::group_contains_node;
use crate::graph::routing::layout_allocation_failed;
use crate::resource::ResourceContext;
use std::rc::Rc;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum LabelContainerRelation {
    Interior,
    Exterior,
    Crossing,
}

/// Shared only within one immutable layout generation. Canonical edge/container indices
/// identify invariant geometry; detached callers use a zero edge count to disable reuse.
#[derive(Debug)]
pub(in crate::graph::routing) struct LabelRegionCache<'layout> {
    layout: &'layout GraphLayout,
    canonical_edge_count: usize,
    relations: RefCell<HashMap<(usize, usize), LabelContainerRelation>>,
}

impl<'layout> LabelRegionCache<'layout> {
    pub(in crate::graph::routing) fn try_new(
        layout: &'layout GraphLayout,
        canonical_edge_count: usize,
        resources: &ResourceContext,
    ) -> Result<Rc<Self>> {
        resources.charge_layout_work(1)?;
        Ok(Rc::new(Self {
            layout,
            canonical_edge_count,
            relations: RefCell::new(HashMap::default()),
        }))
    }

    pub(super) fn layout(&self) -> &'layout GraphLayout {
        self.layout
    }

    pub(super) fn classify(
        &self,
        owner: &RouteOwner,
        group_index: usize,
        group: &GroupLayout,
        resources: &ResourceContext,
    ) -> Result<LabelContainerRelation> {
        let key = (owner.canonical_edge_index, group_index);
        let cacheable = owner.canonical_edge_index < self.canonical_edge_count
            && group_index < self.layout.groups.len();
        if cacheable {
            resources.charge_layout_work(1)?;
            if let Some(relation) = self.relations.borrow().get(&key).copied() {
                return Ok(relation);
            }
        }

        let endpoint_inside = |id: &str| -> Result<bool> {
            resources.charge_layout_work(
                resources.checked_work_add(self.layout.nodes.len(), self.layout.groups.len())?,
            )?;
            // Keep existential matching across all nodes and groups, including duplicate IDs.
            Ok(self
                .layout
                .nodes
                .iter()
                .any(|node| node.id == id && group_contains_node(group, node))
                || self.layout.groups.iter().any(|child| {
                    child.id == id
                        && group.x < child.x
                        && child.right() < group.right()
                        && group.y < child.y
                        && child.bottom() < group.bottom()
                }))
        };
        let from_inside = endpoint_inside(&owner.from)?;
        let to_inside = endpoint_inside(&owner.to)?;
        let interior = if group.id == owner.from {
            to_inside
        } else if group.id == owner.to {
            from_inside
        } else {
            from_inside && to_inside
        };
        let relation = if interior {
            LabelContainerRelation::Interior
        } else if group.id == owner.from || group.id == owner.to || (!from_inside && !to_inside) {
            LabelContainerRelation::Exterior
        } else {
            LabelContainerRelation::Crossing
        };

        if cacheable {
            let mut relations = self.relations.borrow_mut();
            resources.charge_layout_work(1)?;
            if relations.len() == relations.capacity() {
                // Charge retained-key rehashing before growing the lazy table.
                resources.charge_layout_work(relations.len())?;
                relations
                    .try_reserve(1)
                    .map_err(|_| layout_allocation_failed())?;
            }
            relations.insert(key, relation);
        }
        Ok(relation)
    }
}

#[cfg(test)]
mod tests;
