use super::rank::RankLevels;
use super::{GridCoord, charge_sort_work, index_order, node_indices_by_id, try_reserve_vec};
use crate::error::Result;
use crate::graph::demand::SceneDemand;
use crate::graph::layout::NodeLayout;
use crate::graph::model::{AsciiGraph, GraphDirection, GraphNodeSide};
use crate::graph::shape::GraphNodeShapeSize;
use crate::operation::AsciiExecution;
use crate::options::GraphLayoutPolicy;
use crate::resource::ResourceContext;
use std::cmp::Ordering;

const NODE_PITCH: usize = 4;
const ORDERING_SWEEPS: usize = 4;

#[derive(Clone, Copy)]
struct Barycenter {
    total: usize,
    count: usize,
}

impl Barycenter {
    fn compare(self, other: Self) -> Ordering {
        ((self.total as u128) * (other.count as u128))
            .cmp(&((other.total as u128) * (self.count as u128)))
    }
}

struct AlignmentBlock {
    end: usize,
    total: i128,
    weight: usize,
}

pub(super) fn place(
    ranks: &RankLevels,
    direction: GraphDirection,
    resources: &ResourceContext,
    execution: AsciiExecution<'_>,
) -> Result<Vec<GridCoord>> {
    let count = ranks.nodes.len();
    resources.charge_layout_work(resources.checked_work_mul(count, 5)?)?;
    let mut order = index_order(count)?;
    charge_sort_work(count, resources)?;
    order.sort_unstable_by_key(|&index| (ranks.nodes[index], side_order(ranks, direction, index)));
    let mut layers = Vec::new();
    try_reserve_vec(&mut layers, count)?;
    let mut positions = Vec::new();
    try_reserve_vec(&mut positions, count)?;
    positions.resize(count, 0);
    let mut start = 0;
    for end in 1..=count {
        resources.checkpoint()?;
        if end == count || ranks.nodes[order[end]] != ranks.nodes[order[start]] {
            for (lane, &index) in order[start..end].iter().enumerate() {
                positions[index] = resources.checked_grid_mul(lane, NODE_PITCH)?;
            }
            layers.push(start..end);
            start = end;
        }
    }
    let has_wide_layer = layers.iter().any(|layer| layer.len() > 1);
    let children = if has_wide_layer {
        child_indices(&ranks.parent_indices, resources)?
    } else {
        Vec::new()
    };
    let mut scores = Vec::new();
    try_reserve_vec(&mut scores, count)?;
    scores.resize(count, Barycenter { total: 0, count: 1 });
    let mut blocks = Vec::new();
    try_reserve_vec(&mut blocks, count)?;
    let mut offsets = Vec::new();
    try_reserve_vec(&mut offsets, count)?;
    offsets.resize(count, 0);
    for layer in &layers {
        for (lane, &index) in order[layer.clone()].iter().enumerate() {
            offsets[index] = resources.checked_grid_mul(lane, NODE_PITCH)?;
        }
    }

    for _ in 0..if has_wide_layer { ORDERING_SWEEPS } else { 0 } {
        let mut changed = false;
        for forward in [true, false] {
            for iteration in 0..layers.len() {
                execution.checkpoint_loop(merman_model::OperationPhase::Layout, iteration)?;
                let layer = if forward {
                    &layers[iteration]
                } else {
                    &layers[layers.len() - 1 - iteration]
                };
                let neighbors = if forward {
                    &ranks.parent_indices
                } else {
                    &children
                };
                changed |= sweep_layer(
                    &mut order[layer.clone()],
                    neighbors,
                    ranks,
                    direction,
                    &mut positions,
                    &mut scores,
                    &mut blocks,
                    &mut offsets,
                    resources,
                )?;
            }
        }
        resources.charge_layout_work(count)?;
        let minimum = positions.iter().copied().min().unwrap_or_default();
        for position in &mut positions {
            *position -= minimum;
        }
        if !changed {
            break;
        }
    }

    resources.charge_layout_work(count)?;
    let mut placements = Vec::new();
    try_reserve_vec(&mut placements, count)?;
    for (index, cross) in positions.into_iter().enumerate() {
        resources.checkpoint()?;
        placements.push(match direction.canonical() {
            GraphDirection::LeftRight => GridCoord {
                x: ranks.nodes[index],
                y: cross,
            },
            GraphDirection::TopDown => GridCoord {
                x: cross,
                y: ranks.nodes[index],
            },
            GraphDirection::RightLeft | GraphDirection::BottomTop => unreachable!(),
        });
    }
    Ok(placements)
}

