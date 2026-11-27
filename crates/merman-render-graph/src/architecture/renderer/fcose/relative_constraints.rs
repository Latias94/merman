//! Plans and replays Mermaid’s automatic relative placement constraints.

use indexmap::IndexMap;
use manatee::algo::fcose::IndexedRelativePlacementConstraint;
use rustc_hash::{FxHashMap, FxHashSet};

const RELATIVE_DIRS: [(char, (i32, i32)); 4] =
    [('L', (-1, 0)), ('R', (1, 0)), ('T', (0, 1)), ('B', (0, -1))];

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct PositionPlan {
    vertex: usize,
    distance: usize,
    queue_multiplicity: usize,
}

#[derive(Debug, Clone, Copy)]
enum Endpoints {
    Horizontal { left: usize, right: usize },
    Vertical { top: usize, bottom: usize },
}

impl Endpoints {
    fn with_gap(self, gap: f64) -> IndexedRelativePlacementConstraint {
        match self {
            Self::Horizontal { left, right } => IndexedRelativePlacementConstraint {
                left: Some(left),
                right: Some(right),
                top: None,
                bottom: None,
                gap,
            },
            Self::Vertical { top, bottom } => IndexedRelativePlacementConstraint {
                left: None,
                right: None,
                top: Some(top),
                bottom: Some(bottom),
                gap,
            },
        }
    }
}

#[derive(Debug)]
struct ForwardArc {
    target: usize,
    endpoints: Option<Endpoints>,
}

#[derive(Debug)]
struct SpatialPlan {
    forward: Vec<std::ops::Range<usize>>,
    arcs: Vec<ForwardArc>,
    layer_offsets: Vec<usize>,
    queue_entry_count: usize,
    constraint_count: usize,
}

#[derive(Debug)]
pub(super) struct RelativeConstraintPlan {
    spatial: Vec<SpatialPlan>,
    queue_entry_count: usize,
    constraint_count: usize,
}

impl RelativeConstraintPlan {
    pub(super) fn checked_new(
        spatial_maps: &[IndexMap<&str, (i32, i32)>],
        node_index_by_id: &FxHashMap<&str, usize>,
        declared_pairs: &FxHashSet<(usize, usize)>,
    ) -> Option<Self> {
        let mut spatial = Vec::with_capacity(spatial_maps.len());
        let mut total_queue_entries = 0usize;
        let mut total_constraints = 0usize;

        for spatial_map in spatial_maps {
            let mut inv: FxHashMap<(i32, i32), &str> = FxHashMap::default();
            inv.reserve(spatial_map.len().saturating_mul(2));
            for (id, (x, y)) in spatial_map.iter() {
                inv.insert((*x, *y), *id);
            }

            let mut positions: FxHashMap<(i32, i32), PositionPlan> = FxHashMap::default();
            positions.reserve(inv.len().saturating_mul(2));
            positions.insert(
                (0, 0),
                PositionPlan {
                    vertex: 0,
                    distance: 0,
                    queue_multiplicity: 1,
                },
            );
            let mut unique_queue = std::collections::VecDeque::from([(0, 0)]);
            let mut forward = Vec::new();
            let mut arcs = Vec::new();
            let mut layer_offsets = Vec::new();
            let mut constraint_count = 0usize;

            while let Some(curr) = unique_queue.pop_front() {
                let curr_plan = *positions.get(&curr)?;
                debug_assert_eq!(curr_plan.vertex, forward.len());
                let arc_start = arcs.len();
                if layer_offsets.len() == curr_plan.distance {
                    layer_offsets.push(constraint_count);
                }
                let Some(&curr_id) = inv.get(&curr) else {
                    forward.push(arc_start..arc_start);
                    continue;
                };
                let source_index = node_index_by_id.get(curr_id).copied();
                let next_distance = curr_plan.distance.checked_add(1)?;
                for (dir, (sx, sy)) in RELATIVE_DIRS {
                    let new_pos = (curr.0.checked_add(sx)?, curr.1.checked_add(sy)?);
                    let Some(&new_id) = inv.get(&new_pos) else {
                        continue;
                    };
                    let target = match positions.get_mut(&new_pos) {
                        Some(existing) if existing.distance == next_distance => {
                            existing.queue_multiplicity = existing
                                .queue_multiplicity
                                .checked_add(curr_plan.queue_multiplicity)?;
                            existing.vertex
                        }
                        Some(_) => continue,
                        None => {
                            let vertex = positions.len();
                            positions.insert(
                                new_pos,
                                PositionPlan {
                                    vertex,
                                    distance: next_distance,
                                    queue_multiplicity: curr_plan.queue_multiplicity,
                                },
                            );
                            unique_queue.push_back(new_pos);
                            vertex
                        }
                    };
                    let endpoints = match source_index.zip(node_index_by_id.get(new_id).copied()) {
                        Some((curr_idx, new_idx))
                            if !declared_pairs.contains(&(curr_idx, new_idx))
                                && !declared_pairs.contains(&(new_idx, curr_idx)) =>
                        {
                            constraint_count =
                                constraint_count.checked_add(curr_plan.queue_multiplicity)?;
                            Some(match dir {
                                'L' => Endpoints::Horizontal {
                                    left: new_idx,
                                    right: curr_idx,
                                },
                                'R' => Endpoints::Horizontal {
                                    left: curr_idx,
                                    right: new_idx,
                                },
                                'T' => Endpoints::Vertical {
                                    top: new_idx,
                                    bottom: curr_idx,
                                },
                                'B' => Endpoints::Vertical {
                                    top: curr_idx,
                                    bottom: new_idx,
                                },
                                _ => return None,
                            })
                        }
                        _ => None,
                    };
                    arcs.push(ForwardArc { target, endpoints });
                }
                forward.push(arc_start..arcs.len());
            }

            let queue_entry_count = positions.values().try_fold(0usize, |total, position| {
                total.checked_add(position.queue_multiplicity)
            })?;
            total_queue_entries = total_queue_entries.checked_add(queue_entry_count)?;
            total_constraints = total_constraints.checked_add(constraint_count)?;
            spatial.push(SpatialPlan {
                forward,
                arcs,
                layer_offsets,
                queue_entry_count,
                constraint_count,
            });
        }

        Some(Self {
            spatial,
            queue_entry_count: total_queue_entries,
            constraint_count: total_constraints,
        })
    }

