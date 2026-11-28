//! Metered physical and effective group ancestry for Flow SVG planning.

use merman_model::{OperationPhase, families::flowchart::FlowchartModel};
use merman_render_core::{Error, LayoutCluster, Result, resources::OperationWorkMeter};
use rustc_hash::{FxHashMap, FxHashSet};

pub fn apply_solved_group_parents<'a>(
    clusters_in_layout_order: &'a [LayoutCluster],
    solved_parents: Option<&[Option<u32>]>,
    parents: &mut FxHashMap<&'a str, &'a str>,
    clusters: &mut FxHashSet<&'a str>,
) {
    if let Some(solved_parents) = solved_parents {
        for (group, parent) in clusters_in_layout_order.iter().zip(solved_parents) {
            let id = group.id.as_str();
            clusters.insert(id);
            if let Some(index) = parent {
                parents.insert(id, &clusters_in_layout_order[*index as usize].id);
            } else {
                parents.remove(id);
            }
        }
    }
}

pub fn flowchart_node_dom_indices<'a>(model: &'a FlowchartModel) -> FxHashMap<&'a str, usize> {
    if !model.vertex_calls.is_empty() {
        let mut out: FxHashMap<&'a str, usize> = FxHashMap::default();
        out.reserve(model.vertex_calls.len());
        for (vertex_counter, id) in model.vertex_calls.iter().enumerate() {
            let id: &'a str = id.as_str();
            let _ = out.entry(id).or_insert(vertex_counter);
        }
        return out;
    }

    let mut out: FxHashMap<&'a str, usize> = FxHashMap::default();
    out.reserve(model.edges.len().saturating_mul(2) + model.nodes.len());
    let mut vertex_counter: usize = 0;

    // Mermaid FlowDB assigns `domId` when a vertex is first created, but increments the internal
    // `vertexCounter` on every `addVertex(...)` call (even for repeated references). This means the
    // domId suffix depends on the full "first-use" order + repeat uses.
    fn touch<'a>(id: &'a str, out: &mut FxHashMap<&'a str, usize>, c: &mut usize) {
        let _ = out.entry(id).or_insert(*c);
        *c += 1;
    }

    for e in &model.edges {
        touch(e.from.as_str(), &mut out, &mut vertex_counter);
        touch(e.to.as_str(), &mut out, &mut vertex_counter);
    }

    for n in &model.nodes {
        touch(n.id.as_str(), &mut out, &mut vertex_counter);
    }

    out
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct FlowchartClusterAncestryInterval {
    entry: usize,
    exit: usize,
}

/// Strict ancestor queries over the physical group forest used for cluster paint order.
#[derive(Debug, Default)]
pub struct FlowchartAncestryIndex<'a> {
    intervals_by_id: FxHashMap<&'a str, FlowchartClusterAncestryInterval>,
}