pub(super) fn compact_nodes(
    graph: &AsciiGraph,
    nodes: &mut [NodeLayout],
    sizes: &[GraphNodeShapeSize],
    demand: &SceneDemand<'_>,
    policy: &GraphLayoutPolicy,
    resources: &ResourceContext,
) -> Result<()> {
    let count = nodes.len();
    resources.charge_layout_work(resources.checked_work_mul(count, 8)?)?;
    let horizontal = graph.direction.canonical() == GraphDirection::LeftRight;
    let rank = |index: usize| {
        if horizontal {
            nodes[index].grid.x
        } else {
            nodes[index].grid.y
        }
    };
    let mut order = index_order(count)?;
    charge_sort_work(count, resources)?;
    order.sort_unstable_by_key(|&index| {
        (
            rank(index),
            if horizontal {
                nodes[index].grid.y
            } else {
                nodes[index].grid.x
            },
        )
    });
    let mut layers = Vec::new();
    try_reserve_vec(&mut layers, count)?;
    let mut parents = Vec::new();
    try_reserve_vec(&mut parents, count)?;
    parents.resize_with(count, Vec::new);
    let indices = node_indices_by_id(graph)?;
    for edge in &graph.edges {
        resources.checkpoint()?;
        resources.charge_layout_work(1)?;
        let (Some(&from), Some(&to)) = (
            indices.get(edge.from.as_str()),
            indices.get(edge.to.as_str()),
        ) else {
            continue;
        };
        if rank(from) < rank(to) {
            try_reserve_vec(&mut parents[to], 1)?;
            parents[to].push(from);
        }
    }
    let children = child_indices(&parents, resources)?;
    let mut positions = index_order(count)?;
    let mut offsets = index_order(count)?;
    let gap = if horizontal {
        policy.rank_gap_y
    } else {
        policy.rank_gap_x
    };
    let mut start = 0;
    for end in 1..=count {
        resources.checkpoint()?;
        if end == count || rank(order[end]) != rank(order[start]) {
            let mut cursor = 0;
            for &index in &order[start..end] {
                let size = if horizontal {
                    sizes[index].height
                } else {
                    sizes[index].width
                };
                let extent = demand.cross_extent(&nodes[index].id, size, resources)?;
                let center = resources.checked_grid_add(cursor, extent / 2)?;
                offsets[index] = resources.checked_grid_mul(center, 2)?;
                positions[index] = offsets[index];
                cursor =
                    resources.checked_grid_add(resources.checked_grid_add(cursor, extent)?, gap)?;
            }
            layers.push(start..end);
            start = end;
        }
    }
    let mut scores = Vec::new();
    try_reserve_vec(&mut scores, count)?;
    scores.resize(count, Barycenter { total: 0, count: 1 });
    let mut blocks = Vec::new();
    try_reserve_vec(&mut blocks, count)?;
    for _ in 0..ORDERING_SWEEPS {
        let mut changed = false;
        for forward in [true, false] {
            for iteration in 0..layers.len() {
                resources.checkpoint()?;
                let layer = if forward {
                    &layers[iteration]
                } else {
                    &layers[layers.len() - 1 - iteration]
                };
                let layer = &order[layer.clone()];
                neighbor_scores(
                    layer,
                    if forward { &parents } else { &children },
                    &positions,
                    &mut scores,
                    resources,
                )?;
                changed |= compact_layer(
                    layer,
                    &mut positions,
                    &scores,
                    &mut blocks,
                    &offsets,
                    resources,
                )?;
            }
        }
        if !changed {
            break;
        }
    }
    let mut main = 0;
    let mut previous_end = None::<usize>;
    for layer in &layers {
        resources.checkpoint()?;
        resources.charge_layout_work(layer.len())?;
        let mut original_start = usize::MAX;
        let mut original_end = 0;
        let mut main_size = 0;
        for &index in &order[layer.clone()] {
            let node = &nodes[index];
            let (start, end, size) = if horizontal {
                (node.x, node.right(), sizes[index].width)
            } else {
                (node.y, node.bottom(), sizes[index].height)
            };
            original_start = original_start.min(start);
            original_end = original_end.max(end);
            main_size = main_size.max(size);
        }
        let gap = previous_end.map_or(original_start, |end| original_start.saturating_sub(end + 1));
        main = resources.checked_grid_add(main, gap)?;
        for &index in &order[layer.clone()] {
            if horizontal {
                nodes[index].x = main;
            } else {
                nodes[index].y = main;
            }
        }
        main = resources.checked_grid_add(main, main_size)?;
        previous_end = Some(original_end);
    }
    for (index, node) in nodes.iter_mut().enumerate() {
        resources.checkpoint()?;
        let size = sizes[index];
        node.width = size.width;
        node.height = size.height;
        if horizontal {
            node.y = positions[index] / 2 - size.height / 2;
        } else {
            node.x = positions[index] / 2 - size.width / 2;
        }
    }
    Ok(())
}