    pub(super) const fn constraint_count(&self) -> usize {
        self.constraint_count
    }

    pub(super) fn checked_materialization_work_units(&self) -> Option<usize> {
        let constraint_bytes = self
            .constraint_count
            .checked_mul(std::mem::size_of::<IndexedRelativePlacementConstraint>())?;
        let queue_bytes = self
            .queue_entry_count
            .checked_mul(std::mem::size_of::<(i32, i32)>())?;
        if constraint_bytes > isize::MAX as usize || queue_bytes > isize::MAX as usize {
            return None;
        }
        // Keep admission based on the expanded Mermaid traversal, including positions that
        // traverse declared pairs without emitting a constraint.
        self.queue_entry_count
            .checked_mul(8)?
            .checked_add(self.constraint_count)
    }

    /// Replays the plan after the caller admits `checked_materialization_work_units`.
    pub(super) fn materialize(&self, gap: f64) -> Option<Vec<IndexedRelativePlacementConstraint>> {
        let mut relative = Vec::with_capacity(self.constraint_count);
        for spatial in &self.spatial {
            if spatial.constraint_count == 0 {
                continue;
            }
            let output_start = relative.len();
            let output_end = output_start.checked_add(spatial.constraint_count)?;
            relative.resize(
                output_end,
                IndexedRelativePlacementConstraint {
                    left: None,
                    right: None,
                    top: None,
                    bottom: None,
                    gap,
                },
            );
            let output = &mut relative[output_start..output_end];
            let mut cursors = spatial.layer_offsets.clone();
            let mut stack = vec![spatial.forward[0].clone()];
            let mut replayed_entries = 1;

            // Both traversals enumerate direction words in L/R/T/B order within a layer. Give each
            // layer its own output cursor so a depth-first replay needs only one stack frame per
            // active layer, while writing rows in exactly the original breadth-first order.
            while let Some(forward) = stack.last_mut() {
                let Some(arc_index) = forward.next() else {
                    stack.pop();
                    continue;
                };
                let arc = &spatial.arcs[arc_index];
                if cfg!(debug_assertions) {
                    replayed_entries += 1;
                }
                if let Some(endpoints) = arc.endpoints {
                    let cursor = &mut cursors[stack.len() - 1];
                    output[*cursor] = endpoints.with_gap(gap);
                    *cursor += 1;
                }
                let next = &spatial.forward[arc.target];
                if !next.is_empty() {
                    stack.push(next.clone());
                }
            }

            debug_assert_eq!(replayed_entries, spatial.queue_entry_count);
            debug_assert!(
                cursors.into_iter().eq(spatial
                    .layer_offsets
                    .iter()
                    .copied()
                    .skip(1)
                    .chain([spatial.constraint_count]))
            );
        }

        debug_assert_eq!(relative.len(), self.constraint_count);
        Some(relative)
    }
}