impl<'a> FlowchartAncestryIndex<'a> {
    /// Charge the physical identity inventory, then index its deduplicated relationships.
    pub fn prepare<Subgraph, Cluster>(
        subgraphs_by_id: &FxHashMap<&'a str, Subgraph>,
        layout_clusters_by_id: &FxHashMap<&'a str, Cluster>,
        physical_parent_by_id: &FxHashMap<&'a str, &'a str>,
        work_meter: &OperationWorkMeter,
        work_phase: OperationPhase,
    ) -> Result<Self> {
        let parent_identity_capacity = physical_parent_by_id
            .len()
            .checked_mul(2)
            .ok_or_else(|| work_meter.arithmetic_overflow())?;
        let identity_capacity = subgraphs_by_id
            .len()
            .checked_add(layout_clusters_by_id.len())
            .and_then(|count| count.checked_add(parent_identity_capacity))
            .ok_or_else(|| work_meter.arithmetic_overflow())?;
        work_meter.charge_work(identity_capacity, work_phase)?;
        let mut identities_by_id = FxHashSet::default();
        identities_by_id.reserve(identity_capacity);
        identities_by_id.extend(
            subgraphs_by_id
                .keys()
                .chain(layout_clusters_by_id.keys())
                .copied()
                .chain(
                    physical_parent_by_id
                        .iter()
                        .flat_map(|(child, parent)| [*child, *parent]),
                ),
        );
        let identity_count = identities_by_id.len();

        work_meter.charge_work(identity_count, work_phase)?;
        let mut children_by_parent: FxHashMap<&'a str, Vec<&'a str>> = FxHashMap::default();
        children_by_parent.reserve(identity_count);
        for id in identities_by_id.iter().copied() {
            let Some(parent) = physical_parent_by_id.get(id).copied() else {
                continue;
            };
            if identities_by_id.contains(parent) {
                children_by_parent.entry(parent).or_default().push(id);
            }
        }

        work_meter.charge_work(identity_count, work_phase)?;
        let roots = identities_by_id
            .iter()
            .copied()
            .filter(|id| {
                !physical_parent_by_id
                    .get(id)
                    .is_some_and(|parent| identities_by_id.contains(parent))
            })
            .collect::<Vec<_>>();

        let traversal_work = identity_count
            .checked_mul(2)
            .ok_or_else(|| work_meter.arithmetic_overflow())?;
        work_meter.charge_work(traversal_work, work_phase)?;
        let mut entry_by_id = FxHashMap::default();
        entry_by_id.reserve(identity_count);
        let mut intervals_by_id = FxHashMap::default();
        intervals_by_id.reserve(identity_count);
        let mut clock = 0usize;
        let mut stack = Vec::new();
        for root in roots {
            stack.push((root, false));
            while let Some((id, exiting)) = stack.pop() {
                if exiting {
                    let Some(entry) = entry_by_id.get(id).copied() else {
                        return Err(flowchart_parent_cycle_error());
                    };
                    let exit = clock;
                    clock = clock
                        .checked_add(1)
                        .ok_or_else(|| work_meter.arithmetic_overflow())?;
                    intervals_by_id.insert(id, FlowchartClusterAncestryInterval { entry, exit });
                    continue;
                }

                if entry_by_id.contains_key(id) {
                    continue;
                }
                let entry = clock;
                clock = clock
                    .checked_add(1)
                    .ok_or_else(|| work_meter.arithmetic_overflow())?;
                entry_by_id.insert(id, entry);
                stack.push((id, true));
                if let Some(children) = children_by_parent.get(id) {
                    stack.extend(children.iter().rev().map(|child| (*child, false)));
                }
            }
        }

        if intervals_by_id.len() != identity_count {
            return Err(flowchart_parent_cycle_error());
        }
        Ok(Self { intervals_by_id })
    }

    pub fn is_strict_ancestor(&self, ancestor: &str, node: &str) -> bool {
        if ancestor == node {
            return false;
        }
        let Some(ancestor) = self.intervals_by_id.get(ancestor) else {
            return false;
        };
        let Some(node) = self.intervals_by_id.get(node) else {
            return false;
        };
        ancestor.entry < node.entry && node.exit < ancestor.exit
    }
}

/// Owned render parents and depths after the facade's transparent groups are flattened.
#[derive(Debug)]
pub struct FlowchartEffectiveParentIndex {
    entries: FxHashMap<String, (Option<String>, usize)>,
}