fn side_order(ranks: &RankLevels, direction: GraphDirection, index: usize) -> (usize, u8, usize) {
    if direction.canonical() == GraphDirection::TopDown
        && let Some(constraint) = ranks.side_constraints[index]
    {
        return (
            constraint.anchor_node_index,
            match constraint.side {
                GraphNodeSide::Left => 0,
                GraphNodeSide::Right => 2,
            },
            index,
        );
    }
    (index, 1, index)
}

fn child_indices(parents: &[Vec<usize>], resources: &ResourceContext) -> Result<Vec<Vec<usize>>> {
    resources.charge_layout_work(parents.len())?;
    let mut children = Vec::new();
    try_reserve_vec(&mut children, parents.len())?;
    children.resize_with(parents.len(), Vec::new);
    for (child, incoming) in parents.iter().enumerate() {
        resources.checkpoint()?;
        resources.charge_layout_work(incoming.len())?;
        for (iteration, &parent) in incoming.iter().enumerate() {
            if iteration.is_multiple_of(64) {
                resources.checkpoint()?;
            }
            try_reserve_vec(&mut children[parent], 1)?;
            children[parent].push(child);
        }
    }
    Ok(children)
}

#[allow(clippy::too_many_arguments)]
fn sweep_layer(
    layer: &mut [usize],
    neighbors: &[Vec<usize>],
    ranks: &RankLevels,
    direction: GraphDirection,
    positions: &mut [usize],
    scores: &mut [Barycenter],
    blocks: &mut Vec<AlignmentBlock>,
    offsets: &mut [usize],
    resources: &ResourceContext,
) -> Result<bool> {
    neighbor_scores(layer, neighbors, positions, scores, resources)?;
    charge_sort_work(layer.len(), resources)?;
    layer.sort_unstable_by(|&left, &right| {
        let left_side = side_order(ranks, direction, left);
        let right_side = side_order(ranks, direction, right);
        scores[left_side.0]
            .compare(scores[right_side.0])
            .then_with(|| positions[left_side.0].cmp(&positions[right_side.0]))
            .then_with(|| left_side.cmp(&right_side))
    });
    for (lane, &index) in layer.iter().enumerate() {
        offsets[index] = resources.checked_grid_mul(lane, NODE_PITCH)?;
    }
    compact_layer(layer, positions, scores, blocks, offsets, resources)
}

fn neighbor_scores(
    layer: &[usize],
    neighbors: &[Vec<usize>],
    positions: &[usize],
    scores: &mut [Barycenter],
    resources: &ResourceContext,
) -> Result<()> {
    resources.charge_layout_work(layer.len())?;
    for &index in layer.iter() {
        resources.checkpoint()?;
        let adjacent = &neighbors[index];
        resources.charge_layout_work(adjacent.len())?;
        let mut total = 0;
        for (iteration, &neighbor) in adjacent.iter().enumerate() {
            if iteration.is_multiple_of(64) {
                resources.checkpoint()?;
            }
            total = resources.checked_work_add(total, positions[neighbor])?;
        }
        scores[index] = if adjacent.is_empty() {
            Barycenter {
                total: positions[index],
                count: 1,
            }
        } else {
            Barycenter {
                total,
                count: adjacent.len(),
            }
        };
    }
    Ok(())
}