pub(super) fn checked_planning_work_units(
    spatial_maps: &[IndexMap<&str, (i32, i32)>],
) -> Option<usize> {
    let source_positions = spatial_maps
        .iter()
        .try_fold(0usize, |total, map| total.checked_add(map.len()))?;
    spatial_maps
        .len()
        .checked_add(source_positions.checked_mul(6)?)
}

#[cfg(test)]
mod tests {
    use super::RelativeConstraintPlan;

    #[test]
    fn architecture_relative_constraints_preserve_mermaid_duplicate_bfs_pops() {
        let mut spatial_map = indexmap::IndexMap::new();
        spatial_map.insert("ingress", (0, 0));
        spatial_map.insert("fork", (1, 0));
        spatial_map.insert("auth", (2, 0));
        spatial_map.insert("api", (1, -1));
        spatial_map.insert("join", (2, -1));
        spatial_map.insert("db", (3, -1));
        spatial_map.insert("cache", (2, -2));

        let mut node_index_by_id = rustc_hash::FxHashMap::default();
        for (idx, id) in ["ingress", "auth", "api", "db", "cache", "fork", "join"]
            .into_iter()
            .enumerate()
        {
            node_index_by_id.insert(id, idx);
        }

        let spatial_maps = [spatial_map];
        let declared_pairs = rustc_hash::FxHashSet::default();
        let plan =
            RelativeConstraintPlan::checked_new(&spatial_maps, &node_index_by_id, &declared_pairs)
                .expect("relative constraint plan");
        let constraints = plan
            .materialize(120.0)
            .expect("relative constraints materialize");

        assert_eq!(plan.constraint_count(), 9);
        assert_eq!(constraints.len(), plan.constraint_count());

        assert_eq!(
            constraints
                .iter()
                .map(|c| (c.left.zip(c.right), c.top.zip(c.bottom)))
                .collect::<Vec<_>>(),
            [
                (Some((0, 5)), None),
                (Some((5, 1)), None),
                (None, Some((5, 2))),
                (None, Some((1, 6))),
                (Some((2, 6)), None),
                (Some((6, 3)), None),
                (None, Some((6, 4))),
                (Some((6, 3)), None),
                (None, Some((6, 4))),
            ],
        );
    }

    #[test]
    fn architecture_relative_plan_counts_grid_path_multiplicity_without_expansion() {
        let ids = (0..12 * 12)
            .map(|index| format!("node-{index}"))
            .collect::<Vec<_>>();
        let mut spatial_map = indexmap::IndexMap::new();
        let mut node_index_by_id = rustc_hash::FxHashMap::default();
        for y in 0..12 {
            for x in 0..12 {
                let index = y * 12 + x;
                let id = ids[index].as_str();
                spatial_map.insert(id, (x as i32, y as i32));
                node_index_by_id.insert(id, index);
            }
        }

        let plan = RelativeConstraintPlan::checked_new(
            &[spatial_map],
            &node_index_by_id,
            &rustc_hash::FxHashSet::default(),
        )
        .expect("12x12 grid count should fit usize");

        assert_eq!(plan.queue_entry_count, 2_704_155);
        assert_eq!(plan.constraint_count(), 2_704_154);
        assert_eq!(plan.spatial.len(), 1);
        assert_eq!(plan.spatial[0].queue_entry_count, 2_704_155);
        assert_eq!(plan.spatial[0].constraint_count, 2_704_154);
    }

