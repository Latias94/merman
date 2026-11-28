//! Containment and absolute geometry from Mermaid 12.4.0 `groups.ts` and `layoutCore.ts`.

use super::geometry::{
    GridContainerGeometry, GridGroupMeasure, GridItemGeometry, GridMeasuredTracks, GridSpacing,
    GridTrackGaps, materialize_grid_container, measure_grid_tracks, reserve,
};
use super::placement::{
    GridItem, GridPlacementError, GridPlacementOptions, GridPlacementSpec, resolve_grid_placements,
};
use crate::graph_layout::{Point, Rect, Size};
use crate::resources::OperationWorkMeter;
use crate::{Error, Result};
use merman_model::OperationPhase;
use std::collections::HashMap;

pub(super) const GRID_ROOT_ID: &str = "__grid_root__";

#[derive(Debug, Clone, Copy)]
pub struct GridMeasuredItem<'a> {
    pub item: GridItem<'a>,
    pub parent_id: Option<&'a str>,
    pub kind: GridItemKind,
}

#[derive(Debug, Clone, Copy)]
pub enum GridItemKind {
    Leaf(Size),
    Group(GridGroupMeasure),
    EdgeLabel,
}

#[derive(Debug)]
pub(super) struct GridForest<'a> {
    pub items: &'a [GridMeasuredItem<'a>],
    #[cfg(test)]
    pub by_id: HashMap<&'a str, usize>,
    /// The root lives at `items.len()`. Other entries use measured item slots.
    pub children: Vec<Vec<usize>>,
    pub parents: Vec<Option<usize>>,
    pub postorder: Vec<usize>,
}

impl<'a> GridForest<'a> {
    pub(super) fn new(
        items: &'a [GridMeasuredItem<'a>],
        work: &OperationWorkMeter,
    ) -> Result<Self> {
        let root = items.len();
        let slots = root
            .checked_add(1)
            .ok_or_else(|| work.arithmetic_overflow())?;
        work.charge(slots)?;
        let mut by_id = HashMap::new();
        by_id.try_reserve(root).map_err(|_| allocation_error())?;
        let mut parents = Vec::new();
        reserve(&mut parents, root)?;
        let mut children = Vec::new();
        reserve(&mut children, slots)?;
        children.resize_with(slots, Vec::new);

        for (index, node) in items.iter().enumerate() {
            let id = node.item.id;
            work.charge(id.len().max(1))?;
            if id == GRID_ROOT_ID {
                return Err(containment_error(format!(
                    "Node id \"{id}\" is reserved for the grid layout root"
                )));
            }
            if by_id.insert(id, index).is_some() {
                return Err(containment_error(format!("Duplicate node id \"{id}\"")));
            }
        }
        for (index, node) in items.iter().enumerate() {
            work.charge(1)?;
            if matches!(node.kind, GridItemKind::EdgeLabel) {
                parents.push(None);
                continue;
            }
            let parent = match node.parent_id.filter(|&id| id != GRID_ROOT_ID) {
                None => root,
                Some(id) => {
                    work.charge(id.len().max(1))?;
                    let Some(&parent) = by_id.get(id).filter(|&&parent| is_group(items, parent))
                    else {
                        return Err(containment_error(format!(
                            "Missing parent \"{id}\" for \"{}\"",
                            node.item.id
                        )));
                    };
                    parent
                }
            };
            parents.push(Some(parent));
            children[parent]
                .try_reserve(1)
                .map_err(|_| allocation_error())?;
            children[parent].push(index);
        }

        let mut postorder = Vec::new();
        let mut state = Vec::new();
        reserve(&mut postorder, root)?;
        reserve(&mut state, root)?;
        state.resize(root, 0u8);
        let mut stack = Vec::new();
        reserve(&mut stack, root)?;
        // Follow root children first. A second pass catches disconnected cycles in source order.
        for start in children[root].iter().copied().chain(0..root) {
            work.charge(1)?;
            if !is_group(items, start) || state[start] != 0 {
                continue;
            }
            state[start] = 1;
            stack.push((start, 0));
            while let Some((group, child_slot)) = stack.last_mut() {
                work.charge(1)?;
                let group_index = *group;
                if let Some(&child) = children[group_index].get(*child_slot) {
                    *child_slot += 1;
                    if !is_group(items, child) || state[child] == 2 {
                        continue;
                    }
                    if state[child] == 1 {
                        let id = items[child].item.id;
                        return Err(containment_error(format!("Containment cycle at \"{id}\"")));
                    }
                    state[child] = 1;
                    stack.push((child, 0));
                } else {
                    state[group_index] = 2;
                    postorder.push(group_index);
                    stack.pop();
                }
            }
        }
        work.checkpoint(OperationPhase::Layout)?;
        Ok(Self {
            items,
            #[cfg(test)]
            by_id,
            children,
            parents,
            postorder,
        })
    }
}