impl FlowchartEffectiveParentIndex {
    pub fn prepare<'a>(
        physical_parent_by_id: &FxHashMap<&'a str, &'a str>,
        mut is_transparent_parent: impl FnMut(&str) -> bool,
        work_meter: &OperationWorkMeter,
    ) -> Result<Self> {
        work_meter.charge(physical_parent_by_id.len())?;

        let mut parent_by_id = FxHashMap::default();
        parent_by_id.reserve(physical_parent_by_id.len());
        let ids = physical_parent_by_id.keys().copied().collect::<Vec<_>>();
        for id in ids.iter().copied() {
            if parent_by_id.contains_key(id) {
                continue;
            }

            let mut path = Vec::new();
            let mut current = id;
            let effective_parent = loop {
                if let Some(cached) = parent_by_id.get(current).copied() {
                    break cached;
                }
                let Some(parent) = physical_parent_by_id.get(current).copied() else {
                    break None;
                };

                // Charge the physical edge before retaining it in the flattened path. A chain of
                // transparent subgraphs is therefore never hidden inside one logical lookup.
                work_meter.charge(1)?;
                path.push(current);
                if path.len() > physical_parent_by_id.len() {
                    return Err(flowchart_parent_cycle_error());
                }

                if is_transparent_parent(parent) {
                    current = parent;
                } else {
                    break Some(parent);
                }
            };

            for path_id in path.into_iter().rev() {
                parent_by_id.insert(path_id, effective_parent);
            }
        }

        work_meter.charge(parent_by_id.len())?;
        let mut depth_by_id = FxHashMap::default();
        depth_by_id.reserve(parent_by_id.len());
        for id in ids {
            if depth_by_id.contains_key(id) {
                continue;
            }

            let mut path = Vec::new();
            let mut current = id;
            let base_depth = loop {
                if let Some(depth) = depth_by_id.get(current).copied() {
                    break depth;
                }

                work_meter.charge(1)?;
                path.push(current);
                if path.len() > parent_by_id.len().saturating_add(1) {
                    return Err(flowchart_parent_cycle_error());
                }

                let Some(parent) = parent_by_id.get(current).copied().flatten() else {
                    break 0usize;
                };
                current = parent;
            };

            let mut depth = base_depth;
            for path_id in path.into_iter().rev() {
                depth = depth
                    .checked_add(1)
                    .ok_or_else(|| work_meter.arithmetic_overflow())?;
                depth_by_id.insert(path_id, depth);
            }
        }

        Ok(Self {
            entries: depth_by_id
                .into_iter()
                .map(|(id, depth)| {
                    let parent = parent_by_id.get(id).copied().flatten().map(str::to_owned);
                    (id.to_owned(), (parent, depth))
                })
                .collect(),
        })
    }

    pub fn parent(&self, id: &str) -> Option<&str> {
        self.entries
            .get(id)
            .and_then(|(parent, _)| parent.as_deref())
    }

    /// Find the common render root after the caller selects each endpoint's first root.
    pub fn common_root<'a>(
        &'a self,
        mut left: Option<&'a str>,
        mut right: Option<&'a str>,
        work_meter: &OperationWorkMeter,
        work_phase: OperationPhase,
    ) -> Result<Option<&'a str>> {
        let mut left_depth = left.map_or(0, |root| self.depth(root));
        let mut right_depth = right.map_or(0, |root| self.depth(root));

        while left_depth > right_depth {
            work_meter.charge_work(1, work_phase)?;
            left = left.and_then(|root| self.parent(root));
            left_depth -= 1;
        }
        while right_depth > left_depth {
            work_meter.charge_work(1, work_phase)?;
            right = right.and_then(|root| self.parent(root));
            right_depth -= 1;
        }

        while let (Some(left_root), Some(right_root)) = (left, right) {
            work_meter.charge_work(2, work_phase)?;
            if left_root == right_root {
                return Ok(Some(left_root));
            }
            left = self.parent(left_root);
            right = self.parent(right_root);
        }
        Ok(None)
    }

    fn depth(&self, id: &str) -> usize {
        self.entries.get(id).map_or(1, |(_, depth)| *depth)
    }
}

