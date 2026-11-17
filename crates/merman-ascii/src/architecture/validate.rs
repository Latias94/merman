//! Admit complete identities, group ownership, and a size-independent spatial plan together.

use super::layout::LayoutPlan;
use super::reserve;
use crate::resource::{AsciiResourceLimitId, ResourceContext};
use crate::{AsciiError, Result};
use merman_model::families::architecture::ArchitectureDiagramModel;
use rustc_hash::FxHashMap;

pub(super) struct Plan {
    /// Real indices are nodes, then groups; the last entry is the virtual root.
    pub(super) children: Vec<Vec<usize>>,
    pub(super) preorder: Vec<usize>,
    pub(super) endpoints: Vec<(usize, usize)>,
    pub(super) anchors: Vec<(Option<usize>, Option<usize>)>,
    pub(super) geometry: LayoutPlan,
}

/// Invalid detached models and incompatible declared alignments retain complete reading records.
pub(super) fn prepare(
    model: &ArchitectureDiagramModel,
    resources: &ResourceContext,
) -> Result<Option<Plan>> {
    resources.checkpoint()?;
    let count = resources.checked_grid_add(model.nodes.len(), model.groups.len())?;
    let hierarchy_count = resources.checked_grid_add(count, 1)?;
    resources.charge_layout_work(resources.checked_work_add(count, model.edges.len())?)?;
    resources.check(AsciiResourceLimitId::MaxDocumentCells, count)?;
    resources.check(AsciiResourceLimitId::MaxOutputBytes, count)?;
    if count == 0 {
        return Ok(None);
    }

    let mut ids = FxHashMap::default();
    ids.try_reserve(count)
        .map_err(|_| AsciiError::allocation_failed("ascii_architecture"))?;
    for (index, id) in model
        .nodes
        .iter()
        .map(|node| node.id.as_str())
        .chain(model.groups.iter().map(|group| group.id.as_str()))
        .enumerate()
    {
        resources.charge_layout_work(id.len().max(1))?;
        if id.is_empty() || ids.insert(id, index).is_some() {
            return Ok(None);
        }
    }

    let mut children = filled::<Vec<usize>>(hierarchy_count, resources)?;
    let mut parents = filled::<usize>(count, resources)?;
    for (index, parent) in model
        .nodes
        .iter()
        .map(|node| node.in_group.as_deref())
        .chain(model.groups.iter().map(|group| group.in_group.as_deref()))
        .enumerate()
    {
        resources.charge_layout_work(1)?;
        let parent = if let Some(id) = parent {
            resources.charge_layout_work(id.len().max(1))?;
            let Some(&parent) = ids.get(id) else {
                return Ok(None);
            };
            if parent < model.nodes.len() || parent == index {
                return Ok(None);
            }
            parent
        } else {
            count
        };
        parents[index] = parent;
        reserve(&mut children[parent], 1)?;
        children[parent].push(index);
    }

    let mut preorder = Vec::new();
    let mut stack = Vec::new();
    reserve(&mut preorder, hierarchy_count)?;
    reserve(&mut stack, hierarchy_count)?;
    stack.push((count, 0usize));
    while let Some((index, depth)) = stack.pop() {
        resources.charge_layout_work(1)?;
        resources.check_nesting_depth(depth)?;
        preorder.push(index);
        let depth = resources.checked_grid_add(depth, 1)?;
        resources.charge_layout_work(children[index].len())?;
        stack.extend(children[index].iter().rev().map(|&child| (child, depth)));
    }
    if preorder.len() != hierarchy_count {
        // Every item has exactly one parent, so cycles are disconnected from the virtual root.
        return Ok(None);
    }

    let mut endpoints = Vec::new();
    let mut anchors = Vec::new();
    reserve(&mut endpoints, model.edges.len())?;
    reserve(&mut anchors, model.edges.len())?;
    for edge in &model.edges {
        resources.charge_layout_work(edge.lhs_id.len().max(1))?;
        resources.charge_layout_work(edge.rhs_id.len().max(1))?;
        if !matches!(edge.lhs_dir, 'L' | 'R' | 'T' | 'B')
            || !matches!(edge.rhs_dir, 'L' | 'R' | 'T' | 'B')
        {
            return Ok(None);
        }
        let (Some(&from), Some(&to)) =
            (ids.get(edge.lhs_id.as_str()), ids.get(edge.rhs_id.as_str()))
        else {
            return Ok(None);
        };
        if from >= model.nodes.len() || to >= model.nodes.len() {
            return Ok(None);
        }
        if (edge.lhs_group == Some(true) || edge.rhs_group == Some(true))
            && parents[from] != count
            && parents[from] == parents[to]
        {
            return Ok(None);
        }
        let contact = |index: usize, group: Option<bool>| {
            if group == Some(true) {
                (parents[index] != count).then_some((parents[index], Some(index)))
            } else {
                Some((index, None))
            }
        };
        let (Some((from, from_anchor)), Some((to, to_anchor))) =
            (contact(from, edge.lhs_group), contact(to, edge.rhs_group))
        else {
            return Ok(None);
        };
        endpoints.push((from, to));
        anchors.push((from_anchor, to_anchor));
    }

    // Adjacency is redundant, but supplied references must still identify incident edges.
    // Empty inventories remain useful for callers constructing canonical models directly.
    for node in &model.nodes {
        for &index in &node.edge_indices {
            resources.charge_layout_work(node.id.len().max(1))?;
            let Some(edge) = model.edges.get(index) else {
                return Ok(None);
            };
            if edge.lhs_id != node.id && edge.rhs_id != node.id {
                return Ok(None);
            }
        }
    }

    let mut seen = filled::<usize>(model.nodes.len(), resources)?;
    for (hint_index, hint) in model.layout_hints.iter().enumerate() {
        resources.charge_layout_work(1)?;
        if hint.members.len() < 2 {
            return Ok(None);
        }
        let stamp = resources.checked_work_add(hint_index, 1)?;
        for id in &hint.members {
            resources.charge_layout_work(id.len().max(1))?;
            let Some(&index) = ids.get(id.as_str()) else {
                return Ok(None);
            };
            if index >= model.nodes.len() || seen[index] == stamp {
                return Ok(None);
            }
            seen[index] = stamp;
        }
    }

    let Some(geometry) = LayoutPlan::prepare(model, &ids, &children, &endpoints, resources)? else {
        return Ok(None);
    };
    Ok(Some(Plan {
        children,
        preorder,
        endpoints,
        anchors,
        geometry,
    }))
}