    #[test]
    fn architecture_relative_plan_fails_closed_on_path_multiplicity_overflow() {
        let ids = (0..35 * 35)
            .map(|index| format!("node-{index}"))
            .collect::<Vec<_>>();
        let mut spatial_map = indexmap::IndexMap::new();
        let mut node_index_by_id = rustc_hash::FxHashMap::default();
        for y in 0..35 {
            for x in 0..35 {
                let index = y * 35 + x;
                let id = ids[index].as_str();
                spatial_map.insert(id, (x as i32, y as i32));
                node_index_by_id.insert(id, index);
            }
        }

        assert!(
            RelativeConstraintPlan::checked_new(
                &[spatial_map],
                &node_index_by_id,
                &rustc_hash::FxHashSet::default(),
            )
            .is_none()
        );
    }

    #[test]
    #[cfg(target_pointer_width = "64")]
    fn architecture_relative_plan_rejects_unrepresentable_materialization_capacity() {
        let width = 34usize;
        let height = 33usize;
        let ids = (0..width * height)
            .map(|index| format!("node-{index}"))
            .collect::<Vec<_>>();
        let mut spatial_map = indexmap::IndexMap::new();
        let mut node_index_by_id = rustc_hash::FxHashMap::default();
        for y in 0..height {
            for x in 0..width {
                let index = y * width + x;
                let id = ids[index].as_str();
                spatial_map.insert(id, (x as i32, y as i32));
                node_index_by_id.insert(id, index);
            }
        }

        let plan = RelativeConstraintPlan::checked_new(
            &[spatial_map],
            &node_index_by_id,
            &rustc_hash::FxHashSet::default(),
        )
        .expect("path multiplicity should still fit usize");

        assert!(plan.checked_materialization_work_units().is_none());
    }

    #[test]
    fn architecture_relative_plan_matches_materialization_for_all_small_grid_shapes() {
        let positions: [(&str, (i32, i32)); 9] = [
            ("p0", (0, 0)),
            ("p1", (-1, 0)),
            ("p2", (1, 0)),
            ("p3", (0, 1)),
            ("p4", (0, -1)),
            ("p5", (-1, 1)),
            ("p6", (1, 1)),
            ("p7", (-1, -1)),
            ("p8", (1, -1)),
        ];

        for mask in 0usize..(1usize << (positions.len() - 1)) {
            let mut spatial_map = indexmap::IndexMap::new();
            spatial_map.insert(positions[0].0, positions[0].1);
            for (bit, &(id, position)) in positions[1..].iter().enumerate() {
                if mask & (1usize << bit) != 0 {
                    spatial_map.insert(id, position);
                }
            }
            let node_index_by_id = spatial_map
                .keys()
                .enumerate()
                .map(|(index, &id)| (id, index))
                .collect::<rustc_hash::FxHashMap<_, _>>();

            let mut declared_variants = vec![rustc_hash::FxHashSet::default()];
            if let Some(&right) = node_index_by_id.get("p1") {
                declared_variants.push(rustc_hash::FxHashSet::from_iter([(0, right)]));
            }
            let mut all_adjacent = rustc_hash::FxHashSet::default();
            for (&lhs_id, &(lhs_x, lhs_y)) in &spatial_map {
                let lhs = node_index_by_id[lhs_id];
                for (&rhs_id, &(rhs_x, rhs_y)) in &spatial_map {
                    if (lhs_x - rhs_x).abs() + (lhs_y - rhs_y).abs() == 1 {
                        all_adjacent.insert((lhs, node_index_by_id[rhs_id]));
                    }
                }
            }
            declared_variants.push(all_adjacent);

            for declared_pairs in declared_variants {
                let spatial_maps = [spatial_map.clone()];
                let plan = RelativeConstraintPlan::checked_new(
                    &spatial_maps,
                    &node_index_by_id,
                    &declared_pairs,
                )
                .expect("3x3 relative plan should fit");
                let materialized = plan
                    .materialize(120.0)
                    .expect("3x3 relative constraints materialize");
                assert_eq!(
                    plan.constraint_count(),
                    materialized.len(),
                    "constraint count mismatch for mask {mask:#011b}",
                );
            }
        }
    }
}