fn compact_layer(
    layer: &[usize],
    positions: &mut [usize],
    scores: &[Barycenter],
    blocks: &mut Vec<AlignmentBlock>,
    offsets: &[usize],
    resources: &ResourceContext,
) -> Result<bool> {
    blocks.clear();
    let mut changed = false;
    resources.charge_layout_work(resources.checked_work_mul(layer.len(), 3)?)?;
    for (lane, &index) in layer.iter().enumerate() {
        resources.checkpoint()?;
        let score = scores[index];
        let offset = (offsets[index] as i128)
            .checked_mul(score.count as i128)
            .ok_or_else(|| resources.work_overflow())?;
        blocks.push(AlignmentBlock {
            end: lane + 1,
            total: score.total as i128 - offset,
            weight: score.count,
        });
        while blocks.len() >= 2 {
            resources.charge_layout_work(1)?;
            let right = &blocks[blocks.len() - 1];
            let left = &blocks[blocks.len() - 2];
            let left_mean = left
                .total
                .checked_mul(right.weight as i128)
                .ok_or_else(|| resources.work_overflow())?;
            let right_mean = right
                .total
                .checked_mul(left.weight as i128)
                .ok_or_else(|| resources.work_overflow())?;
            if left_mean <= right_mean {
                break;
            }
            let right = blocks.pop().expect("two alignment blocks remain");
            let left = blocks.last_mut().expect("the preceding block remains");
            left.end = right.end;
            left.total = left
                .total
                .checked_add(right.total)
                .ok_or_else(|| resources.work_overflow())?;
            left.weight = resources.checked_work_add(left.weight, right.weight)?;
        }
    }
    let minimum = blocks.first().map_or(0, rounded_block_position).min(0);
    let mut start = 0;
    for block in blocks.iter() {
        let position = rounded_block_position(block) - minimum;
        let position = usize::try_from(position).map_err(|_| resources.work_overflow())?;
        for &index in &layer[start..block.end] {
            let next = resources.checked_grid_add(position, offsets[index])?;
            changed |= next != positions[index];
            positions[index] = next;
        }
        start = block.end;
    }
    Ok(changed)
}

fn rounded_block_position(block: &AlignmentBlock) -> i128 {
    let weight = block.weight as i128;
    block.total.div_euclid(weight * 2) * 2
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::AsciiError;
    use crate::resource::AsciiResourcePolicy;
    use merman_model::resources::ResourceProfile;
    use merman_model::{OperationControl, OperationPhase};

    #[test]
    fn half_cell_alignment_does_not_walk_a_layer_on_repeated_sweeps() {
        let resources = ResourceContext::new(AsciiResourcePolicy::unbounded());
        let mut positions = [8, 6, 24];
        let offsets = [8, 6, 24];
        let mut scores = [Barycenter { total: 0, count: 1 }; 3];
        let neighbors = [vec![1, 2], vec![0], vec![0]];
        let mut blocks = Vec::with_capacity(2);
        let mut aligned = None;
        for _ in 0..8 {
            for layer in [&[0][..], &[1, 2][..]] {
                neighbor_scores(layer, &neighbors, &positions, &mut scores, &resources).unwrap();
                compact_layer(
                    layer,
                    &mut positions,
                    &scores,
                    &mut blocks,
                    &offsets,
                    &resources,
                )
                .unwrap();
            }
            if let Some(previous) = aligned {
                assert_eq!(
                    positions, previous,
                    "rounding must not translate every layer"
                );
            }
            aligned = Some(positions);
        }
    }

    #[test]
    fn parallel_edges_do_not_narrow_weighted_alignment_arithmetic_to_pointer_width() {
        let resources = ResourceContext::new(AsciiResourcePolicy::for_profile(
            ResourceProfile::UnboundedForTrustedInput,
        ));
        let mut positions = [0];
        compact_layer(
            &[0],
            &mut positions,
            &[Barycenter {
                total: 0,
                count: 32_769,
            }],
            &mut Vec::with_capacity(1),
            &[131_072],
            &resources,
        )
        .expect("a large weight must not overflow an otherwise representable coordinate");
        assert_eq!(positions, [131_072]);
    }

    #[test]
    fn one_large_parallel_adjacency_remains_cancellable_and_keeps_charged_work() {
        let policy = AsciiResourcePolicy::default();
        let mut resources = ResourceContext::new(policy);
        let control = OperationControl::new();
        let execution = AsciiExecution::new(&control, &policy);
        execution.rebind_resource_context(&mut resources, OperationPhase::Layout);
        control.cancel_after_checkpoints(4);
        let parents = [vec![1; 1024], Vec::new()];
        let error = child_indices(&parents, &resources)
            .expect_err("copying one large adjacency must observe cancellation");
        assert!(matches!(error, AsciiError::Cancelled(_)));
        assert_eq!(resources.layout_work_used(), 1026);
    }
}