fn flowchart_parent_cycle_error() -> Error {
    Error::InvalidModel {
        message: "Flowchart parent hierarchy contains a cycle".to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use merman_render_core::resources::{RenderResourcePolicy, ResourceLimitId};

    fn parent_fixture() -> FxHashMap<&'static str, &'static str> {
        FxHashMap::from_iter([
            ("leaf-a", "transparent"),
            ("leaf-b", "transparent"),
            ("transparent", "inner"),
            ("inner", "outer"),
        ])
    }

    fn prepare_parent_index(
        work_meter: &OperationWorkMeter,
    ) -> Result<FlowchartEffectiveParentIndex> {
        FlowchartEffectiveParentIndex::prepare(
            &parent_fixture(),
            |id| id == "transparent",
            work_meter,
        )
    }

    #[test]
    fn cluster_ancestry_index_precomputes_strict_relationships_with_exact_work() {
        let parents =
            FxHashMap::from_iter([("inner", "outer"), ("leaf", "inner"), ("sibling", "outer")]);
        let subgraphs =
            FxHashMap::from_iter([("outer", ()), ("inner", ()), ("leaf", ()), ("sibling", ())]);
        let layout_clusters = FxHashMap::from_iter([("outer", ()), ("inner", ())]);
        let unique_cluster_count = 4usize;
        let work_meter =
            OperationWorkMeter::new(RenderResourcePolicy::unbounded_for_trusted_input());
        let index = FlowchartAncestryIndex::prepare(
            &subgraphs,
            &layout_clusters,
            &parents,
            &work_meter,
            OperationPhase::Layout,
        )
        .expect("prepare cluster ancestry");

        assert!(index.is_strict_ancestor("outer", "inner"));
        assert!(index.is_strict_ancestor("outer", "leaf"));
        assert!(index.is_strict_ancestor("inner", "leaf"));
        assert!(!index.is_strict_ancestor("inner", "sibling"));
        assert!(!index.is_strict_ancestor("outer", "outer"));
        assert!(!index.is_strict_ancestor("unknown", "leaf"));
        assert_eq!(
            work_meter.used(),
            subgraphs.len() + layout_clusters.len() + parents.len() * 2 + unique_cluster_count * 4
        );
    }

    #[test]
    fn ungrouped_ancestry_checks_cancellation_in_the_paint_work_phase() {
        let clusters = FxHashMap::<&str, ()>::default();
        let parents = FxHashMap::default();
        for phase in [OperationPhase::Layout, OperationPhase::Emit] {
            for successful_checkpoints in 0..4 {
                let control = merman_model::OperationControl::new();
                control.cancel_after_checkpoints(successful_checkpoints);
                let meter = OperationWorkMeter::new_with_control(
                    RenderResourcePolicy::unbounded_for_trusted_input(),
                    control,
                );
                assert!(matches!(
                    FlowchartAncestryIndex::prepare(&clusters, &clusters, &parents, &meter, phase),
                    Err(Error::Cancelled(error)) if error.phase == phase
                ));
                assert_eq!(meter.used(), 0);
            }
        }
    }

    #[test]
    fn effective_parent_index_flattens_and_charges_each_physical_hop_once() {
        let work_meter =
            OperationWorkMeter::new(RenderResourcePolicy::unbounded_for_trusted_input());
        let index = prepare_parent_index(&work_meter).expect("prepare effective parent index");

        assert_eq!(index.parent("leaf-a"), Some("inner"));
        assert_eq!(index.parent("leaf-b"), Some("inner"));
        assert_eq!(index.parent("transparent"), Some("inner"));
        assert_eq!(index.parent("inner"), Some("outer"));
        assert_eq!(index.parent("outer"), None);
        assert_eq!(index.depth("leaf-a"), 3);
        assert_eq!(index.depth("leaf-b"), 3);
        assert_eq!(index.depth("inner"), 2);
        assert_eq!(index.depth("outer"), 1);

        // Four entries are scanned, each of the four physical parent edges is charged exactly
        // once, four flattened entries are scanned for depth preparation, and five unique
        // effective-hierarchy nodes receive a cached depth.
        assert_eq!(work_meter.used(), 17);
    }

    #[test]
    fn flattened_depth_lca_preserves_nearest_render_root() {
        let work_meter =
            OperationWorkMeter::new(RenderResourcePolicy::unbounded_for_trusted_input());
        let index = prepare_parent_index(&work_meter).expect("prepare effective parent index");
        let work_before_queries = work_meter.used();

        assert_eq!(
            index
                .common_root(
                    Some("inner"),
                    Some("inner"),
                    &work_meter,
                    OperationPhase::Layout
                )
                .expect("same-root LCA"),
            Some("inner")
        );
        assert_eq!(work_meter.used() - work_before_queries, 2);
        assert_eq!(
            index
                .common_root(
                    Some("inner"),
                    Some("outer"),
                    &work_meter,
                    OperationPhase::Layout
                )
                .expect("ancestor LCA"),
            Some("outer")
        );
        assert_eq!(work_meter.used() - work_before_queries, 5);
        assert_eq!(
            index
                .common_root(None, Some("outer"), &work_meter, OperationPhase::Layout)
                .expect("top-root LCA"),
            None
        );
        assert_eq!(work_meter.used() - work_before_queries, 6);
    }

    #[test]
    fn effective_parent_index_rejects_one_unit_below_exact_parent_work() {
        let measuring_meter =
            OperationWorkMeter::new(RenderResourcePolicy::unbounded_for_trusted_input());
        prepare_parent_index(&measuring_meter).expect("measure exact parent work");
        let exact = measuring_meter.used();
        assert!(exact > 0);

        let exact_meter = OperationWorkMeter::new(
            RenderResourcePolicy::unbounded_for_trusted_input()
                .with_limit(ResourceLimitId::MaxLayoutWorkUnits, exact)
                .expect("exact work limit"),
        );
        prepare_parent_index(&exact_meter).expect("exact parent work must succeed");
        assert_eq!(exact_meter.used(), exact);

        let short_meter = OperationWorkMeter::new(
            RenderResourcePolicy::unbounded_for_trusted_input()
                .with_limit(ResourceLimitId::MaxLayoutWorkUnits, exact - 1)
                .expect("short work limit"),
        );
        let error = prepare_parent_index(&short_meter)
            .expect_err("one-unit-short parent budget must fail closed");
        let Error::ResourceLimitExceeded(error) = error else {
            panic!("expected structured layout work rejection");
        };
        assert_eq!(error.limit, "max_layout_work_units");
        assert_eq!(error.actual, exact);
        assert_eq!(error.max, exact - 1);
        assert_eq!(short_meter.used(), exact - 1);
    }
}