fn is_group(items: &[GridMeasuredItem<'_>], index: usize) -> bool {
    matches!(items[index].kind, GridItemKind::Group(_))
}

fn containment_error(message: String) -> Error {
    Error::InvalidModel {
        message: format!("GRID_INVALID_CONTAINMENT: {message}"),
    }
}

fn allocation_error() -> Error {
    Error::LayoutAllocationFailed { backend: "grid" }
}

#[derive(Debug)]
pub(super) struct GridForestGeometry {
    pub containers: Vec<Option<GridContainerGeometry>>,
    // Lookup by measured item, including group shells. Helpers have no geometry.
    locations: Vec<Option<(usize, usize)>>,
}

impl GridForestGeometry {
    pub(super) fn item(&self, slot: usize) -> Option<&GridItemGeometry> {
        let (container, index) = *self.locations.get(slot)?.as_ref()?;
        self.containers[container].as_ref()?.items.get(index)
    }

    pub(super) fn root(&self) -> &GridContainerGeometry {
        self.containers
            .last()
            .and_then(Option::as_ref)
            .expect("the root container was laid out")
    }
}

pub(super) fn layout_measured_forest(
    forest: &GridForest<'_>,
    occurrences: &HashMap<&str, &[GridPlacementSpec]>,
    options: GridPlacementOptions,
    spacing: GridSpacing,
    mut label_gaps: impl FnMut(usize, &[usize], &GridMeasuredTracks<'_>) -> Result<GridTrackGaps>,
    work: &OperationWorkMeter,
) -> Result<GridForestGeometry> {
    let root = forest.items.len();
    let total = forest.children.len();
    work.charge(total)?;
    let mut containers = Vec::new();
    reserve(&mut containers, total)?;
    containers.resize_with(total, || None::<GridContainerGeometry>);
    let mut layout_items = Vec::new();
    let mut measured = Vec::new();
    for group in forest
        .postorder
        .iter()
        .copied()
        .chain(std::iter::once(root))
    {
        work.charge(1)?;
        let children = &forest.children[group];
        let group_node = forest.items.get(group);
        let id = group_node.map(|group| group.item.id);
        let group_measure = group_node.and_then(|group| match group.kind {
            GridItemKind::Group(measure) => Some(measure),
            _ => None,
        });
        work.charge(children.len())?;
        reserve(&mut layout_items, children.len())?;
        reserve(&mut measured, children.len())?;
        for &child in children {
            work.charge(1)?;
            let node = forest.items[child];
            let size = match node.kind {
                GridItemKind::Leaf(size) => size,
                GridItemKind::Group(_) => {
                    containers[child]
                        .as_ref()
                        .expect("a child group precedes its parent")
                        .size
                }
                GridItemKind::EdgeLabel => unreachable!("helper nodes have no placement"),
            };
            layout_items.push(node.item);
            measured.push((node.item.id, size));
        }
        let placements = resolve_grid_placements(id, &layout_items, occurrences, options, work)
            .map_err(|error| match error {
                GridPlacementError::Work(error) => Error::from(error),
                GridPlacementError::AllocationFailed(_) => allocation_error(),
                error @ GridPlacementError::AlignmentConflict { .. } => Error::InvalidModel {
                    message: error.to_string(),
                },
            })?;
        let tracks = measure_grid_tracks(&placements, &measured, spacing, work)?;
        let gaps = label_gaps(group, children, &tracks)?;
        containers[group] = Some(materialize_grid_container(
            &tracks,
            &gaps,
            group_measure,
            work,
        )?);
        layout_items.clear();
        measured.clear();
    }

    let mut locations = Vec::new();
    reserve(&mut locations, root)?;
    locations.resize(root, None);
    let mut pending = Vec::new();
    reserve(&mut pending, total)?;
    pending.push((root, Point::default()));
    // Traverse in local container order, translating each shell, track, and title together.
    while let Some((group, offset)) = pending.pop() {
        work.charge(1)?;
        let geometry = containers[group]
            .as_mut()
            .expect("the measured forest owns each container");
        translate_rect(&mut geometry.content, offset);
        if let Some(title) = &mut geometry.title_bounds {
            translate_rect(title, offset);
        }
        for coordinate in &mut geometry.vertical_corridors {
            work.charge(1)?;
            *coordinate += offset.x;
        }
        for coordinate in &mut geometry.horizontal_corridors {
            work.charge(1)?;
            *coordinate += offset.y;
        }
        for (index, item) in geometry.items.iter_mut().enumerate() {
            work.charge(1)?;
            let slot = forest.children[group][item.item_index];
            item.item_index = slot;
            translate_rect(&mut item.bounds, offset);
            translate_rect(&mut item.cell_bounds, offset);
            item.corridors = item.corridors + kurbo::Vec2::new(offset.x, offset.y);
            if !item.bounds.x.is_finite()
                || !item.bounds.y.is_finite()
                || !item.corridors.is_finite()
            {
                return Err(Error::InvalidModel {
                    message: "Grid absolute geometry exceeds the finite coordinate range"
                        .to_owned(),
                });
            }
            locations[slot] = Some((group, index));
        }
        for &slot in forest.children[group].iter().rev() {
            work.charge(1)?;
            if is_group(forest.items, slot) {
                let (_, index) = locations[slot].expect("direct child has a placement");
                let bounds = geometry.items[index].bounds;
                pending.push((
                    slot,
                    Point {
                        x: bounds.x,
                        y: bounds.y,
                    },
                ));
            }
        }
    }
    work.checkpoint(OperationPhase::Layout)?;
    Ok(GridForestGeometry {
        containers,
        locations,
    })
}

fn translate_rect(rect: &mut Rect, offset: Point) {
    rect.x += offset.x;
    rect.y += offset.y;
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::resources::{RenderResourcePolicy, ResourceLimitId};
    use merman_model::OperationControl;

    const SPACING: GridSpacing = GridSpacing {
        row_gap: 34.0,
        column_gap: 24.0,
        cell_gap: 10.0,
        container_padding: 20.0,
        title_gap: 8.0,
    };

    const LEAF: GridItemKind = GridItemKind::Leaf(Size {
        width: 20.0,
        height: 10.0,
    });
    const GROUP: GridItemKind = GridItemKind::Group(GridGroupMeasure {
        padding: 12.0,
        title: None,
    });

    fn item<'a>(
        id: &'a str,
        parent_id: Option<&'a str>,
        kind: GridItemKind,
        order: usize,
    ) -> GridMeasuredItem<'a> {
        GridMeasuredItem {
            item: GridItem {
                id,
                authored_id: None,
                source_ordinal: Some(order),
                inline: GridPlacementSpec::default(),
            },
            parent_id,
            kind,
        }
    }

    fn layout(
        items: &[GridMeasuredItem<'_>],
        work: &OperationWorkMeter,
    ) -> Result<GridForestGeometry> {
        let forest = GridForest::new(items, work)?;
        layout_measured_forest(
            &forest,
            &HashMap::new(),
            GridPlacementOptions::default(),
            SPACING,
            |_, _, _| Ok(GridTrackGaps::default()),
            work,
        )
    }

    #[test]
    fn subtree_sizes_and_absolute_corridors_move_with_the_same_group() {
        let items = [
            item("outside", None, LEAF, 0),
            item("outer", None, GROUP, 1),
            item("inner", Some("outer"), GROUP, 2),
            item("leaf", Some("inner"), LEAF, 3),
            item("label", Some("missing"), GridItemKind::EdgeLabel, 4),
        ];
        let work = OperationWorkMeter::new(RenderResourcePolicy::unbounded_for_trusted_input());
        let forest = GridForest::new(&items, &work).unwrap();
        assert_eq!(forest.by_id["inner"], 2);
        assert_eq!(forest.parents, [Some(5), Some(5), Some(1), Some(2), None]);
        assert_eq!(forest.postorder, [2, 1]);
        let geometry = layout(&items, &work).unwrap();
        assert_eq!(
            geometry.root().size,
            Size {
                width: 240.0,
                height: 186.0
            }
        );
        let outer = geometry.item(1).unwrap();
        let inner = geometry.item(2).unwrap();
        let leaf = geometry.item(3).unwrap();
        assert_eq!((outer.bounds.x, outer.bounds.y), (44.0, 0.0));
        assert_eq!((inner.bounds.x, inner.bounds.y), (88.0, 44.0));
        assert_eq!((leaf.bounds.x, leaf.bounds.y), (132.0, 88.0));
        assert_eq!(
            geometry.containers[2].as_ref().unwrap().content,
            leaf.bounds
        );
        assert_eq!(leaf.corridors, kurbo::Rect::new(112.0, 68.0, 172.0, 118.0));
        assert!(geometry.item(4).is_none());
    }

    #[test]
    fn containment_reports_source_order_and_rejects_reserved_or_invalid_ids() {
        let work = OperationWorkMeter::new(RenderResourcePolicy::unbounded_for_trusted_input());
        for (items, expected) in [
            (vec![item("__grid_root__", None, LEAF, 0)], "reserved"),
            (
                vec![
                    item("a", None, LEAF, 0),
                    item("a", None, GridItemKind::EdgeLabel, 1),
                ],
                "Duplicate node id \"a\"",
            ),
            (
                vec![item("a", None, LEAF, 0), item("b", Some("a"), LEAF, 1)],
                "Missing parent \"a\" for \"b\"",
            ),
            (
                vec![
                    item("b", Some("a"), GROUP, 0),
                    item("a", Some("b"), GROUP, 1),
                ],
                "Containment cycle at \"b\"",
            ),
        ] {
            assert!(
                GridForest::new(&items, &work)
                    .unwrap_err()
                    .to_string()
                    .contains(expected)
            );
        }
    }

    #[test]
    fn forest_and_geometry_use_the_same_work_and_cancellation_control() {
        let items = [
            item("group", None, GROUP, 0),
            item("child", Some("group"), LEAF, 1),
        ];
        let policy = RenderResourcePolicy::unbounded_for_trusted_input();
        let work = OperationWorkMeter::new(policy.clone());
        layout(&items, &work).unwrap();
        let used = work.used();
        let with_limit = |limit| {
            OperationWorkMeter::new(
                policy
                    .clone()
                    .with_limit(ResourceLimitId::MaxLayoutWorkUnits, limit)
                    .unwrap(),
            )
        };
        assert!(layout(&items, &with_limit(used - 1)).is_err());
        assert!(layout(&items, &with_limit(used)).is_ok());
        let control = OperationControl::new();
        control.cancel_after_checkpoints(8);
        let cancelled = OperationWorkMeter::new_with_control(policy, control);
        assert!(matches!(
            layout(&items, &cancelled),
            Err(Error::Cancelled(_))
        ));
    }
}