pub(super) fn filled<T: Default>(count: usize, resources: &ResourceContext) -> Result<Vec<T>> {
    resources.charge_layout_work(count)?;
    let mut values = Vec::new();
    reserve(&mut values, count)?;
    values.resize_with(count, T::default);
    Ok(values)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::graph::positioned::{PositionedBounds, PositionedSize};
    use crate::resource::{AsciiResourceLimitId, AsciiResourcePolicy};
    use merman_model::OperationControl;
    use merman_model::OperationPhase;
    use merman_model::families::architecture::{
        ArchitectureEdgeModel, ArchitectureGroupModel, ArchitectureLayoutDirection,
        ArchitectureLayoutHintModel, ArchitectureNodeKind, ArchitectureNodeModel,
    };

    fn node(id: &str, parent: Option<&str>) -> ArchitectureNodeModel {
        ArchitectureNodeModel {
            id: id.into(),
            node_type: ArchitectureNodeKind::Service,
            edge_indices: Vec::new(),
            icon: None,
            icon_text: None,
            title: None,
            in_group: parent.map(str::to_owned),
        }
    }

    fn group(id: &str, parent: Option<&str>) -> ArchitectureGroupModel {
        ArchitectureGroupModel {
            id: id.into(),
            icon: None,
            title: None,
            in_group: parent.map(str::to_owned),
        }
    }

    fn edge(from: &str, from_side: char, to: &str, to_side: char) -> ArchitectureEdgeModel {
        ArchitectureEdgeModel {
            lhs_id: from.into(),
            lhs_dir: from_side,
            lhs_into: None,
            lhs_group: None,
            rhs_id: to.into(),
            rhs_dir: to_side,
            rhs_into: None,
            rhs_group: None,
            title: None,
        }
    }

    fn hint(
        direction: ArchitectureLayoutDirection,
        members: &[&str],
    ) -> ArchitectureLayoutHintModel {
        ArchitectureLayoutHintModel {
            direction,
            members: members.iter().map(|id| (*id).into()).collect(),
        }
    }

    fn resources() -> ResourceContext {
        ResourceContext::new(AsciiResourcePolicy::default())
    }

    fn center(bounds: PositionedBounds) -> (usize, usize) {
        (bounds.x + bounds.width / 2, bounds.y + bounds.height / 2)
    }

    fn overlaps(a: PositionedBounds, b: PositionedBounds) -> bool {
        a.x < b.x + b.width && b.x < a.x + a.width && a.y < b.y + b.height && b.y < a.y + a.height
    }

    #[test]
    fn measured_cross_group_grid_keeps_centers_order_and_frame_clearance() {
        use ArchitectureLayoutDirection::{Column, Row};
        let model = ArchitectureDiagramModel {
            nodes: vec![
                node("a", Some("left")),
                node("b", Some("left")),
                node("c", Some("right")),
                node("d", Some("right")),
            ],
            groups: vec![group("left", None), group("right", None)],
            layout_hints: vec![
                hint(Row, &["a", "c"]),
                hint(Row, &["b", "d"]),
                hint(Column, &["a", "b"]),
                hint(Column, &["c", "d"]),
            ],
            ..ArchitectureDiagramModel::default()
        };
        let resources = resources();
        let plan = prepare(&model, &resources).unwrap().unwrap();
        assert_eq!(plan.preorder.first(), Some(&6));
        assert_eq!(plan.children.len(), 7);
        for scale in [1, 3, 7] {
            let minimums = [
                PositionedSize {
                    width: 8 * scale,
                    height: 3,
                },
                PositionedSize {
                    width: 5,
                    height: 4 * scale,
                },
                PositionedSize {
                    width: 7,
                    height: 6 * scale,
                },
                PositionedSize {
                    width: 4 * scale,
                    height: 5,
                },
                PositionedSize {
                    width: 19,
                    height: 7 * scale,
                },
                PositionedSize {
                    width: 23 * scale,
                    height: 6,
                },
            ];
            let bounds = plan.bounds(&minimums, &resources).unwrap();
            let centers: Vec<_> = bounds.iter().copied().map(center).collect();
            assert_eq!(centers[0].1, centers[2].1);
            assert_eq!(centers[1].1, centers[3].1);
            assert_eq!(centers[0].0, centers[1].0);
            assert_eq!(centers[2].0, centers[3].0);
            assert!(centers[0].0 < centers[2].0);
            assert!(centers[0].1 < centers[1].1);
            for siblings in &plan.children {
                for (offset, &a) in siblings.iter().enumerate() {
                    for &b in &siblings[offset + 1..] {
                        assert!(!overlaps(bounds[a], bounds[b]));
                    }
                }
            }
            for (index, bounds) in bounds.iter().enumerate() {
                assert!(bounds.width >= minimums[index].width);
                assert!(bounds.height >= minimums[index].height);
                assert!(bounds.x >= 4 && bounds.y >= 4);
            }
            for (parent, children) in [(4, [0, 1]), (5, [2, 3])] {
                for child in children {
                    assert!(bounds[child].x >= bounds[parent].x + 3);
                    assert!(bounds[child].y >= bounds[parent].y + minimums[parent].height - 2);
                    assert!(
                        bounds[child].x + bounds[child].width + 3
                            <= bounds[parent].x + bounds[parent].width
                    );
                    assert!(
                        bounds[child].y + bounds[child].height + 3
                            <= bounds[parent].y + bounds[parent].height
                    );
                }
            }
        }
    }

    #[test]
    fn explicit_order_overrides_direction_but_conflicting_orders_decline_admission() {
        let mut model = ArchitectureDiagramModel {
            nodes: vec![node("a", None), node("b", None)],
            edges: vec![edge("a", 'R', "b", 'L')],
            layout_hints: vec![hint(ArchitectureLayoutDirection::Row, &["b", "a"])],
            ..ArchitectureDiagramModel::default()
        };
        let resources = resources();
        let plan = prepare(&model, &resources).unwrap().unwrap();
        let bounds = plan
            .bounds(
                &[PositionedSize {
                    width: 9,
                    height: 5,
                }; 2],
                &resources,
            )
            .unwrap();
        assert_eq!(center(bounds[0]).1, center(bounds[1]).1);
        assert!(center(bounds[1]).0 < center(bounds[0]).0);
        model
            .layout_hints
            .push(hint(ArchitectureLayoutDirection::Row, &["a", "b"]));
        assert!(prepare(&model, &resources).unwrap().is_none());
    }

    #[test]
    fn aligned_interleaved_groups_cannot_be_disjoint_frames() {
        let model = ArchitectureDiagramModel {
            nodes: vec![
                node("a", Some("g")),
                node("b", Some("h")),
                node("c", Some("g")),
            ],
            groups: vec![group("g", None), group("h", None)],
            layout_hints: vec![hint(ArchitectureLayoutDirection::Row, &["a", "b", "c"])],
            ..ArchitectureDiagramModel::default()
        };
        assert!(prepare(&model, &resources()).unwrap().is_none());
    }

    #[test]
    fn directional_cycles_and_same_side_edges_keep_spatial_admission() {
        let model = ArchitectureDiagramModel {
            nodes: vec![node("a", None), node("b", None), node("c", None)],
            edges: vec![
                edge("a", 'R', "b", 'L'),
                edge("b", 'R', "c", 'L'),
                edge("c", 'R', "a", 'L'),
                edge("a", 'T', "b", 'T'),
            ],
            ..ArchitectureDiagramModel::default()
        };
        let resources = resources();
        let plan = prepare(&model, &resources).unwrap().unwrap();
        assert_eq!(plan.endpoints.len(), 4);
        assert!(
            plan.bounds(
                &[PositionedSize {
                    width: 9,
                    height: 5
                }; 3],
                &resources
            )
            .is_ok()
        );
    }

    #[test]
    fn group_contacts_keep_owning_frame_and_node_anchor_including_junctions() {
        let mut model = ArchitectureDiagramModel {
            nodes: vec![node("j", Some("g")), node("b", None)],
            groups: vec![group("g", None)],
            edges: vec![edge("j", 'L', "b", 'R')],
            ..ArchitectureDiagramModel::default()
        };
        model.nodes[0].node_type = ArchitectureNodeKind::Junction;
        model.edges[0].lhs_group = Some(true);
        let resources = resources();
        let plan = prepare(&model, &resources).unwrap().unwrap();
        assert_eq!(plan.endpoints, [(2, 1)]);
        assert_eq!(plan.anchors, [(Some(0), None)]);
        model.nodes[1].in_group = Some("g".into());
        assert!(prepare(&model, &resources).unwrap().is_none());
        model.nodes[0].in_group = None;
        assert!(prepare(&model, &resources).unwrap().is_none());
    }

    #[test]
    fn side_demand_grows_odd_contours_before_placement() {
        let mut model = ArchitectureDiagramModel {
            nodes: vec![node("a", None), node("b", None)],
            ..ArchitectureDiagramModel::default()
        };
        for _ in 0..6 {
            model.edges.push(edge("a", 'R', "b", 'L'));
            model.edges.push(edge("a", 'T', "b", 'B'));
        }
        let resources = resources();
        let plan = prepare(&model, &resources).unwrap().unwrap();
        let bounds = plan
            .bounds(
                &[PositionedSize {
                    width: 3,
                    height: 3,
                }; 2],
                &resources,
            )
            .unwrap();
        for bounds in bounds {
            assert!(bounds.width >= 15 && bounds.height >= 15);
            assert_eq!(bounds.width % 2, 1);
            assert_eq!(bounds.height % 2, 1);
        }
    }

    #[test]
    fn invalid_detached_references_and_group_cycles_decline_admission() {
        let mut model = ArchitectureDiagramModel {
            nodes: vec![node("a", None)],
            groups: vec![group("a", None)],
            ..ArchitectureDiagramModel::default()
        };
        let resources = resources();
        assert!(prepare(&model, &resources).unwrap().is_none());
        model.groups = vec![group("g", Some("h")), group("h", Some("g"))];
        assert!(prepare(&model, &resources).unwrap().is_none());
        model.groups.clear();
        model.nodes[0].edge_indices.push(0);
        assert!(prepare(&model, &resources).unwrap().is_none());
        model.nodes[0].edge_indices.clear();
        model.edges.push(edge("a", 'R', "missing", 'L'));
        assert!(prepare(&model, &resources).unwrap().is_none());
    }

    #[test]
    fn cancellation_and_shared_work_limits_propagate_instead_of_declining_admission() {
        let model = ArchitectureDiagramModel {
            nodes: vec![node("a", None)],
            ..ArchitectureDiagramModel::default()
        };
        let control = OperationControl::new();
        control.cancel();
        let resources = resources().controlled(control, OperationPhase::Layout);
        assert!(matches!(
            prepare(&model, &resources),
            Err(AsciiError::Cancelled(_))
        ));
        let policy = AsciiResourcePolicy::default()
            .with_limit(AsciiResourceLimitId::MaxLayoutWorkUnits, 1)
            .unwrap();
        assert!(matches!(
            prepare(&model, &ResourceContext::new(policy)),
            Err(AsciiError::ResourceLimitExceeded(_))
        ));
    }
}
